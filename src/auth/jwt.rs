use anyhow::Result;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
    RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::settings::Settings;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: i64,
    pub iat: i64,
}

pub fn create_token(user_id: Uuid, secret: &str) -> Result<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        exp: (now + Duration::hours(1)).timestamp(),
        iat: now.timestamp(),
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )?)
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
    Settings: FromRef<S>,
{
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;

        let settings = Settings::from_ref(state);

        let token = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(settings.jwt_secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;

        Ok(token.claims)
    }
}
