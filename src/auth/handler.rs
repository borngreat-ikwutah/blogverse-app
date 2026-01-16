use axum::{extract::State, response::IntoResponse, Json};
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::{jwt, utils, AuthResponse, LoginUser, RegisterUser, User, UserResponse},
    config::settings::Settings,
    error::AppError,
    response::ApiResponse,
};

pub async fn signup(
    State(pool): State<PgPool>,
    State(settings): State<Settings>,
    Json(payload): Json<RegisterUser>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let password_hash =
        utils::hash_password(&payload.password).map_err(|_| AppError::InternalServerError)?;

    let user_id = Uuid::new_v4();

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(user_id)
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .fetch_one(&pool)
    .await
    .map_err(|e: sqlx::Error| {
        if e.to_string().contains("duplicate key value") {
            AppError::Conflict("Username or Email already exists".to_string())
        } else {
            tracing::error!("Database error: {:?}", e);
            AppError::InternalServerError
        }
    })?;

    let token = jwt::create_token(user.id, &settings.jwt_secret)
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::success(AuthResponse {
        token,
        user: UserResponse::from(user),
    })
    .created())
}

pub async fn get_user_by_id(
    State(pool): State<PgPool>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {:?}", e);
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    Ok(ApiResponse::success(UserResponse::from(user)))
}

pub async fn login(
    State(pool): State<PgPool>,
    State(settings): State<Settings>,
    Json(payload): Json<LoginUser>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {:?}", e);
            AppError::InternalServerError
        })?
        .ok_or(AppError::Unauthorized)?;

    utils::verify_password(&user.password_hash, &payload.password)
        .map_err(|_| AppError::Unauthorized)?;

    let token = jwt::create_token(user.id, &settings.jwt_secret)
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::success(AuthResponse {
        token,
        user: UserResponse::from(user),
    }))
}

pub async fn get_me(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
) -> Result<impl IntoResponse, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(claims.sub)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {:?}", e);
            AppError::InternalServerError
        })?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    Ok(ApiResponse::success(UserResponse::from(user)))
}
