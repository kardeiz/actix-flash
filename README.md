# actix-flash

[![Docs](https://docs.rs/actix-flash/badge.svg)](https://docs.rs/crate/actix-flash/)
[![Crates.io](https://img.shields.io/crates/v/actix-flash.svg)](https://crates.io/crates/actix-flash)

Flash message middleware for `actix-web` 2.0 or 3.0.

Supports `actix-web` 3.0 by default. For 2.0, use:

```rust
actix-flash = { version = "0.2", features = ["v2"], default-features = false }
```

For `actix-web` 1.0 support, check out [`actix-web-flash`](https://github.com/hatzel/actix-web-flash).

## Usage

```rust
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

<hr/>

Current version: 0.2.0

License: MIT/Apache-2.0
