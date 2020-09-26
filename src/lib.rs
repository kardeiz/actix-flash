/*!
Flash message middleware for `actix-web` 2.0 or 3.0.

Supports `actix-web` 3.0 by default. For 2.0, use:

```
actix-flash = { version = "0.2", features = ["v2"], default-features = false }
```

For `actix-web` 1.0 support, check out [`actix-web-flash`](https://github.com/hatzel/actix-web-flash).

# Usage

```rust,no_run
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};

async fn show_flash(flash: actix_flash::Message<String>) -> impl Responder {
    flash.into_inner()
}

async fn set_flash(_req: HttpRequest) -> actix_flash::Response<HttpResponse, String> {
    actix_flash::Response::with_redirect("This is the message".to_owned(), "/show_flash")
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .wrap(actix_flash::Flash::default())
            .route("/show_flash", web::get().to(show_flash))
            .route("/set_flash", web::get().to(set_flash))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```
*/

use std::rc::Rc;
use std::task::{Context, Poll};

use futures::future::{err, ok, LocalBoxFuture, Ready, TryFutureExt};

use serde::{de::DeserializeOwned, Serialize, Deserialize};

use actix_service::{Service, Transform};

#[cfg(feature = "v2")]
pub(crate) use actix_web_v2 as actix_web;

#[cfg(feature = "v3")]
pub(crate) use actix_web_v3 as actix_web;

use actix_web::cookie::{Cookie, CookieJar};
use actix_web::dev::{MessageBody, ServiceRequest, ServiceResponse};
use actix_web::error::{Error, ErrorBadRequest, Result};
use actix_web::{FromRequest, HttpMessage, HttpRequest, HttpResponse, Responder};

#[derive(Debug)]
struct FlashCookie(Cookie<'static>);
#[derive(Clone)]
struct FlashCookieValue(String);

/// The flash message wrapper
#[derive(Debug)]
pub struct Message<T>(T);

#[derive(Deserialize)]
struct ValuedMessage<T> {
    #[serde(rename="_")]
    value: T
}

#[derive(Serialize)]
struct ValuedMessageRef<'a, T> {
    #[serde(rename="_")]
    value: &'a T
}

impl<T> FromRequest for Message<T>
where
    T: DeserializeOwned + Serialize,
{
    type Config = ();
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = Error;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        if let Some(cookie) = req.extensions().get::<FlashCookie>() {
            match serde_json::from_str(cookie.0.value()) {
                Ok(ValuedMessage { value }) => { return ok(Message(value)); },
                _ => {}
            }
        }
        err(ErrorBadRequest("Invalid/missing flash cookie"))
    }
}

impl<T> Message<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

/// The "flashing" response
pub struct Response<R, T>
where
    R: Responder,
    T: Serialize + DeserializeOwned,
{
    responder: R,
    message: Option<Message<T>>,
}

impl<R, T> Responder for Response<R, T>
where
    R: Responder + 'static,
    T: Serialize + DeserializeOwned + 'static,
{
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<HttpResponse, Self::Error>>;

    fn respond_to(mut self, req: &HttpRequest) -> Self::Future {
        let msg = self.message.take();

        let out = self.responder.respond_to(req).err_into().and_then(|mut res| async {
            if let Some(msg) = msg {
                let json = serde_json::to_string(&ValuedMessageRef { value: &msg.0 })?;
                res.extensions_mut().insert(FlashCookieValue(json));
            }
            Ok(res)
        });

        Box::pin(out)
    }
}

impl<R, T> Response<R, T>
where
    R: Responder,
    T: Serialize + DeserializeOwned,
{
    pub fn new(message: Option<T>, responder: R) -> Self {
        Self { responder, message: message.map(Message) }
    }
}

impl<T> Response<HttpResponse, T>
where
    T: Serialize + DeserializeOwned,
{
    /// Create a new flashing response with given message and redirect location.
    pub fn with_redirect(message: T, location: &str) -> Self {
        let response =
            HttpResponse::SeeOther().header(actix_web::http::header::LOCATION, location).finish();
        Self { message: Some(Message(message)), responder: response }
    }
}

/// The flash middleware transformer
pub struct Flash {
    cookie_name: Rc<str>,
}

impl Flash {
    /// Create a new flash middleware transformer, using the given string as the cookie name.
    pub fn new<I: Into<Rc<str>>>(cookie_name: I) -> Self {
        Self { cookie_name: cookie_name.into() }
    }
}

impl Default for Flash {
    fn default() -> Self {
        Self::new("_flash")
    }
}

/// The actual flash middleware
pub struct FlashMiddleware<S> {
    cookie_name: Rc<str>,
    service: S,
}

impl<S, B> Transform<S> for Flash
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = FlashMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(FlashMiddleware { service, cookie_name: self.cookie_name.clone() })
    }
}

impl<S, B> Service for FlashMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let cookie_name = String::from(self.cookie_name.as_ref());

        if let Some(cookie) = req.cookie(&cookie_name) {
            req.extensions_mut().insert(FlashCookie(cookie));
        }

        Box::pin(self.service.call(req).and_then(|mut res| async move {
            let maybe_set_cookie = res.response().extensions().get::<FlashCookieValue>().cloned();

            if let Some(FlashCookieValue(json)) = maybe_set_cookie {
                let mut cookie = Cookie::new(cookie_name.clone(), json);
                cookie.set_path("/");
                res.response_mut().add_cookie(&cookie)?;
            }

            let mut jar = CookieJar::new();
            if let Some(cookie) = res.request().cookie(&cookie_name) {
                jar.add_original(cookie);
                jar.remove(Cookie::build(cookie_name, "").path("/").finish());
            }

            for cookie in jar.delta() {
                res.response_mut().add_cookie(cookie)?;
            }

            Ok(res)
        }))
    }
}
