use crate::handlers::{auth, health};
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(web::resource("/health").route(web::get().to(health::health_check)))
            .service(
                web::scope("/auth"), // .service(web::resource("/signup").route(web::post().to(auth::create_user)))
                                     // .service(web::resource("/login").route(web::post().to(auth::login_handler))),
            ),
    );
}
