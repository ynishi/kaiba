//! Decision Service - Autonomous action selection
//!
//! Decides what action a Rei should take based on their state.
//! Supports both rule-based and LLM-based decision engines.

use crate::models::ReiState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Actions a Rei can take during autonomous cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Search and learn new information
    Learn,
    /// Consolidate and summarize recent memories
    Digest,
    /// Do nothing, recover energy
    Rest,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Learn => write!(f, "🔍 Learn"),
            Action::Digest => write!(f, "📝 Digest"),
            Action::Rest => write!(f, "😴 Rest"),
        }
    }
}

/// Decision context - all factors considered in decision
#[derive(Debug, Clone, Serialize)]
pub struct DecisionContext {
    pub energy_level: i32,
    pub tokens_remaining: i32,
    pub mood: String,
    pub memories_since_digest: usize,
}

/// Decision result with reasoning
#[derive(Debug, Clone, Serialize)]
pub struct Decision {
    pub action: Action,
    pub reason: String,
    pub context: DecisionContext,
}

// ============================================
// Decision Engine Trait
// ============================================

/// Trait for decision engines (rule-based or LLM-based)
#[async_trait]
pub trait DecisionEngine: Send + Sync {
    /// Decide what action to take based on state
    async fn decide(&self, state: &ReiState, memories_since_digest: usize) -> Decision;

    /// Engine name for logging
    fn name(&self) -> &'static str;
}

// ============================================
// Rule-Based Decision Engine
// ============================================

/// Thresholds for rule-based decision making
#[derive(Debug, Clone)]
pub struct RuleBasedConfig {
    /// Minimum energy to learn
    pub min_energy_learn: i32,
    /// Minimum energy to digest
    pub min_energy_digest: i32,
    /// Minimum tokens remaining to take action
    pub min_tokens_action: i32,
    /// Memories needed before digest is considered
    pub memories_for_digest: usize,
}

impl Default for RuleBasedConfig {
    fn default() -> Self {
        Self {
            min_energy_learn: 50,
            min_energy_digest: 60,
            min_tokens_action: 500,
            memories_for_digest: 5,
        }
    }
}

/// Rule-based decision engine (fast, deterministic)
pub struct RuleBasedEngine {
    config: RuleBasedConfig,
}

impl RuleBasedEngine {
    pub fn new(config: Option<RuleBasedConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
        }
    }
}

#[async_trait]
impl DecisionEngine for RuleBasedEngine {
    async fn decide(&self, state: &ReiState, memories_since_digest: usize) -> Decision {
        let tokens_remaining = state.token_budget - state.tokens_used;

        let context = DecisionContext {
            energy_level: state.energy_level,
            tokens_remaining,
            mood: state.mood.clone(),
            memories_since_digest,
        };

        // Priority 1: Token exhaustion -> Rest
        if tokens_remaining < self.config.min_tokens_action {
            return Decision {
                action: Action::Rest,
                reason: format!(
                    "Token budget low ({} remaining, need {})",
                    tokens_remaining, self.config.min_tokens_action
                ),
                context,
            };
        }

        // Priority 2: Low energy -> Rest
        if state.energy_level < self.config.min_energy_learn {
            return Decision {
                action: Action::Rest,
                reason: format!(
                    "Energy low ({}, need {} to learn)",
                    state.energy_level, self.config.min_energy_learn
                ),
                context,
            };
        }

        // Priority 3: Many undigested memories + enough energy -> Digest
        if memories_since_digest >= self.config.memories_for_digest
            && state.energy_level >= self.config.min_energy_digest
        {
            return Decision {
                action: Action::Digest,
                reason: format!(
                    "{} memories to consolidate, energy sufficient ({})",
                    memories_since_digest, state.energy_level
                ),
                context,
            };
        }

        // Priority 4: Enough energy -> Learn
        if state.energy_level >= self.config.min_energy_learn {
            return Decision {
                action: Action::Learn,
                reason: format!("Energy sufficient ({}) for learning", state.energy_level),
                context,
            };
        }

        // Default: Rest
        Decision {
            action: Action::Rest,
            reason: "Default to rest".to_string(),
            context,
        }
    }

