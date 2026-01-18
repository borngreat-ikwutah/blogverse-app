use axum::{extract::State, response::IntoResponse, Json};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::{
        jwt, utils, AuthResponse, AuthToken, ForgotPasswordRequest, LoginUser, RegisterUser,
        ResendVerificationRequest, ResetPasswordRequest, User, UserResponse, VerifyEmailRequest,
    },
    config::settings::Settings,
    email::EmailService,
    error::AppError,
    response::ApiResponse,
};

/// POST /api/auth/sign-up
/// Creates a new user and sends verification email
pub async fn signup(
    State(pool): State<PgPool>,
    State(email_service): State<EmailService>,
    Json(payload): Json<RegisterUser>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let password_hash =
        utils::hash_password(&payload.password).map_err(|_| AppError::InternalServerError)?;

    let user_id = Uuid::new_v4();

    // Create user with email_verified = false
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (id, username, email, password_hash, email_verified) VALUES ($1, $2, $3, $4, false) RETURNING *",
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

    // Generate verification token
    let token = utils::generate_secure_token();
    let expires_at = Utc::now() + Duration::hours(24);

    sqlx::query(
        "INSERT INTO auth_tokens (user_id, token, token_type, expires_at) VALUES ($1, $2, 'email_verification', $3)",
    )
    .bind(user.id)
    .bind(&token)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    // Send verification email (don't fail signup if email fails)
    if let Err(e) = email_service
        .send_verification_email(&user.email, &token)
        .await
    {
        tracing::error!("Failed to send verification email: {:?}", e);
    }

    Ok(ApiResponse::success_with_message(
        "Account created. Please check your email to verify your account.".to_string(),
        UserResponse::from(user),
    )
    .created())
}

/// POST /api/auth/verify-email
/// Verifies user email with token
pub async fn verify_email(
    State(pool): State<PgPool>,
    State(email_service): State<EmailService>,
    Json(payload): Json<VerifyEmailRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Find valid token
    let auth_token = sqlx::query_as::<_, AuthToken>(
        "SELECT * FROM auth_tokens WHERE token = $1 AND token_type = 'email_verification' AND expires_at > NOW() AND used_at IS NULL",
    )
    .bind(&payload.token)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?
    .ok_or(AppError::BadRequest("Invalid or expired token".to_string()))?;

    // Mark token as used and verify email
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    sqlx::query("UPDATE auth_tokens SET used_at = NOW() WHERE id = $1")
        .bind(auth_token.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let user = sqlx::query_as::<_, User>(
        "UPDATE users SET email_verified = true, updated_at = NOW() WHERE id = $1 RETURNING *",
    )
    .bind(auth_token.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Send welcome email
    if let Err(e) = email_service
        .send_welcome_email(&user.email, &user.username)
        .await
    {
        tracing::error!("Failed to send welcome email: {:?}", e);
    }

    Ok(ApiResponse::ok("Email verified successfully".to_string()))
}

/// POST /api/auth/resend-verification
/// Resends verification email
pub async fn resend_verification(
    State(pool): State<PgPool>,
    State(email_service): State<EmailService>,
    Json(payload): Json<ResendVerificationRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    // Find user
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Always return success to prevent email enumeration
    let Some(user) = user else {
        return Ok(ApiResponse::ok(
            "If an account exists, a verification email has been sent.".to_string(),
        ));
    };

    if user.email_verified {
        return Ok(ApiResponse::ok("Email is already verified.".to_string()));
    }

    // Invalidate existing tokens
    sqlx::query("UPDATE auth_tokens SET used_at = NOW() WHERE user_id = $1 AND token_type = 'email_verification' AND used_at IS NULL")
        .bind(user.id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Generate new token
    let token = utils::generate_secure_token();
    let expires_at = Utc::now() + Duration::hours(24);

    sqlx::query(
        "INSERT INTO auth_tokens (user_id, token, token_type, expires_at) VALUES ($1, $2, 'email_verification', $3)",
    )
    .bind(user.id)
    .bind(&token)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    // Send email
    if let Err(e) = email_service
        .send_verification_email(&user.email, &token)
        .await
    {
        tracing::error!("Failed to send verification email: {:?}", e);
    }

    Ok(ApiResponse::ok(
        "If an account exists, a verification email has been sent.".to_string(),
    ))
}

/// POST /api/auth/sign-in
/// Login with email and password
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

    // Check if email is verified
    if !user.email_verified {
        return Err(AppError::UnprocessableEntity(
            "Please verify your email before logging in".to_string(),
        ));
    }

    let token = jwt::create_token(user.id, &settings.jwt_secret)
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::success(AuthResponse {
        token,
        user: UserResponse::from(user),
    }))
}

/// POST /api/auth/forgot-password
/// Request password reset email
pub async fn forgot_password(
    State(pool): State<PgPool>,
    State(email_service): State<EmailService>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    // Find user
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Always return success to prevent email enumeration
    let Some(user) = user else {
        return Ok(ApiResponse::ok(
            "If an account exists, a password reset email has been sent.".to_string(),
        ));
    };

    // Invalidate existing reset tokens
    sqlx::query("UPDATE auth_tokens SET used_at = NOW() WHERE user_id = $1 AND token_type = 'password_reset' AND used_at IS NULL")
        .bind(user.id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Generate reset token (expires in 1 hour)
    let token = utils::generate_secure_token();
    let expires_at = Utc::now() + Duration::hours(1);

    sqlx::query(
        "INSERT INTO auth_tokens (user_id, token, token_type, expires_at) VALUES ($1, $2, 'password_reset', $3)",
    )
    .bind(user.id)
    .bind(&token)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    // Send reset email
    if let Err(e) = email_service
        .send_password_reset_email(&user.email, &token)
        .await
    {
        tracing::error!("Failed to send password reset email: {:?}", e);
    }

    Ok(ApiResponse::ok(
        "If an account exists, a password reset email has been sent.".to_string(),
    ))
}

/// POST /api/auth/reset-password
/// Reset password with token
pub async fn reset_password(
    State(pool): State<PgPool>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    // Find valid token
    let auth_token = sqlx::query_as::<_, AuthToken>(
        "SELECT * FROM auth_tokens WHERE token = $1 AND token_type = 'password_reset' AND expires_at > NOW() AND used_at IS NULL",
    )
    .bind(&payload.token)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?
    .ok_or(AppError::BadRequest("Invalid or expired token".to_string()))?;

    // Hash new password
    let password_hash =
        utils::hash_password(&payload.new_password).map_err(|_| AppError::InternalServerError)?;

    // Update password and mark token as used
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    sqlx::query("UPDATE auth_tokens SET used_at = NOW() WHERE id = $1")
        .bind(auth_token.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&password_hash)
        .bind(auth_token.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::ok("Password reset successfully".to_string()))
}

/// GET /api/auth/me
/// Get current user profile
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

/// GET /api/user/:id
/// Get user by ID
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
