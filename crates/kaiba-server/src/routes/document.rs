//! Document Routes - Source of Truth for GraphKai
//!
//! All mutation endpoints accept arrays by default (batch API design).
//! Ingestion saves to: DocStore (PostgreSQL) + MemoryKai (Qdrant) + GraphKai (Neo4j)

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use uuid::Uuid;

use kaiba::{DocRepository, Document, EmphasisParser, GraphBuilder, GraphRepository, SaveStatus};

use crate::models::{
    DeleteDocumentsRequest, DeleteDocumentsResponse, DocumentResponse, DocumentSaveResultDto,
    DocumentStatus, DocumentSummary, EmphasisStats, IngestDocumentsRequest,
    IngestDocumentsResponse, IngestSummary, Memory, MemoryType,
};
use crate::AppState;

/// Ingest documents (batch)
///
/// Accepts an array of documents, even for single document ingestion.
/// Returns status for each document and a summary.
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/documents",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = IngestDocumentsRequest,
    responses(
        (status = 200, description = "Documents ingested", body = IngestDocumentsResponse),
        (status = 503, description = "DocStore not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Document"
)]
pub async fn ingest_documents(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<IngestDocumentsRequest>,
) -> Result<Json<IngestDocumentsResponse>, (axum::http::StatusCode, String)> {
    let doc_store = state.doc_store.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "DocStore not available".to_string(),
    ))?;

    // Convert inputs to Document entities
    let documents: Vec<Document> = payload
        .documents
        .iter()
        .map(|input| {
            Document::new(
                rei_id,
                input.title.clone(),
                input.content.clone(),
                input.source_path.clone(),
                input.metadata.clone(),
            )
        })
        .collect();

    // 1. Save to DocStore (PostgreSQL) - Source of Truth
    let save_results = doc_store
        .save_batch(&documents)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let emphasis_parser = EmphasisParser::new();
    let graph_builder = GraphBuilder::new(Default::default());

    // Build response and process RAG + Graph for created/updated documents
    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let mut failed = 0;
    let mut total_emphasis_nodes = 0;
    let mut total_rag_entries = 0;
    let mut total_graph_nodes = 0;

    let mut results: Vec<DocumentSaveResultDto> = Vec::with_capacity(save_results.len());

    for r in &save_results {
        let status = match r.status {
            SaveStatus::Created => {
                created += 1;
                DocumentStatus::Created
            }
            SaveStatus::Updated => {
                updated += 1;
                DocumentStatus::Updated
            }
            SaveStatus::Unchanged => {
                unchanged += 1;
                DocumentStatus::Unchanged
            }
        };

        let mut emphasis = None;
        let mut error = None;

        // Process RAG + Graph for created/updated documents
        if matches!(r.status, SaveStatus::Created | SaveStatus::Updated) {
            // Parse emphasis
            let parse_result = emphasis_parser.parse(r.document.id, &r.document.raw_content);
            total_emphasis_nodes += parse_result.total_count();
            emphasis = Some(EmphasisStats {
                bold: parse_result.bold_count,
                italic: parse_result.italic_count,
                bold_italic: parse_result.bold_italic_count,
                code: parse_result.code_count,
            });

            // 2. Save to MemoryKai (Qdrant) - RAG
            if let (Some(memory_kai), Some(embedding_service)) =
                (&state.memory_kai, &state.embedding)
            {
                match save_document_to_rag(memory_kai, embedding_service, rei_id, &r.document).await
                {
                    Ok(count) => {
                        total_rag_entries += count;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to save doc {} to RAG: {}", r.document.id, e);
                        error = Some(format!("RAG save failed: {}", e));
                    }
                }
            }

            // 3. Save to GraphKai (Neo4j) - Knowledge Graph
            if let Some(graph_kai) = &state.graph_kai {
                match save_document_to_graph(
                    graph_kai.as_ref(),
                    &graph_builder,
                    &emphasis_parser,
                    rei_id,
                    &r.document,
                )
                .await
                {
                    Ok(count) => {
                        total_graph_nodes += count;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to save doc {} to Graph: {}", r.document.id, e);
                        if let Some(ref mut err) = error {
                            err.push_str(&format!(", Graph save failed: {}", e));
                        } else {
                            error = Some(format!("Graph save failed: {}", e));
                        }
                    }
                }
            }

            if error.is_some() {
                failed += 1;
            }
        }

        results.push(DocumentSaveResultDto {
            doc_id: r.document.id,
            title: r.document.title.clone(),
            status,
            emphasis,
            error,
        });
    }

    let summary = IngestSummary {
        total: results.len(),
        created,
        updated,
        unchanged,
        failed,
        total_emphasis_nodes,
    };

    tracing::info!(
        "Ingested {} documents for Rei {}: {} created, {} updated, {} unchanged, {} emphasis, {} RAG entries, {} graph nodes",
        summary.total,
        rei_id,
        created,
        updated,
        unchanged,
        total_emphasis_nodes,
        total_rag_entries,
        total_graph_nodes
    );

    Ok(Json(IngestDocumentsResponse { results, summary }))
}

