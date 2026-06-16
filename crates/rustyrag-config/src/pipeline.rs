use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub version: u32,
    pub name: String,
    pub source: SourceConfig,
    pub parse: ParseConfig,
    pub chunk: ChunkConfig,
    pub embed: EmbedConfig,
    pub store: StoreConfig,
    pub index: IndexConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub adapter: String,
    pub path: String,
    pub glob: String,
    /// Skip files larger than this (bytes). Default: 10 MiB.
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
}

fn default_max_file_size() -> u64 {
    10 * 1024 * 1024
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseConfig {
    pub adapter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkConfig {
    pub adapter: String,
    pub chunk_size: usize,
    #[serde(default)]
    pub chunk_overlap: usize,
    /// Cosine-similarity breakpoint for semantic chunking (0.0–1.0).
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,
}

fn default_similarity_threshold() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedConfig {
    pub adapter: String,
    pub model: String,
    pub batch_size: usize,
    /// Base URL for Ollama (default: http://localhost:11434).
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceMetric {
    Cosine,
    Euclid,
    Dot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    pub adapter: String,
    pub url: String,
    pub collection: String,
    pub distance: DistanceMetric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    pub idempotency_key: String,
    pub batch_upsert_size: usize,
    /// Reject documents that produce more chunks than this limit.
    #[serde(default = "default_max_chunks")]
    pub max_chunks_per_document: usize,
    /// Persist checkpoint after each document (enables resume on crash).
    #[serde(default = "default_checkpoint")]
    pub checkpoint: bool,
}

fn default_max_chunks() -> usize {
    10_000
}

fn default_checkpoint() -> bool {
    true
}
