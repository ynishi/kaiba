use axum::{
    middleware,
    routing::get,
    Router,
    Json,
};
use serde::Serialize;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;

mod auth;
mod models;
mod routes;
mod services;

#[derive(Serialize)]
struct HealthCheck {
    status: String,
    message: String,
    version: String,
}

async fn health_check() -> Json<HealthCheck> {
    Json(HealthCheck {
        status: "ok".to_string(),
        message: "Kaiba API is running - memories flow through the hippocampus".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
    #[shuttle_runtime::Secrets] secrets: shuttle_runtime::SecretStore,
) -> shuttle_axum::ShuttleAxum {
    tracing::info!("🧠 Kaiba API initializing...");

    // Initialize API key from secrets
    if let Some(api_key) = secrets.get("KAIBA_API_KEY") {
        auth::init_api_key(api_key);
        tracing::info!("🔐 API key authentication enabled");
    } else {
        tracing::warn!("⚠️  No KAIBA_API_KEY set - authentication disabled");
    }

    // Run migrations
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("✅ Database migrations completed");

    // Protected routes (require authentication)
    let protected_routes = Router::new()
        .merge(routes::rei::router())
        .merge(routes::tei::router())
        .merge(routes::call::router())
        .layer(middleware::from_fn(auth::auth_middleware));

    // Build router with shared state
    let router = Router::new()
        .route("/health", get(health_check))
        .merge(protected_routes)
        .layer(CorsLayer::permissive())
        .with_state(pool);

    tracing::info!("✅ Kaiba API ready - Rei awakens in Tei");

    Ok(router.into())
}
