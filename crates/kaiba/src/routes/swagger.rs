//! OpenAPI Documentation
//!
//! Centralized API documentation using utoipa.

use utoipa::OpenApi;

use crate::models::{
    // Rei models
    Rei, ReiState, CreateReiRequest, UpdateReiRequest, ReiResponse, ReiStateResponse, UpdateReiStateRequest,
    // Tei models
    Provider, Tei, CreateTeiRequest, UpdateTeiRequest, TeiResponse, AssociateTeiRequest,
    // Memory models
    MemoryType, Memory, CreateMemoryRequest, SearchMemoriesRequest, MemoryResponse,
    // Call models
    TaskHealth, CallLog, CallContext, CallRequest, MemoryReference, CallResponse,
    // Prompt models
    PromptFormat, PromptResponse, ReiSummary,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Kaiba API",
        version = "0.1.0",
        description = "海馬 (Hippocampus) - Autonomous Persona Architecture API\n\nSeparates persistent Rei (霊 - Spirit) from ephemeral Tei (体 - Body).",
        license(name = "MIT"),
    ),
    servers(
        (url = "/", description = "Current server"),
    ),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Rei", description = "Rei (霊) - Persistent persona identity management"),
        (name = "Tei", description = "Tei (体) - Execution interface management"),
        (name = "Memory", description = "Memory (記憶) - Long-term storage via Qdrant"),
        (name = "Call", description = "Call - LLM invocation with RAG"),
        (name = "Prompt", description = "Prompt - Generate prompts for external Teis"),
        (name = "Search", description = "Search - Web search via Gemini"),
        (name = "Learning", description = "Learning - Autonomous self-learning"),
    ),
    components(
        schemas(
            // Rei
            Rei,
            ReiState,
            CreateReiRequest,
            UpdateReiRequest,
            ReiResponse,
            ReiStateResponse,
            UpdateReiStateRequest,
            // Tei
            Provider,
            Tei,
            CreateTeiRequest,
            UpdateTeiRequest,
            TeiResponse,
            AssociateTeiRequest,
            // Memory
            MemoryType,
            Memory,
            CreateMemoryRequest,
            SearchMemoriesRequest,
            MemoryResponse,
            // Call
            TaskHealth,
            CallLog,
            CallContext,
            CallRequest,
            MemoryReference,
            CallResponse,
            // Prompt
            PromptFormat,
            PromptResponse,
            ReiSummary,
        )
    ),
)]
pub struct ApiDoc;
