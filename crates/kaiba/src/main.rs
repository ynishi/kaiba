use axum::{
    extract::FromRef,
    middleware,
    routing::get,
    Router,
    Json,
};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

mod auth;
mod models;
mod routes;
mod services;

use services::qdrant::MemoryKai;
use services::embedding::EmbeddingService;
use services::web_search::WebSearchAgent;
use services::scheduler;

/// Application state shared across all routes
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub memory_kai: Option<Arc<MemoryKai>>,
    pub embedding: Option<EmbeddingService>,
    pub web_search: Option<WebSearchAgent>,
}

// Allow extracting PgPool directly from AppState (for backward compatibility)
impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> PgPool {
        state.pool.clone()
    }
}

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

    // Initialize MemoryKai (Qdrant) if configured
    let memory_kai = match (secrets.get("QDRANT_URL"), secrets.get("QDRANT_API_KEY")) {
        (Some(url), api_key) => {
            match MemoryKai::new(&url, api_key).await {
                Ok(kai) => {
                    tracing::info!("🌊 MemoryKai (記憶海) connected");
                    Some(Arc::new(kai))
                }
                Err(e) => {
                    tracing::warn!("⚠️  Failed to connect to MemoryKai: {}", e);
                    None
                }
            }
        }
        _ => {
            tracing::warn!("⚠️  No QDRANT_URL set - MemoryKai disabled");
            None
        }
    };

    // Initialize Embedding service if configured
    let embedding = secrets.get("OPENAI_API_KEY").map(|key| {
        tracing::info!("🧬 Embedding service initialized");
        EmbeddingService::new(key)
    });

    if embedding.is_none() {
        tracing::warn!("⚠️  No OPENAI_API_KEY set - Embedding disabled");
    }

    // Initialize WebSearch agent if configured
    let web_search = secrets.get("GEMINI_API_KEY").map(|key| {
        tracing::info!("🔍 WebSearch agent initialized (Gemini)");
        WebSearchAgent::new(key)
    });

    if web_search.is_none() {
        tracing::warn!("⚠️  No GEMINI_API_KEY set - WebSearch disabled");
    }

    // Create application state
    let state = AppState {
        pool: pool.clone(),
        memory_kai: memory_kai.clone(),
        embedding: embedding.clone(),
        web_search: web_search.clone(),
    };

    // Start autonomous scheduler (1 hour interval)
    let scheduler_interval = secrets
        .get("LEARNING_INTERVAL_SECS")
        .and_then(|s| s.parse().ok());
    let gemini_api_key = secrets.get("GEMINI_API_KEY");

    if let Some(_handle) = scheduler::maybe_start_scheduler(
        pool,
        memory_kai,
        embedding,
        web_search,
        gemini_api_key,
        scheduler_interval,
    ) {
        tracing::info!("📅 Autonomous scheduler started");
    } else {
        tracing::warn!("⚠️  Autonomous scheduler disabled (missing services)");
    }

    // Protected routes (require authentication)
    let protected_routes = Router::new()
        .merge(routes::rei::router())
        .merge(routes::tei::router())
        .merge(routes::call::router())
        .merge(routes::memory::router())
        .merge(routes::search::router())
        .merge(routes::learning::router())
        .merge(routes::prompt::router())
        .layer(middleware::from_fn(auth::auth_middleware));

    // Build router with shared state
    let router = Router::new()
        .route("/health", get(health_check))
        .merge(protected_routes)
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!("✅ Kaiba API ready - Rei awakens in Tei");

    Ok(router.into())
}
