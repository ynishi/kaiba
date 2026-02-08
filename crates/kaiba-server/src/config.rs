use anyhow::{Context, Result};
use secrecy::{ExposeSecret, Secret};

/// Server configuration loaded from environment variables.
///
/// Required: `DATABASE_URL`
/// Optional: All others (services degrade gracefully when missing)
pub struct Config {
    // Required
    pub database_url: Secret<String>,
    pub port: u16,

    // Authentication
    pub kaiba_api_key: Option<Secret<String>>,

    // MemoryKai (Qdrant)
    pub qdrant_url: Option<String>,
    pub qdrant_api_key: Option<Secret<String>>,

    // Embedding (OpenAI)
    pub openai_api_key: Option<Secret<String>>,

    // WebSearch (Gemini)
    pub gemini_api_key: Option<Secret<String>>,

    // GraphKai (Neo4j)
    pub neo4j_uri: Option<String>,
    pub neo4j_user: Option<String>,
    pub neo4j_password: Option<Secret<String>>,

    // Decision Engine (Groq)
    pub groq_api_key: Option<Secret<String>>,

    // Decision Engine (Ollama) - local LLM fallback
    pub ollama_url: Option<String>,
    pub ollama_model: Option<String>,

    // Decision Engine - persona for LLM context
    pub decision_persona: Option<String>,

    // Scheduler
    pub learning_interval_secs: Option<u64>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

        let port = match std::env::var("PORT") {
            Ok(s) => s.parse().unwrap_or_else(|_| {
                tracing::warn!("Invalid PORT value '{}', using default 8080", s);
                8080
            }),
            Err(_) => 8080,
        };

        Ok(Self {
            database_url: Secret::new(database_url),
            port,
            kaiba_api_key: opt_secret("KAIBA_API_KEY"),
            qdrant_url: opt("QDRANT_URL"),
            qdrant_api_key: opt_secret("QDRANT_API_KEY"),
            openai_api_key: opt_secret("OPENAI_API_KEY"),
            gemini_api_key: opt_secret("GEMINI_API_KEY"),
            neo4j_uri: opt("NEO4J_URI"),
            neo4j_user: opt("NEO4J_USER"),
            neo4j_password: opt_secret("NEO4J_PASSWORD"),
            groq_api_key: opt_secret("GROQ_API_KEY"),
            ollama_url: opt("OLLAMA_URL"),
            ollama_model: opt("OLLAMA_MODEL"),
            decision_persona: opt("DECISION_PERSONA"),
            learning_interval_secs: opt("LEARNING_INTERVAL_SECS").and_then(|s| {
                s.parse()
                    .map_err(|_| {
                        tracing::warn!("Invalid LEARNING_INTERVAL_SECS value '{}', ignoring", s);
                    })
                    .ok()
            }),
        })
    }

    /// Neo4j credentials as a tuple (all three required).
    /// Password is intentionally exposed here for driver initialization.
    pub fn neo4j_credentials(&self) -> Option<(&str, &str, &str)> {
        match (&self.neo4j_uri, &self.neo4j_user, &self.neo4j_password) {
            (Some(uri), Some(user), Some(pass)) => Some((uri, user, pass.expose_secret())),
            _ => None,
        }
    }
}

fn opt(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn opt_secret(key: &str) -> Option<Secret<String>> {
    std::env::var(key).ok().map(Secret::new)
}