/// Save document content to MemoryKai (Qdrant) for RAG retrieval
async fn save_document_to_rag(
    memory_kai: &crate::services::qdrant::MemoryKai,
    embedding_service: &crate::services::embedding::EmbeddingService,
    rei_id: Uuid,
    document: &Document,
) -> Result<usize, String> {
    // Create a Memory entry from the document
    let memory = Memory {
        id: format!("doc:{}", document.id),
        rei_id: rei_id.to_string(),
        content: document.raw_content.clone(),
        memory_type: MemoryType::Fact,
        importance: 0.8,
        tags: vec!["document".to_string(), "source".to_string()],
        topic_path: None, // TODO: Extract from document metadata or content
        metadata: Some(serde_json::json!({
            "doc_id": document.id.to_string(),
            "title": document.title,
            "source_path": document.source_path,
        })),
        created_at: document.created_at,
    };

    // Generate embedding
    let vector = embedding_service
        .embed(&document.raw_content)
        .await
        .map_err(|e| e.to_string())?;

    // Save to Qdrant
    memory_kai
        .add_memory(&rei_id.to_string(), memory, vector)
        .await
        .map_err(|e| e.to_string())?;

    Ok(1)
}

/// Save document to GraphKai (Neo4j) - builds knowledge graph from emphasis
async fn save_document_to_graph(
    graph_kai: &dyn GraphRepository,
    graph_builder: &GraphBuilder,
    emphasis_parser: &EmphasisParser,
    rei_id: Uuid,
    document: &Document,
) -> Result<usize, String> {
    // Parse emphasis from document
    let parse_result = emphasis_parser.parse(document.id, &document.raw_content);

    if parse_result.nodes.is_empty() {
        return Ok(0);
    }

    // Build graph nodes and edges
    let build_result = graph_builder.build_from_emphasis(
        rei_id,
        document.id,
        &document.title,
        &parse_result.nodes,
    );

    let mut nodes_created = 0;

    // Upsert document node
    if let Some(doc_node) = &build_result.doc_node {
        graph_kai
            .upsert_node(doc_node)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Upsert concept nodes
    if !build_result.nodes.is_empty() {
        let result = graph_kai
            .upsert_nodes(&build_result.nodes)
            .await
            .map_err(|e| e.to_string())?;
        nodes_created = result.created + result.updated;
    }

    // Upsert edges
    if !build_result.extraction_edges.is_empty() {
        graph_kai
            .upsert_edges(&build_result.extraction_edges)
            .await
            .map_err(|e| e.to_string())?;
    }

    if !build_result.co_occurrence_edges.is_empty() {
        graph_kai
            .upsert_edges(&build_result.co_occurrence_edges)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(nodes_created)
}

/// List documents for a Rei
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/documents",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    responses(
        (status = 200, description = "Document list", body = Vec<DocumentSummary>),
        (status = 503, description = "DocStore not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Document"
)]
pub async fn list_documents(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<Vec<DocumentSummary>>, (axum::http::StatusCode, String)> {
    let doc_store = state.doc_store.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "DocStore not available".to_string(),
    ))?;

    let documents = doc_store
        .find_by_rei(rei_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        documents.into_iter().map(DocumentSummary::from).collect(),
    ))
}

/// Get a single document by ID
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/documents/{doc_id}",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("doc_id" = Uuid, Path, description = "Document ID")
    ),
    responses(
        (status = 200, description = "Document found", body = DocumentResponse),
        (status = 404, description = "Document not found"),
        (status = 503, description = "DocStore not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Document"
)]
pub async fn get_document(
    State(state): State<AppState>,
    Path((rei_id, doc_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<DocumentResponse>, (axum::http::StatusCode, String)> {
    let doc_store = state.doc_store.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "DocStore not available".to_string(),
    ))?;

    let document = doc_store
        .find_by_id(doc_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Document not found".to_string(),
        ))?;

    // Verify document belongs to the Rei
    if document.rei_id != rei_id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Document not found".to_string(),
        ));
    }

    Ok(Json(DocumentResponse::from(document)))
}

/// Delete documents (batch)
#[utoipa::path(
    delete,
    path = "/kaiba/rei/{rei_id}/documents",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = DeleteDocumentsRequest,
    responses(
        (status = 200, description = "Documents deleted", body = DeleteDocumentsResponse),
        (status = 503, description = "DocStore not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Document"
)]
pub async fn delete_documents(
    State(state): State<AppState>,
    Path(_rei_id): Path<Uuid>,
    Json(payload): Json<DeleteDocumentsRequest>,
) -> Result<Json<DeleteDocumentsResponse>, (axum::http::StatusCode, String)> {
    let doc_store = state.doc_store.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "DocStore not available".to_string(),
    ))?;

    let result = doc_store
        .delete_batch(&payload.doc_ids)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        "Deleted {} documents, {} not found",
        result.deleted,
        result.not_found.len()
    );

    Ok(Json(DeleteDocumentsResponse {
        deleted: result.deleted,
        not_found: result.not_found,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/documents", post(ingest_documents))
        .route("/kaiba/rei/:rei_id/documents", get(list_documents))
        .route("/kaiba/rei/:rei_id/documents/:doc_id", get(get_document))
        .route("/kaiba/rei/:rei_id/documents", delete(delete_documents))
}
