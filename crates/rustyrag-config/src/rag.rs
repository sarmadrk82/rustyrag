use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    pub version: u32,
    pub name: String,
    pub retrieval: RetrievalConfig,
    pub generation: GenerationConfig,
    pub context: ContextConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    pub collection: String,
    pub store_url: String,
    pub embed_model: String,
    pub top_k: usize,
    pub search_mode: String,
    pub rerank: RerankConfig,
    /// Weight for dense score in hybrid mode (0.0–1.0). BM25 weight = 1 - dense_weight.
    #[serde(default = "default_hybrid_dense_weight")]
    pub hybrid_dense_weight: f32,
    /// Base URL for Ollama embedder (when embed adapter is ollama).
    #[serde(default = "default_ollama_url")]
    pub embed_base_url: String,
    /// Embed adapter name for query-time embedding.
    #[serde(default = "default_embed_adapter")]
    pub embed_adapter: String,
}

fn default_hybrid_dense_weight() -> f32 {
    0.7
}

fn default_ollama_url() -> String {
    "http://localhost:11434".into()
}

fn default_embed_adapter() -> String {
    "openai".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankConfig {
    pub enabled: bool,
    pub adapter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub adapter: String,
    pub model: String,
    pub system_prompt: String,
    /// Base URL for Ollama (default: http://localhost:11434).
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub template: String,
}
