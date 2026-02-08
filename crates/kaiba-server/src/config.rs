use anyhow::{Context, Result};

/// Server configuration loaded from environment variables.
///
/// Required: `DATABASE_URL`
/// Optional: All others (services degrade gracefully when missing)
pub struct Config {
    // Required
    pub database_url: String,
    pub port: u16,

    // Authentication
    pub kaiba_api_key: Option<String>,

    // MemoryKai (Qdrant)
    pub qdrant_url: Option<String>,
    pub qdrant_api_key: Option<String>,

    // Embedding (OpenAI)
    pub openai_api_key: Option<String>,

    // WebSearch (Gemini)
    pub gemini_api_key: Option<String>,

    // GraphKai (Neo4j)
    pub neo4j_uri: Option<String>,
    pub neo4j_user: Option<String>,
    pub neo4j_password: Option<String>,

    // Decision Engine (Groq)
    pub groq_api_key: Option<String>,

    // Scheduler
    pub learning_interval_secs: Option<u64>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

        let port = std::env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        Ok(Self {
            database_url,
            port,
            kaiba_api_key: opt("KAIBA_API_KEY"),
            qdrant_url: opt("QDRANT_URL"),
            qdrant_api_key: opt("QDRANT_API_KEY"),
            openai_api_key: opt("OPENAI_API_KEY"),
            gemini_api_key: opt("GEMINI_API_KEY"),
            neo4j_uri: opt("NEO4J_URI"),
            neo4j_user: opt("NEO4J_USER"),
            neo4j_password: opt("NEO4J_PASSWORD"),
            groq_api_key: opt("GROQ_API_KEY"),
            learning_interval_secs: opt("LEARNING_INTERVAL_SECS").and_then(|s| s.parse().ok()),
        })
    }

    /// Neo4j credentials as a tuple (all three required)
    pub fn neo4j_credentials(&self) -> Option<(&str, &str, &str)> {
        match (&self.neo4j_uri, &self.neo4j_user, &self.neo4j_password) {
            (Some(uri), Some(user), Some(pass)) => Some((uri, user, pass)),
            _ => None,
        }
    }
}

fn opt(key: &str) -> Option<String> {
    std::env::var(key).ok()
}
