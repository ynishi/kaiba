# Kaiba (海馬) - Autonomous Persona Architecture

[![Crates.io](https://img.shields.io/crates/v/kaiba-cli.svg)](https://crates.io/crates/kaiba-cli)

> "Memories flow through the hippocampus" - A headless API service for persistent AI personas

## Overview

**Kaiba** (海馬 - hippocampus) is a minimalist, API-first implementation of autonomous AI personas. Inspired by neuroscience, it separates the persistent **Rei** (霊 - spirit/identity) from the ephemeral **Tei** (体 - body/model), creating personas that accumulate knowledge and maintain continuity across platforms.

### Core Concepts

| Term | Kanji | Meaning |
|:-----|:------|:--------|
| **Kaiba** | 海馬 | Hippocampus - memory formation center |
| **Rei** | 霊 | Spirit - persistent identity, memory, state |
| **Tei** | 体 | Body - LLM model, execution interface |

### Core Principles

- **API-First**: Pure REST API, no UI required
- **Memory Ocean**: Distributed vector memory via Qdrant
- **Persona Protocol**: Gravatar-style simple persona fetching (`GET /personas/{id}`)
- **Rei/Tei Separation**: Identity persists, models are swappable
- **Calm Technology**: Silent operations, selective notifications

## Architecture

```
┌─────────────────┐
│  Shuttle Rust   │  Kaiba API (Axum)
│     Backend     │
└────────┬────────┘
         │
         ├─────► Qdrant Cloud (Memory Ocean)
         │        - mai_memories
         │        - yui_memories
         │        - ...
         │
         └─────► Shuttle Postgres (Metadata)
                  - personas (Rei)
                  - persona_states
```

## Project Structure

```
kaiba/
├── crates/
│   └── kaiba/              # Main Shuttle API
│       ├── src/
│       │   ├── main.rs
│       │   ├── models/     # Persona, Memory, State
│       │   ├── routes/     # API endpoints
│       │   └── services/   # Qdrant integration
│       ├── Cargo.toml
│       └── Shuttle.toml
├── docs/
│   └── design/             # Design documents
│       ├── concept.md      # Original concept
│       └── rei-design.md   # Detailed design
└── Cargo.toml              # Workspace config
```

## API Endpoints

### Health Check
```bash
GET /health
```

### Persona Management

#### Get Persona (Public - Gravatar Style)
```bash
GET /personas/{id}

Response:
{
  "base": {
    "name": "Yui",
    "role": "Principal Engineer",
    "avatar_url": "https://...",
    "constraints": ["code_quality", "scalability"],
    "voice_settings": {
      "tone": "professional",
      "quirk": "technical_deep_dives"
    }
  },
  "status": {
    "energy_level": 85,
    "mood": "focused",
    "last_active": "2024-12-15T18:00:00Z"
  }
}
```

### Memory Management

#### Add Memory
```bash
POST /personas/{id}/memories
Content-Type: application/json

{
  "content": "Rust async/await pattern insights",
  "memory_type": "learning",
  "importance": 0.8
}
```

#### Search Memories
```bash
POST /personas/{id}/memories/search
Content-Type: application/json

{
  "query": "Rust async patterns",
  "limit": 5
}
```

## Setup

### Prerequisites

- Rust 1.75+ (via rustup)
- Shuttle CLI: `cargo install cargo-shuttle`
- Qdrant Cloud account (free tier available)

### Local Development

1. **Clone the repository**
   ```bash
   git clone <repo-url>
   cd kaiba
   ```

2. **Install dependencies**
   ```bash
   cargo build
   ```

3. **Configure Qdrant**
   - Create a free Qdrant Cloud cluster at https://cloud.qdrant.io
   - Get your API URL and API Key

4. **Set Shuttle secrets** (for deployment)
   ```bash
   cd crates/kaiba
   shuttle secrets add QDRANT_URL="https://your-qdrant-url"
   shuttle secrets add QDRANT_API_KEY="your-api-key"
   ```

5. **Run locally**
   ```bash
   cd crates/kaiba
   shuttle run
   ```

   API will be available at `http://localhost:8000`

### Deployment

```bash
cd crates/kaiba
shuttle deploy
```

Your API will be live at `https://kaiba.shuttleapp.rs` (or your custom domain)

## Development

### Check compilation
```bash
cargo check
```

### Run tests
```bash
cargo test
```

### Format code
```bash
cargo fmt
```

## Design Philosophy

### Rei/Tei Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Rei (霊/Spirit)          │  Tei (体/Body)              │
├─────────────────────────────────────────────────────────┤
│  ・Identity (name, role) │  ・Interchangeable           │
│  ・Personality           │  ・Could be Claude/GPT/etc   │
│  ・Memories (MemoryKai)  │  ・Or SubAgent               │
│  ・Energy/State          │  ・Or User's own LLM         │
│  ・Interests             │  ・Or even human(!?)         │
└─────────────────────────────────────────────────────────┘
          ↓
    Prompt Generation (with RAG)
          ↓
    Execution anywhere ← The key insight
```

The "Ghost" (Rei) is completely decoupled from the "Shell" (Tei). Your persona's
identity, memories, and state persist regardless of which LLM—or even which
platform—executes them. This enables true portability and continuity.

Like the hippocampus forms and consolidates memories, Kaiba allows AI personas to:
- Maintain persistent identity and accumulated knowledge (Rei)
- Switch between different LLM models based on resource constraints (Tei)
- Exhibit "fatigue" when token budgets are low (selecting cheaper models)
- Accumulate specialized knowledge over time through curated memory

### Calm Technology
Following Mark Weiser's principles:
- APIs don't demand attention - they simply exist and respond
- Personas can work silently, logging actions without notifications
- Information is available when needed, not pushed aggressively

### Unix Philosophy
- Do one thing well: Provide persona state and memory access
- Simple interfaces: REST API with predictable endpoints
- Composability: Personas can be integrated into any platform

## Using Claude Code as Tei

Kaiba doesn't wrap LLMs—it generates prompts that any execution environment can use.
This means you can use **Claude Code** (or any LLM CLI) as your Tei.

### Prompt Generation Endpoint

```bash
GET /kaiba/rei/{id}/prompt?format={format}
```

**Parameters:**
- `format` - Output format: `casting`, `claude-code`, `raw` (default: `raw`)
- `include_memories` - Include RAG memories (default: `true`)
- `memory_limit` - Max memories to include (default: `5`)
- `context` - Query for memory search (default: Rei's name)

**Response:**
```json
{
  "system_prompt": "You are しーちゃん, Senior Coding Assistant...",
  "format": "claude-code",
  "rei": {
    "id": "cd4efdf2-...",
    "name": "しーちゃん",
    "role": "Senior Coding Assistant",
    "energy_level": 85,
    "mood": "focused"
  },
  "memories_included": 5
}
```

### Example: Claude Code with Kaiba Persona

```bash
# Fetch Rei's prompt and pipe to Claude Code
claude --system-prompt "$(
  curl -s -H "Authorization: Bearer $KAIBA_API_KEY" \
    "$KAIBA_URL/kaiba/rei/$REI_ID/prompt?format=claude-code" \
  | jq -r '.system_prompt'
)"
```

Or as a shell function:

```bash
# Add to your .zshrc / .bashrc
kaiba-claude() {
  local rei_id="${1:-$KAIBA_DEFAULT_REI}"
  claude --system-prompt "$(
    curl -s -H "Authorization: Bearer $KAIBA_API_KEY" \
      "$KAIBA_URL/kaiba/rei/$rei_id/prompt?format=claude-code" \
    | jq -r '.system_prompt'
  )"
}

# Usage
kaiba-claude cd4efdf2-be22-41ec-9238-227f5ccb1523
```

This pattern keeps Kaiba focused on **identity and memory**, while letting you choose
any LLM or execution environment as the Tei.

## Roadmap

- [x] Basic API structure (Axum + Shuttle)
- [x] Qdrant integration (MemoryKai)
- [x] Shuttle Postgres integration
- [x] Authentication (API Key)
- [x] RAG integration for LLM calls
- [x] WebSearch (Gemini grounded search)
- [x] Autonomous learning from interests
- [x] Decision system (Learn/Digest/Rest)
- [x] Energy regeneration
- [x] Prompt endpoint for external Tei (Claude Code, Casting, etc.)
- [ ] Web UI (optional, later)

## Contributing

This is currently a personal/experimental project. Feel free to fork and experiment!

## License

MIT

---

**Built with**
🦀 Rust | 🚀 Shuttle | 🌊 Qdrant | 🧠 Kaiba
