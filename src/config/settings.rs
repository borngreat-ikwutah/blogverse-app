use std::env;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct Settings {
    pub port: u16,
    pub addr: SocketAddr,
    pub database_url: String,
    pub jwt_secret: String,
}

impl Settings {
    pub fn new() -> Self {
        let port: u16 = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3000);
        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");

        Self {
            port,
            addr,
            database_url,
            jwt_secret,
        }
    }
}