    fn name(&self) -> &'static str {
        "RuleBased"
    }
}

// ============================================
// LLM-Based Decision Engine (Groq/Ollama)
// ============================================

/// Configuration for LLM-based decision engine
#[derive(Debug, Clone)]
pub struct LlmEngineConfig {
    /// API endpoint (Groq: https://api.groq.com/openai/v1)
    pub base_url: String,
    /// API key
    pub api_key: String,
    /// Model name (e.g., "llama-3.2-3b-preview", "llama-3.1-8b-instant")
    pub model: String,
    /// Rei's persona for decision context
    pub persona: Option<String>,
}

impl LlmEngineConfig {
    /// Create config for Groq
    pub fn groq(api_key: &str, model: &str) -> Self {
        Self {
            base_url: "https://api.groq.com/openai/v1".to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            persona: None,
        }
    }

    /// Create config for local Ollama
    pub fn ollama(model: &str) -> Self {
        Self {
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: "ollama".to_string(), // Ollama doesn't need real key
            model: model.to_string(),
            persona: None,
        }
    }

    pub fn with_persona(mut self, persona: &str) -> Self {
        self.persona = Some(persona.to_string());
        self
    }
}

/// LLM-based decision engine using OpenAI-compatible API
pub struct LlmEngine {
    config: LlmEngineConfig,
    client: reqwest::Client,
    /// Fallback to rule-based on API failure
    fallback: RuleBasedEngine,
}

impl LlmEngine {
    pub fn new(config: LlmEngineConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            fallback: RuleBasedEngine::new(None),
        }
    }

    fn build_prompt(&self, context: &DecisionContext) -> String {
        let persona_intro = self
            .config
            .persona
            .as_ref()
            .map(|p| format!("You are {}.\n\n", p))
            .unwrap_or_default();

        format!(
            r#"{persona_intro}You are deciding what action to take based on your current state.

Current State:
- Energy Level: {energy}% (0-100)
- Tokens Remaining: {tokens}
- Mood: {mood}
- Unprocessed Memories: {memories}

Available Actions:
1. **learn** - Search and learn new information (requires energy >= 50)
2. **digest** - Consolidate and summarize recent memories (requires energy >= 60, best when memories >= 5)
3. **rest** - Do nothing, recover energy (always available)

Respond with ONLY a JSON object in this exact format:
{{"action": "learn|digest|rest", "reason": "brief explanation in character"}}

Consider your energy, mood, and whether you have memories to process. Be true to your personality."#,
            persona_intro = persona_intro,
            energy = context.energy_level,
            tokens = context.tokens_remaining,
            mood = context.mood,
            memories = context.memories_since_digest,
        )
    }

    async fn call_llm(&self, prompt: &str) -> Result<(Action, String), String> {
        let request_body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.7,
            "max_tokens": 150,
        });

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API error ({}): {}", status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {}", e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("Missing content in response")?;

        // Parse the JSON response
        self.parse_llm_response(content)
    }

    fn parse_llm_response(&self, content: &str) -> Result<(Action, String), String> {
        // Try to extract JSON from response (LLM might add extra text)
        let json_start = content.find('{').ok_or("No JSON found in response")?;
        let json_end = content.rfind('}').ok_or("No JSON end found")? + 1;
        let json_str = &content[json_start..json_end];

        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("JSON parse error: {} in '{}'", e, json_str))?;

        let action_str = parsed["action"].as_str().ok_or("Missing 'action' field")?;

        let action = match action_str.to_lowercase().as_str() {
            "learn" => Action::Learn,
            "digest" => Action::Digest,
            "rest" => Action::Rest,
            other => return Err(format!("Unknown action: {}", other)),
        };

        let reason = parsed["reason"]
            .as_str()
            .unwrap_or("No reason provided")
            .to_string();

        Ok((action, reason))
    }
}

