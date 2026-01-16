//! Graph Routes - GraphKai Knowledge Graph API
//!
//! Provides endpoints for graph construction, traversal, and operations.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, Utc};
use std::time::Instant;
use uuid::Uuid;

use kaiba::{DocRepository, EmphasisParser, GraphBuilder, GraphRepository, LinkageConfig};

use crate::models::{
    GraphEdgeSummary, GraphExportMetadata, GraphExportResponse, GraphNodeSummary,
    GraphStatsResponse, IncrementalRebuildRequest, IncrementalRebuildResponse,
    LinkageConfigResponse, NodeNeighborsResponse, RebuildGraphRequest, RebuildGraphResponse,
    UpdateLinkageConfigRequest,
};
use crate::AppState;

/// Rebuild knowledge graph from documents
///
/// Parses all documents (or specified ones) and builds/updates the knowledge graph
/// based on emphasis (bold, italic, code) detected in the content.
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/graph/rebuild",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = RebuildGraphRequest,
    responses(
        (status = 200, description = "Graph rebuilt", body = RebuildGraphResponse),
        (status = 503, description = "GraphKai or DocStore not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Graph"
)]
pub async fn rebuild_graph(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<RebuildGraphRequest>,
) -> Result<Json<RebuildGraphResponse>, (axum::http::StatusCode, String)> {
    let start = Instant::now();

    let graph_kai = state.graph_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "GraphKai not available".to_string(),
    ))?;

    let doc_store = state.doc_store.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "DocStore not available".to_string(),
    ))?;

    // Get documents to process
    let documents = match &payload.doc_ids {
        Some(ids) => {
            let mut docs = Vec::new();
            for id in ids {
                if let Some(doc) = doc_store
                    .find_by_id(*id)
                    .await
                    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                {
                    if doc.rei_id == rei_id {
                        docs.push(doc);
                    }
                }
            }
            docs
        }
        None => doc_store
            .find_by_rei(rei_id)
            .await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    };

    // Clear existing graph if requested
    if payload.clear_existing {
        graph_kai
            .clear_rei_graph(rei_id)
            .await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Build graph configuration
    let config = payload.config.unwrap_or_default();
    let builder = GraphBuilder::new(config);
    let emphasis_parser = EmphasisParser::new();

    let mut total_nodes_created = 0;
    let mut total_edges_created = 0;
    let mut total_nodes_skipped = 0;
    let mut errors: Vec<String> = Vec::new();

    // Process each document
    for doc in &documents {
        // Parse emphasis from document content
        let parse_result = emphasis_parser.parse(doc.id, &doc.raw_content);

        // Build graph nodes and edges
        let build_result =
            builder.build_from_emphasis(rei_id, doc.id, &doc.title, &parse_result.nodes);

        // Upsert document node
        if let Some(doc_node) = &build_result.doc_node {
            if let Err(e) = graph_kai.upsert_node(doc_node).await {
                errors.push(format!("Failed to upsert doc node for {}: {}", doc.id, e));
                continue;
            }
        }

        // Upsert concept nodes
        if !build_result.nodes.is_empty() {
            match graph_kai.upsert_nodes(&build_result.nodes).await {
                Ok(result) => {
                    total_nodes_created += result.created + result.updated;
                }
                Err(e) => {
                    errors.push(format!("Failed to upsert nodes for {}: {}", doc.id, e));
                }
            }
        }

        // Upsert extraction edges (node -> document)
        if !build_result.extraction_edges.is_empty() {
            match graph_kai.upsert_edges(&build_result.extraction_edges).await {
                Ok(result) => {
                    total_edges_created += result.created;
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to upsert extraction edges for {}: {}",
                        doc.id, e
                    ));
                }
            }
        }

        // Upsert co-occurrence edges
        if !build_result.co_occurrence_edges.is_empty() {
            match graph_kai.upsert_edges(&build_result.co_occurrence_edges).await {
                Ok(result) => {
                    total_edges_created += result.created;
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to upsert co-occurrence edges for {}: {}",
                        doc.id, e
                    ));
                }
            }
        }

        total_nodes_skipped += build_result.stats.nodes_skipped;
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        "Rebuilt graph for Rei {}: {} docs, {} nodes, {} edges, {} skipped in {}ms",
        rei_id,
        documents.len(),
        total_nodes_created,
        total_edges_created,
        total_nodes_skipped,
        duration_ms
    );

    Ok(Json(RebuildGraphResponse {
        documents_processed: documents.len(),
        nodes_created: total_nodes_created,
        edges_created: total_edges_created,
        nodes_skipped: total_nodes_skipped,
        errors,
        duration_ms,
    }))
}

