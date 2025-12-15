# Kaiba API Design

## Overview

```
/kaiba
├── /rei/{id}           # Rei (霊) - Persona Identity
├── /tei/{id}           # Tei (体) - Execution Interface + Expertise
└── /rei/{id}/call      # LLM Invocation
```

## Core Concepts

### Rei (霊) - Persistent Identity
- Persona metadata (name, role, avatar)
- State (energy, mood, tokens)
- Long-term memories (Qdrant)

### Tei (体) - Execution Interface
- LLM provider/model configuration
- **Expertise** (via `llm-toolkit::agent::expertise`)
  - WeightedFragments with Priority levels
  - Context-aware activation (TaskHealth)
- Dynamic model selection based on Rei's energy level

## Endpoints

### Rei Management

```
GET    /kaiba/rei              # List all Reis
POST   /kaiba/rei              # Create new Rei
GET    /kaiba/rei/{id}         # Get Rei details
PUT    /kaiba/rei/{id}         # Update Rei
DELETE /kaiba/rei/{id}         # Delete Rei
```

#### Request/Response

```rust
// POST /kaiba/rei
CreateReiRequest {
    name: String,
    role: String,
    avatar_url: Option<String>,
    manifest: serde_json::Value,  // traits, constraints
}

// GET /kaiba/rei/{id}
ReiResponse {
    id: Uuid,
    name: String,
    role: String,
    avatar_url: Option<String>,
    manifest: serde_json::Value,
    state: ReiState,
    created_at: DateTime<Utc>,
}

ReiState {
    energy_level: i32,       // 0-100
    mood: String,
    token_budget: i32,
    tokens_used: i32,
    last_active_at: Option<DateTime<Utc>>,
}
```

### Rei State

```
GET    /kaiba/rei/{id}/state   # Get current state
PUT    /kaiba/rei/{id}/state   # Update state
```

### Rei Memories (Qdrant)

```
GET    /kaiba/rei/{id}/memories         # Search memories
POST   /kaiba/rei/{id}/memories         # Add memory
DELETE /kaiba/rei/{id}/memories/{mid}   # Delete memory
```

```rust
// POST /kaiba/rei/{id}/memories
CreateMemoryRequest {
    content: String,
    memory_type: MemoryType,  // Conversation, Learning, Fact, Expertise
    importance: f32,          // 0.0 - 1.0
}

// GET /kaiba/rei/{id}/memories?query=...&limit=10
SearchMemoriesRequest {
    query: String,
    limit: Option<usize>,
    memory_type: Option<MemoryType>,
}

MemoryResponse {
    id: String,
    content: String,
    memory_type: MemoryType,
    importance: f32,
    similarity: f32,  // from vector search
    created_at: DateTime<Utc>,
}
```

---

### Tei Management

```
GET    /kaiba/tei              # List all Teis
POST   /kaiba/tei              # Create new Tei
GET    /kaiba/tei/{id}         # Get Tei details
PUT    /kaiba/tei/{id}         # Update Tei
DELETE /kaiba/tei/{id}         # Delete Tei
```

#### Request/Response

```rust
// POST /kaiba/tei
CreateTeiRequest {
    name: String,
    provider: Provider,           // Anthropic, OpenAI, Google
    model_id: String,             // claude-3-5-sonnet, gpt-4, etc.
    is_fallback: bool,            // Use when energy is low
    priority: i32,                // Selection priority (0 = highest)
    config: serde_json::Value,    // temperature, max_tokens, etc.
    expertise: Option<Expertise>, // llm-toolkit Expertise
}

// GET /kaiba/tei/{id}
TeiResponse {
    id: Uuid,
    name: String,
    provider: Provider,
    model_id: String,
    is_fallback: bool,
    priority: i32,
    config: serde_json::Value,
    expertise: Option<Expertise>,
    created_at: DateTime<Utc>,
}

enum Provider {
    Anthropic,
    OpenAI,
    Google,
}
```

### Tei Expertise

```
GET    /kaiba/tei/{id}/expertise   # Get Expertise
PUT    /kaiba/tei/{id}/expertise   # Update Expertise
```

```rust
// PUT /kaiba/tei/{id}/expertise
// Uses llm-toolkit::agent::expertise::Expertise directly
UpdateExpertiseRequest {
    expertise: Expertise,
}
```

---

### LLM Invocation

```
POST   /kaiba/rei/{reiId}/call     # Call LLM with Rei context
```

