use actix_web::{HttpResponse, Result};
use chrono::{SecondsFormat, Utc};
use serde_json::json;

pub async fn health_check() -> Result<HttpResponse> {
    let current_timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    Ok(HttpResponse::Ok().json(json!({
        "status": "healthy",
        "service": "blogverse-backend",
        "timestamp": current_timestamp,
        "version": env!("CARGO_PKG_VERSION")
    })))
}
