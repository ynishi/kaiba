//! Document Routes - Source of Truth for GraphKai
//!
//! All mutation endpoints accept arrays by default (batch API design).

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use uuid::Uuid;

use kaiba::{DocRepository, Document, EmphasisParser, SaveStatus};

use crate::models::{
    DeleteDocumentsRequest, DeleteDocumentsResponse, DocumentResponse, DocumentSaveResultDto,
    DocumentStatus, DocumentSummary, EmphasisStats, IngestDocumentsRequest, IngestDocumentsResponse,
    IngestSummary,
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

    // Save batch
    let save_results = doc_store
        .save_batch(&documents)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Parse emphasis for each document
    let emphasis_parser = EmphasisParser::new();

    // Build response
    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let failed = 0;
    let mut total_emphasis_nodes = 0;

    let results: Vec<DocumentSaveResultDto> = save_results
        .iter()
        .map(|r| {
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

            // Parse emphasis for created/updated documents
            let emphasis = if matches!(r.status, SaveStatus::Created | SaveStatus::Updated) {
                let parse_result = emphasis_parser.parse(r.document.id, &r.document.raw_content);
                total_emphasis_nodes += parse_result.total_count();
                Some(EmphasisStats {
                    bold: parse_result.bold_count,
                    italic: parse_result.italic_count,
                    bold_italic: parse_result.bold_italic_count,
                    code: parse_result.code_count,
                })
            } else {
                None
            };

            DocumentSaveResultDto {
                doc_id: r.document.id,
                title: r.document.title.clone(),
                status,
                emphasis,
                error: None,
            }
        })
        .collect();

    let summary = IngestSummary {
        total: results.len(),
        created,
        updated,
        unchanged,
        failed,
        total_emphasis_nodes,
    };

    tracing::info!(
        "Ingested {} documents for Rei {}: {} created, {} updated, {} unchanged, {} emphasis nodes",
        summary.total,
        rei_id,
        created,
        updated,
        unchanged,
        total_emphasis_nodes
    );

    Ok(Json(IngestDocumentsResponse { results, summary }))
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

    Ok(Json(documents.into_iter().map(DocumentSummary::from).collect()))
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
