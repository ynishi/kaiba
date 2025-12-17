use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, Distance, FieldType,
    Filter, PointStruct, Range, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use std::collections::HashMap;

use crate::models::{Memory, MemoryType, TagMatchMode};

/// Search filter options for memory queries
#[derive(Debug, Default)]
pub struct SearchFilter {
    /// Filter by memory type
    pub memory_type: Option<MemoryType>,
    /// Filter by tags
    pub tags: Vec<String>,
    /// Tag matching mode
    pub tags_match_mode: TagMatchMode,
    /// Minimum importance score
    pub min_importance: Option<f32>,
}

/// Qdrant client wrapper - Gateway to the Memory Sea (記憶海)
pub struct MemoryKai {
    client: Qdrant,
}

impl MemoryKai {
    /// Initialize connection to Qdrant
    pub async fn new(
        url: &str,
        api_key: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
            // Ensure indexes exist (idempotent)
            self.ensure_field_indexes(&collection_name).await?;
            return Ok(());
        }

        // Create collection with 1536 dimensions (OpenAI ada-002)
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&collection_name)
                    .vectors_config(VectorParamsBuilder::new(1536, Distance::Cosine)),
            )
            .await?;

        tracing::info!("✨ Created collection: {}", collection_name);

        // Create field indexes for filtering
        self.ensure_field_indexes(&collection_name).await?;

        Ok(())
    }

    /// Ensure required field indexes exist for filtering
    async fn ensure_field_indexes(
        &self,
        collection_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create indexes for filterable fields
        let indexes = [
            ("memory_type", FieldType::Keyword),
            ("tags", FieldType::Keyword),
            ("importance", FieldType::Float),
        ];

        for (field_name, field_type) in indexes {
            match self
                .client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    collection_name,
                    field_name,
                    field_type,
                ))
                .await
            {
                Ok(_) => {
                    tracing::info!("📇 Created index for {} in {}", field_name, collection_name);
                }
                Err(e) => {
                    // Index might already exist, which is fine
                    tracing::debug!(
                        "Index creation for {} (may already exist): {}",
                        field_name,
                        e
                    );
                }
            }
        }

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
        let payload: HashMap<String, serde_json::Value> =
            serde_json::from_value(serde_json::to_value(&memory)?)?;

        // Create point
        let point = PointStruct::new(memory.id.clone(), embedding, payload);

        // Upsert point
        self.client
            .upsert_points(UpsertPointsBuilder::new(&collection_name, vec![point]))
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
        self.search_memories_with_filter(persona_id, query_vector, limit, SearchFilter::default())
            .await
    }

    /// Search memories with filter options
    pub async fn search_memories_with_filter(
        &self,
        persona_id: &str,
        query_vector: Vec<f32>,
        limit: usize,
        filter: SearchFilter,
    ) -> Result<Vec<Memory>, Box<dyn std::error::Error>> {
        let collection_name = format!("{}_memories", persona_id);

        // Build filter conditions
        let qdrant_filter = self.build_filter(&filter);

        // Search with optional filter
        let mut search_builder =
            SearchPointsBuilder::new(&collection_name, query_vector, limit as u64)
                .with_payload(true);

        if let Some(f) = qdrant_filter {
            search_builder = search_builder.filter(f);
        }

        let search_result = self.client.search_points(search_builder).await?;

        // Parse results
        let memories: Vec<Memory> = search_result
            .result
            .into_iter()
            .filter_map(|point| {
                let payload_json = serde_json::to_value(&point.payload).ok()?;
                serde_json::from_value(payload_json).ok()
            })
            .collect();

        tracing::info!(
            "🔍 Found {} memories in MemoryKai (filter: {:?})",
            memories.len(),
            filter
        );

        Ok(memories)
    }

    /// Build Qdrant filter from SearchFilter
    fn build_filter(&self, filter: &SearchFilter) -> Option<Filter> {
        let mut must_conditions: Vec<Condition> = vec![];
        let mut should_conditions: Vec<Condition> = vec![];

        // Memory type filter (must/AND)
        if let Some(ref memory_type) = filter.memory_type {
            must_conditions.push(Condition::matches("memory_type", memory_type.to_string()));
        }

        // Min importance filter (must/AND)
        if let Some(min_imp) = filter.min_importance {
            must_conditions.push(Condition::range(
                "importance",
                Range {
                    gte: Some(min_imp as f64),
                    ..Default::default()
                },
            ));
        }

        // Tags filter
        if !filter.tags.is_empty() {
            match filter.tags_match_mode {
                TagMatchMode::Any => {
                    // OR: any tag matches
                    for tag in &filter.tags {
                        should_conditions.push(Condition::matches("tags", tag.clone()));
                    }
                }
                TagMatchMode::All => {
                    // AND: all tags must match
                    for tag in &filter.tags {
                        must_conditions.push(Condition::matches("tags", tag.clone()));
                    }
                }
            }
        }

        // Return None if no conditions
        if must_conditions.is_empty() && should_conditions.is_empty() {
            return None;
        }

        // Build filter
        let mut filter_builder = Filter::default();

        if !must_conditions.is_empty() {
            filter_builder.must = must_conditions.into_iter().map(Into::into).collect();
        }

        if !should_conditions.is_empty() {
            filter_builder.should = should_conditions.into_iter().map(Into::into).collect();
            // When using should with must, we need at least 1 should to match
            // This is handled automatically by Qdrant when should is non-empty
        }

        Some(filter_builder)
    }
}
