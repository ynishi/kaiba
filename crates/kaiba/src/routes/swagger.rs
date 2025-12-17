//! OpenAPI Documentation
//!
//! Centralized API documentation using utoipa.

use utoipa::OpenApi;

use crate::models::{
    AssociateTeiRequest,
    CallContext,
    CallLog,
    CallRequest,
    CallResponse,
    CreateMemoryRequest,
    CreateReiRequest,
    CreateTeiRequest,
    Memory,
    MemoryReference,
    MemoryResponse,
    // Memory models
    MemoryType,
    // Prompt models
    PromptFormat,
    PromptResponse,
    // Tei models
    Provider,
    // Rei models
    Rei,
    ReiResponse,
    ReiState,
    ReiStateResponse,
    ReiSummary,
    SearchMemoriesRequest,
    // Call models
    TaskHealth,
    Tei,
    TeiResponse,
    UpdateReiRequest,
    UpdateReiStateRequest,
    UpdateTeiRequest,
};

use crate::services::self_learning::LearningSession;
use crate::services::web_search::WebSearchReference;

// Local route types
use super::learning::{
    BatchLearnResponse, LearnRequest, LearnResponse, RechargeRequest, RechargeResponse,
};
use super::search::{SearchRequest, SearchResult};

#[derive(OpenApi)]
#[openapi(
    paths(
        // Rei endpoints
        super::rei::list_reis,
        super::rei::create_rei,
        super::rei::get_rei,
        super::rei::update_rei,
        super::rei::delete_rei,
        super::rei::get_rei_state,
        super::rei::update_rei_state,
        // Tei endpoints
        super::tei::list_teis,
        super::tei::create_tei,
        super::tei::get_tei,
        super::tei::update_tei,
        super::tei::delete_tei,
        super::tei::get_tei_expertise,
        super::tei::update_tei_expertise,
        super::tei::list_rei_teis,
        super::tei::associate_tei,
        super::tei::disassociate_tei,
        // Memory endpoints
        super::memory::add_memory,
        super::memory::search_memories,
        // Call endpoints
        super::call::call_llm,
        super::call::get_call_history,
        // Prompt endpoints
        super::prompt::generate_prompt,
        // Search endpoints
        super::search::web_search,
        // Learning endpoints
        super::learning::learn_rei,
        super::learning::learn_all,
        super::learning::recharge_rei,
    ),
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
            // Search
            SearchRequest,
            SearchResult,
            WebSearchReference,
            // Learning
            LearnRequest,
            LearnResponse,
            BatchLearnResponse,
            RechargeRequest,
            RechargeResponse,
            LearningSession,
        )
    ),
)]
pub struct ApiDoc;
