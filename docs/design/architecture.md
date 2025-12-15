# Kaiba（海馬）Architecture

> FULL RUST, API-First, Minimalist

## Core Concepts

| Term | Kanji | Role |
|:-----|:------|:-----|
| **Kaiba** | 海馬 | Memory formation center - the system |
| **Rei** | 霊 | Persistent identity, memory, expertise |
| **Tei** | 体 | LLM model interface, execution body |

## System Architecture

```
┌─────────────────────────────────────────────────┐
│                   Clients                        │
├──────────────────┬──────────────────────────────┤
│   kaiba-cli      │   External Services          │
│   (Auth & Ops)   │   (Claude Code, etc.)        │
└────────┬─────────┴──────────────┬───────────────┘
         │                        │
         ▼                        ▼
┌─────────────────────────────────────────────────┐
│              Kaiba API (Axum)                   │
│  ┌───────────┬───────────┬───────────────────┐  │
│  │  /health  │ /personas │ /personas/:id/... │  │
│  └───────────┴───────────┴───────────────────┘  │
└────────┬────────────────────────┬───────────────┘
         │                        │
         ▼                        ▼
┌─────────────────┐    ┌─────────────────────────┐
│ Shuttle Postgres│    │     Qdrant Cloud        │
│                 │    │                         │
│ - personas      │    │ - {persona}_memories    │
│ - persona_states│    │   (vector collections)  │
│ - persona_teis  │    │                         │
│ - persona_logs  │    │                         │
└─────────────────┘    └─────────────────────────┘
```

## Crate Structure

```
kaiba/
├── crates/
│   ├── kaiba/              # API Server (Shuttle)
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── models/     # Rei, Tei, Memory
│   │   │   ├── routes/     # API handlers
│   │   │   └── services/   # Qdrant, LLM integrations
│   │   └── migrations/
│   │
│   └── kaiba-cli/          # CLI Tool
│       └── src/
│           ├── main.rs
│           └── commands/   # auth, persona, memory
│
├── docs/design/
└── Cargo.toml              # Workspace
```

## Data Models

### Rei (霊) - Persistent Identity

```rust
/// Core persona identity
pub struct Persona {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub manifest: serde_json::Value,  // traits, constraints, expertise
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Current state
pub struct PersonaState {
    pub persona_id: Uuid,
    pub token_budget: i32,
    pub tokens_used: i32,
    pub energy_level: i32,  // 0-100
    pub mood: String,
    pub last_active_at: Option<DateTime<Utc>>,
}

/// Long-term memory (stored in Qdrant)
pub struct Memory {
    pub id: String,
    pub persona_id: String,
    pub content: String,
    pub memory_type: MemoryType,  // Conversation, Learning, Fact, Expertise
    pub importance: f32,
    pub created_at: DateTime<Utc>,
}

pub enum MemoryType {
    Conversation,
    Learning,
    Fact,
    Expertise,  // Skills, knowledge domains
}
```

### Tei (体) - Execution Interface

```rust
/// LLM model configuration
pub struct PersonaTei {
    pub id: Uuid,
    pub persona_id: Uuid,
    pub provider: Provider,
    pub model_id: String,
    pub is_fallback: bool,
    pub priority: i32,
    pub config: serde_json::Value,
}

pub enum Provider {
    Anthropic,  // Claude
    OpenAI,     // GPT-4
    Google,     // Gemini
}
```

## API Endpoints

### Public (No Auth)
```
GET  /health                     # Health check
GET  /personas/{id}              # Get persona public info
```

### Authenticated
```
POST /personas                   # Create persona
PUT  /personas/{id}              # Update persona
DELETE /personas/{id}            # Delete persona

GET  /personas/{id}/state        # Get current state
PUT  /personas/{id}/state        # Update state

POST /personas/{id}/memories     # Add memory
GET  /personas/{id}/memories     # Search memories
DELETE /personas/{id}/memories/{mid}  # Delete memory

GET  /personas/{id}/teis         # List tei configs
POST /personas/{id}/teis         # Add tei config
PUT  /personas/{id}/teis/{tid}   # Update tei config
```

## kaiba-cli

```bash
# Authentication
kaiba auth login
kaiba auth logout
kaiba auth status

# Persona management
kaiba persona list
kaiba persona create --name "Yui" --role "Engineer"
kaiba persona show <id>
kaiba persona delete <id>

# Memory operations
kaiba memory add <persona_id> "learned about Rust async patterns"
kaiba memory search <persona_id> "async patterns"
kaiba memory list <persona_id> --type expertise

# Tei configuration
kaiba tei add <persona_id> --provider anthropic --model claude-3-5-sonnet
kaiba tei list <persona_id>
```

## Implementation Phases

### Phase 1: Foundation (Current)
- [x] Basic API structure (Axum + Shuttle)
- [x] Postgres integration (personas, persona_states)
- [x] Qdrant integration (memory storage)
- [ ] Authentication (JWT or API key)

### Phase 2: CLI
- [ ] kaiba-cli crate setup
- [ ] Auth commands
- [ ] Persona CRUD commands
- [ ] Memory commands

### Phase 3: Tei Integration
- [ ] LLM provider abstraction
- [ ] Dynamic model selection based on energy
- [ ] Token consumption tracking

### Phase 4: Autonomy
- [ ] Heartbeat loop (background task)
- [ ] Context gathering
- [ ] Decision engine
- [ ] Logging & notifications

## Design Principles

1. **API-First**: Everything through REST API
2. **Calm Tech**: Silent operations, opt-in notifications
3. **Expertise Focus**: Tei holds specialized knowledge/skills
4. **Energy Model**: Resource constraints create personality
5. **FULL RUST**: No JavaScript, no external frameworks

## Tech Stack

| Layer | Technology |
|:------|:-----------|
| API | Axum |
| Deploy | Shuttle |
| DB | PostgreSQL (Shuttle) |
| Vector | Qdrant Cloud |
| CLI | clap |
| Auth | JWT / API Key |
| Serialization | serde |
