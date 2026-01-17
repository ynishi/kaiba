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
POST /kaiba/rei/{id}/documents           # Ingest documents (batch)
GET  /kaiba/rei/{id}/documents           # List documents
GET  /kaiba/rei/{id}/documents/{doc_id}  # Get document
DELETE /kaiba/rei/{id}/documents         # Delete documents (batch)

POST /kaiba/rei/{id}/graph/rebuild       # Rebuild graph with new thresholds
GET  /kaiba/rei/{id}/graph/nodes         # List graph nodes
GET  /kaiba/rei/{id}/graph/neighbors/{node_id}  # Get node neighbors
```

### Batch API Design

All mutation endpoints accept arrays by default:

```rust
// POST /kaiba/rei/{id}/documents
#[derive(Deserialize)]
pub struct IngestDocumentsRequest {
    pub documents: Vec<DocumentInput>,  // Always array, even for single doc
}

#[derive(Deserialize)]
pub struct DocumentInput {
    pub title: String,
    pub content: String,               // Raw Markdown
    pub source_path: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct IngestDocumentsResponse {
    pub results: Vec<IngestResult>,
    pub summary: IngestSummary,
}

#[derive(Serialize)]
pub struct IngestResult {
    pub doc_id: String,
    pub title: String,
    pub status: IngestStatus,          // Created, Updated, Failed
    pub nodes_created: usize,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct IngestSummary {
    pub total: usize,
    pub created: usize,
    pub updated: usize,
    pub failed: usize,
}
```

```rust
// DELETE /kaiba/rei/{id}/documents
#[derive(Deserialize)]
pub struct DeleteDocumentsRequest {
    pub doc_ids: Vec<String>,          // Always array
}

#[derive(Serialize)]
pub struct DeleteDocumentsResponse {
    pub deleted: usize,
    pub not_found: Vec<String>,
}
```

**Design Rationale**:
- Single doc = array of 1 (no separate endpoint)
- Reduces API surface
- Enables efficient bulk operations (transaction batching)
- Client always uses same code path

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

### Search Strategies (8種類)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HybridStrategy {
    // === 複合戦略 (5つ) ===
    GraphFirst,     // Graph → Keywords → RAG
    RagFirst,       // RAG → Keywords → Graph → RAG
    Parallel,       // RAG + DB + Graph 同時実行
    MultiHop,       // RAG → Graph → RAG (iterative, depth指定)
    #[default]
    Auto,           // クエリから自動判定

    // === 単体戦略 (3つ) ===
    SingleRag,      // RAG (Qdrant) only
    SingleDb,       // DB full-text (PostgreSQL) only
    SingleGraph,    // Graph (Neo4j) only
}
```

### Strategy Set (複数戦略の組み合わせ)

```rust
/// 戦略セット（複数指定 → 並列実行）
pub struct StrategySet {
    pub strategies: HashSet<HybridStrategy>,
    pub hop_depth: u32,  // MultiHop用
}

// 使用例
StrategySet::single(HybridStrategy::Auto)                    // 従来通り
StrategySet::multiple([GraphFirst, SingleDb])                // GraphFirst + DB並列
StrategySet::multiple([MultiHop, SingleDb])                  // MultiHop + DB並列
```

| 設定 | 動作 |
|------|------|
| `[Auto]` | クエリから自動判定 |
| `[SingleRag, SingleDb]` | RAG + DB 並列 |
| `[GraphFirst, SingleDb]` | GraphFirst + DB 並列 |
| `[MultiHop]` | RAG → Graph → RAG (iterative) |
| `[MultiHop, SingleDb]` | MultiHop + DB 並列 |

### MultiHop Search Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MultiHop Search Flow                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Query: "async runtime"                                              │
│      │                                                               │
│      ▼                                                               │
│  ┌─────────────────────────────────────────┐                        │
│  │ HOP 1: Initial RAG Search               │                        │
│  │   Qdrant vector search                  │                        │
│  │   Output: [tokio docs, async-std...]    │                        │
│  └─────────────────────────────────────────┘                        │
│      │                                                               │
│      ▼                                                               │
│  ┌─────────────────────────────────────────┐                        │
│  │ HOP 2: Keyword Extraction               │                        │
│  │   - Tags from memories                  │                        │
│  │   - Significant words from content      │                        │
│  │   Output: ["tokio", "polling", "Pin"]   │                        │
│  └─────────────────────────────────────────┘                        │
│      │                                                               │
│      ▼                                                               │
│  ┌─────────────────────────────────────────┐                        │
│  │ HOP 3: Graph Expansion                  │                        │
│  │   Neo4j: find_nodes + get_neighbors     │                        │
│  │   Output: ["smol", "mio", "epoll",      │                        │
│  │            "green threads", "executor"] │                        │
│  └─────────────────────────────────────────┘                        │
│      │                                                               │
│      ▼                                                               │
│  ┌─────────────────────────────────────────┐                        │
│  │ HOP 4: Expanded RAG Search              │                        │
│  │   Embed expanded keywords               │                        │
│  │   Search with lower score (0.9x)        │                        │
│  └─────────────────────────────────────────┘                        │
│      │                                                               │
│      ▼                                                               │
│  Merge all results → apply_context → Final                          │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Integrated Search Flow (Parallel)

```
Query + Context
      ↓
      ├──────────────────────┬────────────────────────┐
      ↓                      ↓                        ↓
[Path A: RAG]          [Path B: DB全文検索]     [Path C: Graph]
  ↓                        ↓                        ↓
Qdrant Vector         PostgreSQL ILIKE        Neo4j Text
Search                + to_tsvector           Search
  ↓                        ↓                        ↓
  └──────────────┬─────────┴────────────────────────┘
                 ↓
          [Merge & Dedupe]
                 ↓
          [Post Re-ranking]
            apply_context()
            - topic_path match: 1.5x boost
            - tags match: 1.2x boost
            - content match: 1.0x boost
            - weight=0: exclude
                 ↓
          [Sort by Score]
                 ↓
            Results
```

### Result Sources

```rust
pub struct HybridSearchResult {
    pub memories: Vec<ScoredMemory>,
    pub rag_sources: Vec<String>,     // Memory IDs from Qdrant
    pub graph_sources: Vec<String>,   // Node IDs from Neo4j
    pub db_sources: Vec<String>,      // Document IDs from PostgreSQL
    pub strategy_used: HybridStrategy,
}
```

### Context Weights (Post Re-ranking)

```rust
/// Context weight for boosting/excluding topics
/// - weight > 0: boost (1.0 = full boost)
/// - weight = 0: exclude
pub type ContextWeights = HashMap<String, f32>;

pub struct HybridSearchConfig {
    pub strategy: HybridStrategy,
    pub rag_limit: usize,
    pub graph_depth: u32,
    pub min_similarity: f32,
    pub context: ContextWeights,  // For post re-ranking
}
```

## Micro-Expertise Generation

### Concept

Unlike traditional RAG that chunks existing documents, Kaiba generates **structured Micro-Expertises** at creation time:

```
┌─────────────────────────────────────────────────────────────┐
│ TRADITIONAL RAG                                              │
│                                                              │
│  Long Document → Chunker → Chunks[] → Embeddings → Qdrant   │
│                    ↓                                         │
│              (post-hoc splitting, metadata extraction)       │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ KAIBA APPROACH                                               │
│                                                              │
│  Web Results → LLM Digest → Micro-Expertise[] → Qdrant      │
│                    ↓              ↓                          │
│              (structured prompt)  (topic_path, concepts      │
│                                    extracted in-line)        │
└─────────────────────────────────────────────────────────────┘
```

### Implementation

- **Separator-based parsing**: LLM output uses `=====` delimiter
- **Per-expertise storage**: Each expertise stored as separate Memory
- **topic_path extraction**: Hierarchical category from expertise content

```rust
pub struct Memory {
    pub id: String,
    pub rei_id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub importance: f32,
    pub tags: Vec<String>,
    pub topic_path: Option<String>,  // "Rust > Concurrency > Async"
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
```

## Context-Aware Search Protocol

### Request Structure

```json
{
  "search_query": "term",
  "context_injection": {
    "positive_weights": [
      { "topic": "Rust", "weight": 1.0 },
      { "topic": "Terminal UI", "weight": 0.8 }
    ],
    "negative_weights": [
      { "topic": "Finance", "weight": 0.0 }
    ]
  }
}
```

### Processing Flow

1. **Parallel Search**: Execute RAG + DB + Graph simultaneously
2. **Merge & Dedupe**: Combine results with ID-based deduplication
3. **Exclude Filter**: Remove matches where weight = 0
4. **Boost Scoring**: Apply priority multipliers
   - topic_path match: 1.5x
   - tags match: 1.2x
   - content match: 1.0x
5. **Re-sort**: Order by final score descending

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

### Phase 0: DocStore (Source of Truth) ✅
- [x] `Document` entity definition
- [x] `DocRepository` trait definition
- [x] PostgreSQL implementation (`documents` table)
- [x] `POST /kaiba/rei/{id}/documents` endpoint
- [x] `search_fulltext()` for DB full-text search

### Phase 1: Emphasis Parser ✅
- [x] `EmphasisNode` / `EmphasisStyle` types
- [x] Markdown parser (pulldown-cmark extension)
- [x] Context window extraction (±50 tokens)
- [x] Weight calculation logic

### Phase 2: GraphKai Adapter (Neo4j) ✅
- [x] Neo4j Rust client integration (neo4rs)
- [x] `GraphRepository` trait definition
- [x] `GraphNode` / `GraphEdge` entities
- [x] Basic CRUD operations

### Phase 3: Graph Builder Engine ✅
- [x] `LinkageConfig` default implementation
- [x] Contextual embedding generation
- [x] Similarity-based auto edge generation
- [x] Co-occurrence detection
- [x] `POST /kaiba/rei/{id}/graph/rebuild` endpoint

### Phase 4: Hybrid Search Router ✅
- [x] `HybridSearchService` implementation
- [x] Query classification logic (Japanese support)
- [x] GraphFirst / RagFirst / Parallel search implementation
- [x] **Triple-store parallel search** (RAG + DB + Graph)
- [x] Context merge algorithm
- [x] **Post Re-ranking** with topic_path/tags/content priority
- [x] Replace existing `search_memories_for_rag`

### Phase 5: Triple-Store Document Ingestion ✅
- [x] Document ingest saves to all 3 stores:
  - DocStore (PostgreSQL) - Source of Truth
  - MemoryKai (Qdrant) - RAG
  - GraphKai (Neo4j) - Knowledge Graph
- [x] `topic_path` field in Memory model

### Phase 6: Micro-Expertise Generation ✅
- [x] Separator-based parsing (`=====`) for multiple expertises
- [x] `topic_path` extraction from expertise content
- [x] Each expertise stored as separate Memory in Qdrant

### Phase 7: Operations (Partial)
- [ ] Linkage strategy config API
- [ ] Graph visualization endpoint
- [ ] Parameter tuning UI
- [x] Incremental indexing (`find_modified_since`)

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