#[async_trait]
impl DecisionEngine for LlmEngine {
    async fn decide(&self, state: &ReiState, memories_since_digest: usize) -> Decision {
        let tokens_remaining = state.token_budget - state.tokens_used;

        let context = DecisionContext {
            energy_level: state.energy_level,
            tokens_remaining,
            mood: state.mood.clone(),
            memories_since_digest,
        };

        // Build prompt and call LLM
        let prompt = self.build_prompt(&context);

        match self.call_llm(&prompt).await {
            Ok((action, reason)) => {
                tracing::debug!(
                    "LLM decision: {:?} - {} (model: {})",
                    action,
                    reason,
                    self.config.model
                );
                Decision {
                    action,
                    reason,
                    context,
                }
            }
            Err(e) => {
                tracing::warn!("LLM decision failed, falling back to rule-based: {}", e);
                self.fallback.decide(state, memories_since_digest).await
            }
        }
    }

    fn name(&self) -> &'static str {
        "LLM"
    }
}

// ============================================
// Factory
// ============================================

/// Create decision engine based on configuration
pub fn create_decision_engine(llm_config: Option<LlmEngineConfig>) -> Box<dyn DecisionEngine> {
    let engine: Box<dyn DecisionEngine> = match llm_config {
        Some(config) => {
            tracing::info!(
                "Decision engine: {} @ {} (model: {})",
                "LLM",
                config.base_url,
                config.model
            );
            Box::new(LlmEngine::new(config))
        }
        None => Box::new(RuleBasedEngine::new(None)),
    };
    tracing::info!("Decision engine initialized: {}", engine.name());
    engine
}

// ============================================
// Legacy compatibility
// ============================================

/// Legacy DecisionMaker (wraps RuleBasedEngine)
pub type DecisionConfig = RuleBasedConfig;

pub struct DecisionMaker {
    engine: RuleBasedEngine,
}

impl DecisionMaker {
    pub fn new(config: Option<DecisionConfig>) -> Self {
        Self {
            engine: RuleBasedEngine::new(config),
        }
    }

    pub fn decide(&self, state: &ReiState, memories_since_digest: usize) -> Decision {
        // Blocking wrapper for legacy code
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.engine.decide(state, memories_since_digest))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn mock_state(energy: i32, tokens_used: i32) -> ReiState {
        ReiState {
            id: Uuid::new_v4(),
            rei_id: Uuid::new_v4(),
            token_budget: 100000,
            tokens_used,
            energy_level: energy,
            mood: "neutral".to_string(),
            last_active_at: Some(Utc::now()),
            updated_at: Utc::now(),
            energy_regen_per_hour: 10,
            last_digest_at: None,
            last_learn_at: None,
        }
    }

    #[tokio::test]
    async fn test_rule_based_low_energy_rests() {
        let engine = RuleBasedEngine::new(None);
        let state = mock_state(20, 0);
        let decision = engine.decide(&state, 0).await;
        assert_eq!(decision.action, Action::Rest);
    }

    #[tokio::test]
    async fn test_rule_based_high_energy_learns() {
        let engine = RuleBasedEngine::new(None);
        let state = mock_state(80, 0);
        let decision = engine.decide(&state, 0).await;
        assert_eq!(decision.action, Action::Learn);
    }

    #[tokio::test]
    async fn test_rule_based_many_memories_digests() {
        let engine = RuleBasedEngine::new(None);
        let state = mock_state(80, 0);
        let decision = engine.decide(&state, 10).await;
        assert_eq!(decision.action, Action::Digest);
    }

    #[tokio::test]
    async fn test_rule_based_token_exhausted_rests() {
        let engine = RuleBasedEngine::new(None);
        let state = mock_state(100, 99900); // Only 100 tokens left
        let decision = engine.decide(&state, 0).await;
        assert_eq!(decision.action, Action::Rest);
    }

    #[test]
    fn test_llm_response_parsing() {
        let config = LlmEngineConfig::groq("test", "test");
        let engine = LlmEngine::new(config);

        // Valid JSON
        let (action, reason) = engine
            .parse_llm_response(r#"{"action": "learn", "reason": "I'm curious today!"}"#)
            .unwrap();
        assert_eq!(action, Action::Learn);
        assert_eq!(reason, "I'm curious today!");

        // JSON with extra text
        let (action, _) = engine
            .parse_llm_response(
                r#"Here's my decision: {"action": "rest", "reason": "tired"} That's it."#,
            )
            .unwrap();
        assert_eq!(action, Action::Rest);
    }
}
