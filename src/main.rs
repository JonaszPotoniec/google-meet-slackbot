use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use serde_json::{json, Value};
use std::env;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod crypto;
mod database;
mod google;
mod handlers;
mod models;
mod rate_limiter;
mod utils;
mod validation;

use database::Database;
use rate_limiter::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub rate_limiter: RateLimiter,
    pub slack_signing_secret: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "meet_slack_bot=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/bot.db".to_string());
    
    let db = Database::new(&database_url).await?;
    db.migrate().await?;

    let rate_limiter = RateLimiter::new();
    let state = AppState {
        db,
        rate_limiter: rate_limiter.clone(),
        slack_signing_secret: env::var("SLACK_SIGNING_SECRET")
            .expect("SLACK_SIGNING_SECRET must be set"),
        google_client_id: env::var("GOOGLE_CLIENT_ID")
            .expect("GOOGLE_CLIENT_ID must be set"),
        google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
            .expect("GOOGLE_CLIENT_SECRET must be set"),
        google_redirect_uri: env::var("GOOGLE_REDIRECT_URI")
            .expect("GOOGLE_REDIRECT_URI must be set"),
    };

    tokio::spawn(rate_limiter::start_cleanup_task(rate_limiter));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/slack/commands", post(handlers::slack::handle_slash_command))
        .route("/auth/google", get(handlers::auth::initiate_google_oauth))
        .route("/auth/google/callback", get(handlers::auth::handle_google_callback))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{}:{}", host, port);

    info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[instrument]
async fn health_check() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "status": "healthy",
        "service": "meet-slack-bot",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}
