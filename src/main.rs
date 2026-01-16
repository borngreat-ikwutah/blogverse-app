use axum::{
    extract::FromRef,
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing::info;

mod auth;
mod config;
mod error;
mod response;
mod stories;

use config::settings::Settings;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
    settings: Settings,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let settings = Settings::new();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await?;

    info!("database connected");

    let app_state = AppState {
        pool,
        settings: settings.clone(),
    };

    let auth_router = Router::new()
        .route("/sign-in", post(auth::handler::login))
        .route("/sign-up", post(auth::handler::signup))
        .route("/me", get(auth::handler::get_me));

    let user_router = Router::new().route("/:id", get(auth::handler::get_user_by_id));

    let story_router = Router::new()
        .route(
            "/",
            post(stories::handler::create_story).get(stories::handler::get_feed),
        )
        .route("/s/:slug", get(stories::handler::get_story))
        .route(
            "/:id",
            axum::routing::put(stories::handler::update_story)
                .delete(stories::handler::delete_story),
        )
        .route("/:id/clap", post(stories::handler::clap_story));

    let tag_router = Router::new().route("/", get(stories::handler::get_tags));

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .nest("/api/auth", auth_router)
        .nest("/api/user", user_router)
        .nest("/api/stories", story_router)
        .nest("/api/tags", tag_router)
        .with_state(app_state);

    info!("Server running on http://localhost:{}", settings.port);

    let listener = tokio::net::TcpListener::bind(settings.addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
