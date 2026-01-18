use std::env;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct Settings {
    pub port: u16,
    pub addr: SocketAddr,
    pub database_url: String,
    pub jwt_secret: String,
    // Email settings
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_email: String,
    pub from_name: String,
    pub frontend_url: String,
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

        // Email settings with defaults for development
        let smtp_host = env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string());
        let smtp_port: u16 = env::var("SMTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(587);
        let smtp_username = env::var("SMTP_USERNAME").unwrap_or_default();
        let smtp_password = env::var("SMTP_PASSWORD").unwrap_or_default();
        let from_email =
            env::var("FROM_EMAIL").unwrap_or_else(|_| "noreply@blogverse.com".to_string());
        let from_name = env::var("FROM_NAME").unwrap_or_else(|_| "BlogVerse".to_string());
        let frontend_url =
            env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

        Self {
            port,
            addr,
            database_url,
            jwt_secret,
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            from_email,
            from_name,
            frontend_url,
        }
    }
}
