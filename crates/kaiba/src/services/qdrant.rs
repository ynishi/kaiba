use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParamsBuilder, PointStruct, SearchPointsBuilder, UpsertPointsBuilder};
use qdrant_client::Qdrant;
use std::collections::HashMap;

use crate::models::Memory;

/// Qdrant client wrapper - Gateway to the Memory Sea (記憶海)
pub struct MemoryKai {
    client: Qdrant,
}

impl MemoryKai {
    /// Initialize connection to Qdrant
    pub async fn new(url: &str, api_key: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let client = if let Some(key) = api_key {
            Qdrant::from_url(url).api_key(key).build()?
        } else {
            Qdrant::from_url(url).build()?
        };

        tracing::info!("🌊 Connected to MemoryKai (記憶海)");

        Ok(Self { client })
    }

    /// Create a collection for a persona's memories
    pub async fn create_persona_collection(
        &self,
        persona_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let collection_name = format!("{}_memories", persona_id);

        // Check if collection exists
        if self.client.collection_exists(&collection_name).await? {
            tracing::info!("Collection {} already exists", collection_name);
            return Ok(());
        }

        // Create collection with 1536 dimensions (OpenAI ada-002)
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&collection_name)
                    .vectors_config(VectorParamsBuilder::new(1536, Distance::Cosine))
            )
            .await?;

        tracing::info!("✨ Created collection: {}", collection_name);

        Ok(())
    }

    /// Add a memory to the ocean
    pub async fn add_memory(
        &self,
        persona_id: &str,
        memory: Memory,
        embedding: Vec<f32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let collection_name = format!("{}_memories", persona_id);

        // Ensure collection exists
        self.create_persona_collection(persona_id).await?;

        // Convert memory to HashMap for payload
        let payload: HashMap<String, serde_json::Value> = serde_json::from_value(serde_json::to_value(&memory)?)?;

        // Create point
        let point = PointStruct::new(
            memory.id.clone(),
            embedding,
            payload,
        );

        // Upsert point
        self.client
            .upsert_points(
                UpsertPointsBuilder::new(&collection_name, vec![point])
            )
            .await?;

        tracing::info!("💾 Memory stored in MemoryKai: {}", memory.id);

        Ok(())
    }

    /// Search memories in the ocean
    pub async fn search_memories(
        &self,
        persona_id: &str,
        query_vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<Memory>, Box<dyn std::error::Error>> {
        let collection_name = format!("{}_memories", persona_id);

        // Search
        let search_result = self
            .client
            .search_points(
                SearchPointsBuilder::new(&collection_name, query_vector, limit as u64)
                    .with_payload(true)
            )
            .await?;

        // Parse results
        let memories: Vec<Memory> = search_result
            .result
            .into_iter()
            .filter_map(|point| {
                // Convert payload to HashMap<String, serde_json::Value>
                let payload_json = serde_json::to_value(&point.payload).ok()?;
                serde_json::from_value(payload_json).ok()
            })
            .collect();

        tracing::info!("🔍 Found {} memories in MemoryKai", memories.len());

        Ok(memories)
    }
}