```rust
// POST /kaiba/rei/{reiId}/call
CallRequest {
    tei_ids: Vec<Uuid>,           // Teis to use (expertise combined)
    message: String,              // User message
    context: Option<CallContext>, // Additional context
}

CallContext {
    task_type: Option<String>,    // debug, review, etc.
    task_health: Option<TaskHealth>, // OnTrack, AtRisk, OffTrack
    include_memories: bool,       // Include relevant memories
    memory_limit: Option<usize>,  // Max memories to include
}

CallResponse {
    response: String,             // LLM response
    tei_used: Uuid,               // Which Tei was actually used
    tokens_consumed: i32,
    memories_included: Vec<MemoryReference>,
}

MemoryReference {
    id: String,
    similarity: f32,
}
```

## Tei Selection Logic

When calling with multiple `tei_ids`:

```rust
fn select_tei(rei: &Rei, tei_ids: &[Uuid], teis: &[Tei]) -> Tei {
    let energy = rei.state.energy_level;

    // Filter to requested Teis
    let available: Vec<_> = teis.iter()
        .filter(|t| tei_ids.contains(&t.id))
        .collect();

    // Energy-based selection
    if energy < 20 {
        // Tired mode: use fallback
        available.iter()
            .find(|t| t.is_fallback)
            .or_else(|| available.iter().max_by_key(|t| t.priority))
    } else if energy < 50 {
        // Low energy: use mid-tier
        available.iter()
            .filter(|t| t.priority >= 1)
            .min_by_key(|t| t.priority)
    } else {
        // Full energy: use best
        available.iter()
            .min_by_key(|t| t.priority)
    }
}
```

## Expertise Combination

When multiple Teis are specified, their Expertises are combined:

```rust
fn combine_expertise(teis: &[Tei]) -> Expertise {
    let mut combined = Expertise::new("combined", "1.0");

    for tei in teis {
        if let Some(exp) = &tei.expertise {
            for fragment in exp.fragments() {
                combined = combined.with_fragment(fragment.clone());
            }
        }
    }

    combined
}
```

## Call Flow

```
1. Receive CallRequest
   ↓
2. Load Rei + State
   ↓
3. Load requested Teis
   ↓
4. Select Tei based on Rei's energy
   ↓
5. Combine Expertise from Teis
   ↓
6. (Optional) Search relevant memories from Qdrant
   ↓
7. Build prompt with:
   - Rei's identity (manifest)
   - Combined Expertise (context-aware rendering)
   - Relevant memories
   - User message
   ↓
8. Call LLM via llm-toolkit
   ↓
9. Update Rei state (consume tokens, update last_active)
   ↓
10. Return response
```

## Database Schema

```sql
-- Rei (personas)
CREATE TABLE reis (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    avatar_url TEXT,
    manifest JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Rei State
CREATE TABLE rei_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rei_id UUID NOT NULL REFERENCES reis(id) ON DELETE CASCADE,
    token_budget INTEGER DEFAULT 100000,
    tokens_used INTEGER DEFAULT 0,
    energy_level INTEGER DEFAULT 100,
    mood TEXT DEFAULT 'neutral',
    last_active_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(rei_id)
);

-- Tei
CREATE TABLE teis (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    provider TEXT NOT NULL,
    model_id TEXT NOT NULL,
    is_fallback BOOLEAN DEFAULT false,
    priority INTEGER DEFAULT 0,
    config JSONB DEFAULT '{}',
    expertise JSONB,  -- Serialized Expertise
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Rei-Tei Association (many-to-many)
CREATE TABLE rei_teis (
    rei_id UUID NOT NULL REFERENCES reis(id) ON DELETE CASCADE,
    tei_id UUID NOT NULL REFERENCES teis(id) ON DELETE CASCADE,
    PRIMARY KEY (rei_id, tei_id)
);

-- Call Logs
CREATE TABLE call_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rei_id UUID NOT NULL REFERENCES reis(id),
    tei_id UUID NOT NULL REFERENCES teis(id),
    message TEXT NOT NULL,
    response TEXT NOT NULL,
    tokens_consumed INTEGER,
    context JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

## Integration with llm-toolkit

```toml
[dependencies]
llm-toolkit = { version = "0.58", features = ["agent"] }
```

```rust
use llm_toolkit::agent::expertise::{
    Expertise, WeightedFragment, KnowledgeFragment,
    Priority, ContextProfile, TaskHealth,
};
use llm_toolkit::context::RenderContext;

// Create expertise for a code reviewer Tei
let expertise = Expertise::new("rust-reviewer", "1.0")
    .with_tag("lang:rust")
    .with_fragment(
        WeightedFragment::new(KnowledgeFragment::Text(
            "Always run cargo check before reviewing".to_string()
        ))
        .with_priority(Priority::Critical)
    );

// Render with context
let context = RenderContext::new()
    .with_task_health(TaskHealth::AtRisk);
let prompt = expertise.to_prompt_with_render_context(&context);
```