/// Get graph statistics
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/graph/stats",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    responses(
        (status = 200, description = "Graph statistics", body = GraphStatsResponse),
        (status = 503, description = "GraphKai not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Graph"
)]
pub async fn get_graph_stats(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<GraphStatsResponse>, (axum::http::StatusCode, String)> {
    let graph_kai = state.graph_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "GraphKai not available".to_string(),
    ))?;

    let stats = graph_kai
        .get_stats(rei_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(GraphStatsResponse::from(stats)))
}

/// Get node neighbors
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/graph/nodes/{node_id}/neighbors",
    params(
        ("rei_id" = Uuid, Path, description = "Rei ID"),
        ("node_id" = Uuid, Path, description = "Node ID")
    ),
    responses(
        (status = 200, description = "Node neighbors", body = NodeNeighborsResponse),
        (status = 404, description = "Node not found"),
        (status = 503, description = "GraphKai not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Graph"
)]
pub async fn get_node_neighbors(
    State(state): State<AppState>,
    Path((_rei_id, node_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<NodeNeighborsResponse>, (axum::http::StatusCode, String)> {
    let graph_kai = state.graph_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "GraphKai not available".to_string(),
    ))?;

    // Get the node
    let node = graph_kai
        .get_node(node_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "Node not found".to_string(),
        ))?;

    // Get neighbors (depth 1)
    let neighbors = graph_kai
        .get_neighbors(node_id, 1)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get edges connecting to neighbors (both directions)
    let mut edges = graph_kai
        .get_edges_from(node_id, None)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let edges_to = graph_kai
        .get_edges_to(node_id, None)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    edges.extend(edges_to);

    Ok(Json(NodeNeighborsResponse {
        node: GraphNodeSummary::from(node),
        neighbors: neighbors.into_iter().map(GraphNodeSummary::from).collect(),
        edges: edges.into_iter().map(GraphEdgeSummary::from).collect(),
    }))
}

// ============================================
// Phase 5: Operations Endpoints
// ============================================

/// Get current linkage configuration
///
/// Returns the default LinkageConfig. Per-Rei custom configs would require
/// database storage which is not implemented yet.
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/graph/config",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    responses(
        (status = 200, description = "Linkage configuration", body = LinkageConfigResponse),
    ),
    tag = "Graph"
)]
pub async fn get_linkage_config(
    Path(_rei_id): Path<Uuid>,
) -> Result<Json<LinkageConfigResponse>, (axum::http::StatusCode, String)> {
    // For now, return default config
    // TODO: Store per-Rei config in database
    Ok(Json(LinkageConfigResponse {
        config: LinkageConfig::default(),
        updated_at: None,
    }))
}

/// Update linkage configuration
///
/// Currently accepts the config but does not persist it (returns the config
/// for immediate use in rebuild operations). Per-Rei persistence would require
/// database storage.
#[utoipa::path(
    put,
    path = "/kaiba/rei/{rei_id}/graph/config",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = UpdateLinkageConfigRequest,
    responses(
        (status = 200, description = "Configuration updated", body = LinkageConfigResponse),
    ),
    tag = "Graph"
)]
pub async fn update_linkage_config(
    Path(_rei_id): Path<Uuid>,
    Json(payload): Json<UpdateLinkageConfigRequest>,
) -> Result<Json<LinkageConfigResponse>, (axum::http::StatusCode, String)> {
    // For now, just return the provided config
    // TODO: Store per-Rei config in database
    Ok(Json(LinkageConfigResponse {
        config: payload.config,
        updated_at: Some(Utc::now()),
    }))
}

