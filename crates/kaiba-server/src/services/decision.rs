//! Decision Service - Autonomous action selection
//!
//! Decides what action a Rei should take based on their state.
//! Currently rule-based, designed to be swappable with LLM-based decision.

use crate::models::ReiState;
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

/// Thresholds for decision making (configurable)
#[derive(Debug, Clone)]
pub struct DecisionConfig {
    /// Minimum energy to learn
    pub min_energy_learn: i32,
    /// Minimum energy to digest
    pub min_energy_digest: i32,
    /// Minimum tokens remaining to take action
    pub min_tokens_action: i32,
    /// Memories needed before digest is considered
    pub memories_for_digest: usize,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            min_energy_learn: 50,
            min_energy_digest: 60,
            min_tokens_action: 500,
            memories_for_digest: 5,
        }
    }
}

/// Rule-based decision maker
/// TODO: Replace with LLM-based decision (like orcs agent pattern)
pub struct DecisionMaker {
    config: DecisionConfig,
}

impl DecisionMaker {
    pub fn new(config: Option<DecisionConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
        }
    }

    /// Decide what action to take
    pub fn decide(&self, state: &ReiState, memories_since_digest: usize) -> Decision {
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
        }
    }

    #[test]
    fn test_low_energy_rests() {
        let maker = DecisionMaker::new(None);
        let state = mock_state(20, 0);
        let decision = maker.decide(&state, 0);
        assert_eq!(decision.action, Action::Rest);
    }

    #[test]
    fn test_high_energy_learns() {
        let maker = DecisionMaker::new(None);
        let state = mock_state(80, 0);
        let decision = maker.decide(&state, 0);
        assert_eq!(decision.action, Action::Learn);
    }

    #[test]
    fn test_many_memories_digests() {
        let maker = DecisionMaker::new(None);
        let state = mock_state(80, 0);
        let decision = maker.decide(&state, 10);
        assert_eq!(decision.action, Action::Digest);
    }

    #[test]
    fn test_token_exhausted_rests() {
        let maker = DecisionMaker::new(None);
        let state = mock_state(100, 99900); // Only 100 tokens left
        let decision = maker.decide(&state, 0);
        assert_eq!(decision.action, Action::Rest);
    }
}
