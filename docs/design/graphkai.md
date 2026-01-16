# GraphKai - Hybrid GraphRAG Integration

> Emphasis-Driven Semantic Linking for Dense Knowledge Retrieval

## Overview

GraphKai extends Kaiba's existing RAG (MemoryKai/Qdrant) with a Knowledge Graph layer (Neo4j), enabling **Hybrid GraphRAG**. The system interprets Markdown emphasis syntax (`**bold**`, `*italic*`) as semantic intent for automatic graph construction.

**Key Principle**: Markdown is the Source of Truth. Both Vector DB (Qdrant) and Graph DB (Neo4j) are derived data that can be rebuilt from the raw documents.

## Architecture

```
┌───────────────────────────────────────────────────────────────────────────┐
│                              Kaiba System                                  │
├───────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  ┌──────────────┐     ┌──────────────┐     ┌─────────────────────────┐   │
│  │   Markdown   │────▶│   Ingestion  │────▶│      Triple Store       │   │
│  │   Source     │     │   Pipeline   │     │                         │   │
│  └──────────────┘     └──────────────┘     │  ┌─────────────────┐   │   │
│                              │              │  │    DocStore     │   │   │
│                              │              │  │   (PostgreSQL)  │   │   │
│                              │              │  │  ← Source of    │   │   │
│                              │              │  │     Truth       │   │   │
│                              │              │  └─────────────────┘   │   │
│                              ▼              │           │            │   │
│                       ┌──────────────┐     │           ▼            │   │
│                       │   Emphasis   │     │  ┌───────────────┐    │   │
│                       │   Parser     │     │  │    Qdrant     │    │   │
│                       └──────────────┘     │  │    (RAG)      │    │   │
│                              │              │  │  ← Derived     │    │   │
│                              ▼              │  └───────────────┘    │   │
│                       ┌──────────────┐     │           │            │   │
│                       │ Graph Builder│     │           ▼            │   │
│                       │   Engine     │────▶│  ┌───────────────┐    │   │
│                       └──────────────┘     │  │    Neo4j      │    │   │
│                              ▲              │  │   (Graph)     │    │   │
│  ┌──────────────┐           │              │  │  ← Derived     │    │   │
│  │   Linkage    │───────────┘              │  └───────────────┘    │   │
│  │   Config     │  Rebuild on threshold    └─────────────────────────┘   │
│  │  (YAML/API)  │  change                            │                   │
│  └──────────────┘                                    ▼                   │
│                                              ┌──────────────┐             │
│                                              │   Hybrid     │             │
│  ┌──────────────┐                           │   Search     │             │
│  │  User Query  │──────────────────────────▶│   Router     │             │
│  └──────────────┘                           └──────────────┘             │
│                                                     │                     │
│                                                     ▼                     │
│                                              ┌──────────────┐             │
│                                              │   Response   │             │
│                                              │   (Dense)    │             │
│                                              └──────────────┘             │
└───────────────────────────────────────────────────────────────────────────┘
```

## Storage Hierarchy

| Store | Role | Rebuild | Backend |
|:------|:-----|:--------|:--------|
| **DocStore** | Raw Markdown (Source of Truth) | - | Shuttle Shared DB (PostgreSQL) |
| **Qdrant** | Chunk embeddings (Derived) | From DocStore | Qdrant Cloud (existing) |
| **Neo4j** | Knowledge Graph (Derived) | From DocStore + thresholds | Neo4j Aura Free |

## Infrastructure

```
┌─────────────────────────────────────────────────────────────────┐
│                    Shuttle (Serverless Rust)                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    kaiba-server                          │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ Shuttle Shared  │  │  Qdrant Cloud   │  │ Neo4j Aura Free │
│   DB (PG)       │  │   (existing)    │  │   (new)         │
├─────────────────┤  ├─────────────────┤  ├─────────────────┤
│ • reis          │  │ • memories      │  │ • :Concept      │
│ • rei_states    │  │   (vectors)     │  │ • :Tag          │
│ • documents ←   │  │ • chunks ←      │  │ • :Document     │
│ • teis          │  │                 │  │ • SIMILAR_TO    │
│ • ...           │  │                 │  │ • BELONGS_TO    │
└─────────────────┘  └─────────────────┘  └─────────────────┘
   Source of Truth      Derived (RAG)       Derived (Graph)
```

## Data Models

### Document (Source of Truth)

```rust
pub struct Document {
    pub id: String,
    pub rei_id: String,
    pub title: String,
    pub raw_content: String,           // Raw Markdown
    pub source_path: Option<String>,   // Original file path
    pub checksum: String,              // SHA256 for change detection
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### EmphasisNode (Extracted from Markdown)

```rust
pub struct EmphasisNode {
    pub text: String,           // Emphasized text
    pub style: EmphasisStyle,   // Bold, Italic, Code, etc.
    pub weight: f32,            // Style-based weight
    pub context: String,        // Surrounding ±50 tokens
    pub source_doc: String,     // Source document ID
    pub position: usize,        // Position in document
}

