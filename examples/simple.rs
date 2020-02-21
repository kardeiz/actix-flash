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