/// Incremental graph rebuild
///
/// Only processes documents modified since the specified time.
/// Much faster than full rebuild for active documents.
#[utoipa::path(
    post,
    path = "/kaiba/rei/{rei_id}/graph/incremental",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    request_body = IncrementalRebuildRequest,
    responses(
        (status = 200, description = "Incremental rebuild completed", body = IncrementalRebuildResponse),
        (status = 503, description = "GraphKai or DocStore not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Graph"
)]
pub async fn incremental_rebuild(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
    Json(payload): Json<IncrementalRebuildRequest>,
) -> Result<Json<IncrementalRebuildResponse>, (axum::http::StatusCode, String)> {
    let start = Instant::now();
    let until = Utc::now();

    let graph_kai = state.graph_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "GraphKai not available".to_string(),
    ))?;

    let doc_store = state.doc_store.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "DocStore not available".to_string(),
    ))?;

    // Default to last hour if not specified
    let since = payload.since.unwrap_or_else(|| Utc::now() - Duration::hours(1));

    // Find modified documents
    let documents = doc_store
        .find_modified_since(rei_id, since)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let documents_found = documents.len();

    if documents.is_empty() {
        return Ok(Json(IncrementalRebuildResponse {
            documents_found: 0,
            documents_processed: 0,
            nodes_created: 0,
            edges_created: 0,
            errors: vec![],
            duration_ms: start.elapsed().as_millis() as u64,
            since,
            until,
        }));
    }

    // Build graph
    let config = payload.config.unwrap_or_default();
    let builder = GraphBuilder::new(config);
    let emphasis_parser = EmphasisParser::new();

    let mut total_nodes_created = 0;
    let mut total_edges_created = 0;
    let mut errors: Vec<String> = Vec::new();

    for doc in &documents {
        // Delete existing nodes for this document first
        if let Err(e) = graph_kai.delete_nodes_by_document(doc.id).await {
            errors.push(format!("Failed to clear old nodes for {}: {}", doc.id, e));
        }

        // Parse emphasis from document content
        let parse_result = emphasis_parser.parse(doc.id, &doc.raw_content);

        // Build graph nodes and edges
        let build_result =
            builder.build_from_emphasis(rei_id, doc.id, &doc.title, &parse_result.nodes);

        // Upsert document node
        if let Some(doc_node) = &build_result.doc_node {
            if let Err(e) = graph_kai.upsert_node(doc_node).await {
                errors.push(format!("Failed to upsert doc node for {}: {}", doc.id, e));
                continue;
            }
        }

        // Upsert concept nodes
        if !build_result.nodes.is_empty() {
            match graph_kai.upsert_nodes(&build_result.nodes).await {
                Ok(result) => {
                    total_nodes_created += result.created + result.updated;
                }
                Err(e) => {
                    errors.push(format!("Failed to upsert nodes for {}: {}", doc.id, e));
                }
            }
        }

        // Upsert edges
        let all_edges: Vec<_> = build_result
            .extraction_edges
            .into_iter()
            .chain(build_result.co_occurrence_edges)
            .collect();

        if !all_edges.is_empty() {
            match graph_kai.upsert_edges(&all_edges).await {
                Ok(result) => {
                    total_edges_created += result.created;
                }
                Err(e) => {
                    errors.push(format!("Failed to upsert edges for {}: {}", doc.id, e));
                }
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        "Incremental rebuild for Rei {}: {} modified docs, {} nodes, {} edges in {}ms",
        rei_id,
        documents_found,
        total_nodes_created,
        total_edges_created,
        duration_ms
    );

    Ok(Json(IncrementalRebuildResponse {
        documents_found,
        documents_processed: documents.len(),
        nodes_created: total_nodes_created,
        edges_created: total_edges_created,
        errors,
        duration_ms,
        since,
        until,
    }))
}

/// Export graph for visualization
///
/// Returns all nodes and edges in a format suitable for graph visualization tools.
#[utoipa::path(
    get,
    path = "/kaiba/rei/{rei_id}/graph/export",
    params(("rei_id" = Uuid, Path, description = "Rei ID")),
    responses(
        (status = 200, description = "Graph export", body = GraphExportResponse),
        (status = 503, description = "GraphKai not available"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Graph"
)]
pub async fn export_graph(
    State(state): State<AppState>,
    Path(rei_id): Path<Uuid>,
) -> Result<Json<GraphExportResponse>, (axum::http::StatusCode, String)> {
    let graph_kai = state.graph_kai.as_ref().ok_or((
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "GraphKai not available".to_string(),
    ))?;

    // Get all nodes for this Rei
    // We use find_nodes_by_text with empty string to get all nodes (limit to reasonable number)
    let nodes = graph_kai
        .find_nodes_by_text(rei_id, "", None, 1000)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Collect all edges for found nodes using batch operation (avoids N+1)
    let node_ids: Vec<Uuid> = nodes.iter().map(|n| n.id).collect();
    let all_edges = graph_kai
        .get_edges_for_nodes(&node_ids)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get stats
    let stats = graph_kai
        .get_stats(rei_id)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(GraphExportResponse {
        nodes: nodes.into_iter().map(GraphNodeSummary::from).collect(),
        edges: all_edges.into_iter().map(GraphEdgeSummary::from).collect(),
        stats: GraphStatsResponse::from(stats),
        metadata: GraphExportMetadata {
            rei_id,
            exported_at: Utc::now(),
            format_version: "1.0".to_string(),
        },
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kaiba/rei/:rei_id/graph/rebuild", post(rebuild_graph))
        .route("/kaiba/rei/:rei_id/graph/stats", get(get_graph_stats))
        .route(
            "/kaiba/rei/:rei_id/graph/nodes/:node_id/neighbors",
            get(get_node_neighbors),
        )
        // Phase 5: Operations
        .route("/kaiba/rei/:rei_id/graph/config", get(get_linkage_config))
        .route("/kaiba/rei/:rei_id/graph/config", put(update_linkage_config))
        .route(
            "/kaiba/rei/:rei_id/graph/incremental",
            post(incremental_rebuild),
        )
        .route("/kaiba/rei/:rei_id/graph/export", get(export_graph))
}
