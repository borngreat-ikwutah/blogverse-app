use axum::{
    extract::FromRef,
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing::info;

mod auth;
mod comments;
mod config;
mod email;
mod error;
mod follows;
mod response;
mod stories;

use config::settings::Settings;
use email::EmailService;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
    settings: Settings,
    email_service: EmailService,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(app_state: &AppState) -> PgPool {
        app_state.pool.clone()
    }
}

impl FromRef<AppState> for Settings {
    fn from_ref(app_state: &AppState) -> Settings {
        app_state.settings.clone()
    }
}

impl FromRef<AppState> for EmailService {
    fn from_ref(app_state: &AppState) -> EmailService {
        app_state.email_service.clone()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let settings = Settings::new();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await?;

    info!("Database connected");

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;
    info!("Migrations complete");

    // Initialize email service
    let email_service = EmailService::new(
        &settings.smtp_host,
        settings.smtp_port,
        &settings.smtp_username,
        &settings.smtp_password,
        &settings.from_email,
        &settings.from_name,
        &settings.frontend_url,
    )?;
    info!("Email service initialized");

    let app_state = AppState {
        pool,
        settings: settings.clone(),
        email_service,
    };

    // Auth routes
    let auth_router = Router::new()
        .route("/sign-up", post(auth::handler::signup))
        .route("/sign-in", post(auth::handler::login))
        .route("/verify-email", post(auth::handler::verify_email))
        .route(
            "/resend-verification",
            post(auth::handler::resend_verification),
        )
        .route("/forgot-password", post(auth::handler::forgot_password))
        .route("/reset-password", post(auth::handler::reset_password))
        .route("/me", get(auth::handler::get_me));

    // User routes (with follow operations)
    let user_router = Router::new()
        .route("/{id}", get(auth::handler::get_user_by_id))
        .route("/{id}/profile", get(follows::handler::get_user_profile))
        .route(
            "/{id}/follow",
            post(follows::handler::follow_user).delete(follows::handler::unfollow_user),
        )
        .route("/{id}/followers", get(follows::handler::get_followers))
        .route("/{id}/following", get(follows::handler::get_following))
        .route("/{id}/is-following", get(follows::handler::check_following));

    // Story routes
    let story_router = Router::new()
        .route(
            "/",
            post(stories::handler::create_story).get(stories::handler::get_feed),
        )
        .route("/s/{slug}", get(stories::handler::get_story))
        // More specific routes must come before /{id}
        .route("/{id}/clap", post(stories::handler::clap_story))
        .route(
            "/{id}/comments",
            post(comments::handler::create_comment).get(comments::handler::get_story_comments),
        )
        // Generic /{id} route comes last
        .route(
            "/{id}",
            axum::routing::put(stories::handler::update_story)
                .delete(stories::handler::delete_story),
        );

    // Tag routes
    let tag_router = Router::new().route("/", get(stories::handler::get_tags));

    // Comment routes (for individual comment operations)
    let comment_router = Router::new()
        .route(
            "/{id}",
            get(comments::handler::get_comment)
                .put(comments::handler::update_comment)
                .delete(comments::handler::delete_comment),
        )
        .route("/{id}/replies", get(comments::handler::get_comment_replies))
        .route("/{id}/clap", post(comments::handler::clap_comment));

    // Feed routes (personalized feed)
    let feed_router = Router::new().route("/following", get(follows::handler::get_following_feed));

    let app = Router::new()
        .route("/", get(|| async { "BlogVerse API v1.0" }))
        .route("/health", get(|| async { "OK" }))
        .nest("/api/auth", auth_router)
        .nest("/api/user", user_router)
        .nest("/api/stories", story_router)
        .nest("/api/tags", tag_router)
        .nest("/api/comments", comment_router)
        .nest("/api/feed", feed_router)
        .with_state(app_state);

    info!("Server running on http://localhost:{}", settings.port);

    let listener = tokio::net::TcpListener::bind(settings.addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