pub enum EmphasisStyle {
    Bold,           // **text** → weight: 1.0
    Italic,         // *text*   → weight: 0.7
    BoldItalic,     // ***text*** → weight: 1.2
    Code,           // `text`   → weight: 0.8
    Highlight,      // ==text== → weight: 1.1
}
```

### GraphNode & GraphEdge (Neo4j)

```rust
pub struct GraphNode {
    pub id: String,
    pub text: String,
    pub node_type: NodeType,        // Concept, Entity, Tag
    pub weight: f32,
    pub embedding: Vec<f32>,        // Contextual embedding
    pub metadata: serde_json::Value,
}

pub struct GraphEdge {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: EdgeType,        // SemanticSimilarity, CoOccurrence, TagMembership
    pub strength: f32,              // 0.0 - 1.0
}
```

## Repository Traits

### DocRepository

```rust
#[async_trait]
pub trait DocRepository: Send + Sync {
    async fn save(&self, doc: &Document) -> Result<(), DomainError>;
    async fn get(&self, doc_id: &str) -> Result<Option<Document>, DomainError>;
    async fn list_by_rei(&self, rei_id: &str) -> Result<Vec<Document>, DomainError>;
    async fn delete(&self, doc_id: &str) -> Result<(), DomainError>;
    async fn list_modified_since(
        &self,
        rei_id: &str,
        since: DateTime<Utc>
    ) -> Result<Vec<Document>, DomainError>;
}
```

### GraphRepository

```rust
#[async_trait]
pub trait GraphRepository: Send + Sync {
    // Node operations
    async fn upsert_node(&self, node: &GraphNode) -> Result<(), DomainError>;
    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>, DomainError>;
    async fn delete_node(&self, id: &str) -> Result<(), DomainError>;

    // Edge operations
    async fn create_edge(&self, edge: &GraphEdge) -> Result<(), DomainError>;
    async fn get_neighbors(&self, node_id: &str, depth: u32) -> Result<Vec<GraphNode>, DomainError>;

    // Search
    async fn traverse(&self, start: &str, query: &TraversalQuery) -> Result<Vec<GraphPath>, DomainError>;
    async fn find_by_embedding(&self, embedding: Vec<f32>, threshold: f32) -> Result<Vec<GraphNode>, DomainError>;
}
```

## API Integration

### Existing Endpoints (No Changes)

| Endpoint | Function | Purpose |
|:---------|:---------|:--------|
| `POST /kaiba/rei/{id}/call` | `call_llm()` | LLM invocation with RAG |
| `GET /kaiba/rei/{id}/prompt` | `generate_prompt()` | Prompt generation with RAG |
| `POST /kaiba/rei/{id}/memories/search` | `search_memories()` | Direct memory search |

### Integration Points (Internal)

The existing internal functions are replaced with `HybridSearchService`:

```rust
// Before (current)
async fn search_memories_for_rag(...) {
    // Qdrant search only
    let memories = memory_kai.search_memories(...).await?;
}

// After (GraphKai integration)
async fn search_memories_for_rag(...) {
    // Delegate to HybridSearchService
    let hybrid = state.hybrid_search.as_ref()?;
    let config = HybridSearchConfig {
        strategy: SearchStrategy::Auto,
        rag_limit: limit.unwrap_or(5),
        graph_depth: 2,
    };
    let memories = hybrid.search(&rei_id.to_string(), query, config).await?;
}
```

### New Endpoints

```
POST /kaiba/rei/{id}/documents           # Ingest document
GET  /kaiba/rei/{id}/documents           # List documents
GET  /kaiba/rei/{id}/documents/{doc_id}  # Get document
DELETE /kaiba/rei/{id}/documents/{doc_id} # Delete document

POST /kaiba/rei/{id}/graph/rebuild       # Rebuild graph with new thresholds
GET  /kaiba/rei/{id}/graph/nodes         # List graph nodes
GET  /kaiba/rei/{id}/graph/neighbors/{node_id}  # Get node neighbors
```

### AppState Extension

```rust
pub struct AppState {
    pub pool: PgPool,
    pub doc_store: Option<DocStore>,        // PostgreSQL (new) ← Source of Truth
    pub memory_kai: Option<MemoryKai>,      // Qdrant (existing) ← Derived
    pub graph_kai: Option<GraphKai>,        // Neo4j (new) ← Derived
    pub hybrid_search: Option<HybridSearchService>,  // Unified layer (new)
    pub embedding: Option<EmbeddingService>,
    // ...
}
```

## Hybrid Search

### Search Strategies

```rust
pub enum SearchStrategy {
    GraphFirst,     // Specific concept query → Graph traversal → RAG supplement
    RagFirst,       // Broad/fuzzy query → RAG → Graph expansion
    Parallel,       // Execute both and merge
    Auto,           // Automatically determine based on query
}
```

### Result Merging

```
┌─────────────────────────────────────────────────────────────────┐
│                    HybridSearchService                           │
│                                                                  │
│  ┌──────────────────┐     ┌──────────────────┐                 │
│  │   MemoryKai      │     │    GraphKai      │                 │
│  │   (Qdrant)       │     │    (Neo4j)       │                 │
│  └────────┬─────────┘     └────────┬─────────┘                 │
│           │                        │                            │
│           ▼                        ▼                            │
│  ┌─────────────────────────────────────────────┐               │
│  │            ResultMerger                      │               │
│  │  - Score normalization                       │               │
│  │  - Deduplication                             │               │
│  │  - Graph path → Memory conversion            │               │
│  └─────────────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────────────┘
                           │
                           ▼
                  Vec<Memory> (existing type)
