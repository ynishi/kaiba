use axum::{extract::FromRef, middleware, routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod adapters;
mod application;
mod auth;
mod config;
mod models;
mod routes;
mod services;

use adapters::{
    HttpWebhook, Neo4jGraphRepository, PgDocRepository, PgReiRepository, PgReiWebhookRepository,
    PgTeiRepository,
};
use application::{ReiService, TeiService};
use config::Config;
use secrecy::ExposeSecret;
use services::decision::{create_decision_engine, LlmEngineConfig};
use services::embedding::EmbeddingService;
use services::qdrant::MemoryKai;
use services::scheduler;
use services::web_search::WebSearchAgent;
use services::HybridSearchService;

/// Type aliases for application services with concrete repository implementations
pub type AppReiService = ReiService<PgReiRepository>;
pub type AppTeiService = TeiService<PgTeiRepository>;

/// Application state shared across all routes
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub rei_service: Arc<AppReiService>,
    pub tei_service: Arc<AppTeiService>,
    pub doc_store: Option<Arc<PgDocRepository>>,
    pub memory_kai: Option<Arc<MemoryKai>>,
    pub graph_kai: Option<Arc<Neo4jGraphRepository>>,
    pub hybrid_search: Option<Arc<HybridSearchService>>,
    pub embedding: Option<EmbeddingService>,
    pub web_search: Option<WebSearchAgent>,
    pub webhook_repo: Arc<PgReiWebhookRepository>,
    pub http_webhook: Arc<HttpWebhook>,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kaiba_server=info,tower_http=info".into()),
        )
        .init();

    let cfg = Config::from_env()?;

    tracing::info!("Kaiba API initializing...");

    // Authentication
    if let Some(ref api_key) = cfg.kaiba_api_key {
        auth::init_api_key(api_key.expose_secret().clone());
        tracing::info!("API key authentication enabled");
    } else {
        tracing::warn!("No KAIBA_API_KEY set - authentication disabled");
    }

    // PostgreSQL
    let pool = PgPool::connect(cfg.database_url.expose_secret()).await?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run database migrations: {}", e))?;
    tracing::info!("Database migrations completed");

    // MemoryKai (Qdrant)
    let memory_kai = match (&cfg.qdrant_url, &cfg.qdrant_api_key) {
        (Some(url), api_key) => {
            let key = api_key.as_ref().map(|s| s.expose_secret().clone());
            match MemoryKai::new(url, key).await {
                Ok(kai) => {
                    tracing::info!("MemoryKai connected");
                    Some(Arc::new(kai))
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to MemoryKai: {}", e);
                    None
                }
            }
        }
        _ => {
            tracing::warn!("No QDRANT_URL set - MemoryKai disabled");
            None
        }
    };

    // Embedding (OpenAI)
    let embedding = cfg.openai_api_key.as_ref().map(|key| {
        tracing::info!("Embedding service initialized");
        EmbeddingService::new(key.expose_secret().clone())
    });
    if embedding.is_none() {
        tracing::warn!("No OPENAI_API_KEY set - Embedding disabled");
    }

    // WebSearch (Gemini)
    let web_search = cfg.gemini_api_key.as_ref().map(|key| {
        tracing::info!("WebSearch agent initialized (Gemini)");
        WebSearchAgent::new(key.expose_secret().clone())
    });
    if web_search.is_none() {
        tracing::warn!("No GEMINI_API_KEY set - WebSearch disabled");
    }

    // GraphKai (Neo4j)
    let graph_kai = match cfg.neo4j_credentials() {
        Some((uri, user, password)) => match Neo4jGraphRepository::new(uri, user, password).await {
            Ok(repo) => {
                tracing::info!("GraphKai connected to Neo4j");
                Some(Arc::new(repo))
            }
            Err(e) => {
                tracing::warn!("Failed to connect to GraphKai: {}", e);
                None
            }
        },
        None => {
            tracing::warn!("No NEO4J_* credentials set - GraphKai disabled");
            None
        }
    };

    // Decision Engine: Groq > Ollama > RuleBased
    let decision_engine = if let Some(api_key) = &cfg.groq_api_key {
        let mut config = LlmEngineConfig::groq(api_key.expose_secret(), "llama-3.2-3b-preview");
        if let Some(persona) = &cfg.decision_persona {
            config = config.with_persona(persona);
        }
        Arc::from(create_decision_engine(Some(config)))
    } else if let Some(model) = &cfg.ollama_model {
        let mut config = LlmEngineConfig::ollama(model);
        if let Some(url) = &cfg.ollama_url {
            config.base_url = url.clone();
        }
        if let Some(persona) = &cfg.decision_persona {
            config = config.with_persona(persona);
        }
        Arc::from(create_decision_engine(Some(config)))
    } else {
        Arc::from(create_decision_engine(None))
    };

    // Application services
    let rei_repo = Arc::new(PgReiRepository::new(pool.clone()));
    let tei_repo = Arc::new(PgTeiRepository::new(pool.clone()));
    let webhook_repo = Arc::new(PgReiWebhookRepository::new(pool.clone()));
    let doc_store = Arc::new(PgDocRepository::new(pool.clone()));
    let rei_service = Arc::new(ReiService::new(rei_repo));
    let tei_service = Arc::new(TeiService::new(tei_repo));
    let http_webhook = Arc::new(HttpWebhook::new());

    // HybridSearchService
    let hybrid_search = match (&memory_kai, &graph_kai, &embedding) {
        (Some(mem), Some(graph), Some(emb)) => {
            tracing::info!("HybridSearchService initialized (RAG + Graph + DB)");
            Some(Arc::new(HybridSearchService::new(
                mem.clone(),
                graph.clone(),
                Some(doc_store.clone() as Arc<dyn kaiba::DocRepository>),
                emb.clone(),
            )))
        }
        _ => {
            tracing::warn!("HybridSearchService disabled (missing required services)");
            None
        }
    };

    let state = AppState {
        pool: pool.clone(),
        rei_service,
        tei_service,
        doc_store: Some(doc_store),
        memory_kai: memory_kai.clone(),
        graph_kai,
        hybrid_search,
        embedding: embedding.clone(),
        web_search: web_search.clone(),
        webhook_repo,
        http_webhook,
    };

    // Autonomous scheduler
    let doc_store_dyn: Option<Arc<dyn kaiba::DocRepository>> = state
        .doc_store
        .clone()
        .map(|ds| ds as Arc<dyn kaiba::DocRepository>);
    let graph_kai_dyn: Option<Arc<dyn kaiba::GraphRepository>> = state
        .graph_kai
        .clone()
        .map(|gk| gk as Arc<dyn kaiba::GraphRepository>);

    if let Some(_handle) = scheduler::maybe_start_scheduler(
        pool,
        memory_kai,
        embedding,
        web_search,
        cfg.gemini_api_key
            .as_ref()
            .map(|s| s.expose_secret().clone()),
        cfg.learning_interval_secs,
        decision_engine,
        Some(state.webhook_repo.clone()),
        Some(state.http_webhook.clone()),
        doc_store_dyn,
        graph_kai_dyn,
    ) {
        tracing::info!("Autonomous scheduler started");
    } else {
        tracing::warn!("Autonomous scheduler disabled (missing services)");
    }

    // Routes
    let protected_routes = Router::new()
        .merge(routes::rei::router())
        .merge(routes::tei::router())
        .merge(routes::call::router())
        .merge(routes::memory::router())
        .merge(routes::document::router())
        .merge(routes::graph::router())
        .merge(routes::search::router())
        .merge(routes::learning::router())
        .merge(routes::prompt::router())
        .merge(routes::webhook::router())
        .merge(routes::dashboard::router())
        .merge(routes::trigger::router())
        .layer(middleware::from_fn(auth::auth_middleware));

    let openapi = routes::swagger::ApiDoc::openapi();

    let router = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi))
        .route("/health", get(health_check))
        .merge(protected_routes)
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server
    let addr = format!("0.0.0.0:{}", cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Kaiba API ready - listening on {}", addr);
    axum::serve(listener, router).await?;
    Ok(())
}