```

## Linkage Configuration

```yaml
# config/graphkai.yaml
graph:
  neo4j_uri: "${NEO4J_URI}"
  neo4j_user: "${NEO4J_USER}"
  neo4j_password: "${NEO4J_PASSWORD}"

emphasis_weights:
  bold: 1.0
  italic: 0.7
  bold_italic: 1.2
  code: 0.8
  highlight: 1.1

linkage_strategy:
  similarity_threshold: 0.85
  co_occurrence_weight: 0.6
  tag_membership_weight: 1.0
  max_edges_per_node: 20
  decay_factor: 0.9  # Decay for over-emphasis in same paragraph

search:
  default_strategy: "auto"
  graph_depth: 2
  rag_top_k: 5
```

## Neo4j Schema

```cypher
// Nodes
(:Concept {id, text, weight, embedding, source_doc, created_at})
(:Tag {id, name})
(:Document {id, path, title})

// Edges
(:Concept)-[:SIMILAR_TO {strength}]->(:Concept)
(:Concept)-[:CO_OCCURS_WITH {strength}]->(:Concept)
(:Concept)-[:BELONGS_TO]->(:Tag)
(:Concept)-[:EXTRACTED_FROM]->(:Document)
```

## Database Migration

```sql
-- migrations/XXXXXX_create_documents.sql
CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rei_id UUID NOT NULL REFERENCES reis(id),
    title TEXT NOT NULL,
    raw_content TEXT NOT NULL,
    source_path TEXT,
    checksum TEXT NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_documents_rei_id ON documents(rei_id);
CREATE INDEX idx_documents_checksum ON documents(checksum);
CREATE INDEX idx_documents_updated_at ON documents(updated_at);
```

## Environment Variables

```bash
# Shuttle secrets (new)
NEO4J_URI="neo4j+s://xxxxx.databases.neo4j.io"
NEO4J_USER="neo4j"
NEO4J_PASSWORD="..."
```

## Implementation Phases

### Phase 0: DocStore (Source of Truth)
- [ ] `Document` entity definition
- [ ] `DocRepository` trait definition
- [ ] PostgreSQL implementation (`documents` table)
- [ ] `POST /kaiba/rei/{id}/documents` endpoint

### Phase 1: Emphasis Parser
- [ ] `EmphasisNode` / `EmphasisStyle` types
- [ ] Markdown parser (pulldown-cmark extension)
- [ ] Context window extraction (±50 tokens)
- [ ] Weight calculation logic

### Phase 2: GraphKai Adapter (Neo4j)
- [ ] Neo4j Rust client integration (neo4rs)
- [ ] `GraphRepository` trait definition
- [ ] `GraphNode` / `GraphEdge` entities
- [ ] Basic CRUD operations

### Phase 3: Graph Builder Engine
- [ ] `LinkageConfig` YAML loading
- [ ] Contextual embedding generation
- [ ] Similarity-based auto edge generation
- [ ] Co-occurrence detection
- [ ] `POST /kaiba/rei/{id}/graph/rebuild` endpoint

### Phase 4: Hybrid Search Router
- [ ] `HybridSearchService` implementation
- [ ] Query classification logic
- [ ] GraphFirst / RagFirst search implementation
- [ ] Context merge algorithm
- [ ] Replace existing `search_memories_for_rag`

### Phase 5: Operations
- [ ] Linkage strategy config API
- [ ] Graph visualization endpoint
- [ ] Parameter tuning UI
- [ ] Incremental indexing (`list_modified_since`)

## Design Principles

1. **Markdown First**: Content stored as plain text
2. **Implicit Structure**: `**Bold**` implies Concept Node; `#Tag` implies Category
3. **Dynamic Resolution**: Links calculated via vector similarity, not hardcoded
4. **Parameter-Based Maintenance**: Graph density managed by tuning thresholds
5. **Backward Compatible**: Existing APIs unchanged, internal implementation upgraded

## References

- [GraphRAG (Microsoft Research)](https://github.com/microsoft/graphrag)
- [Obsidian / Zettelkasten](https://obsidian.md/)
- [LlamaIndex PropertyGraphIndex](https://docs.llamaindex.ai/)
- [neo4rs - Neo4j Rust Driver](https://github.com/neo4j-labs/neo4rs)
