use serde::{Deserialize, Serialize};

/// Metadata for one adapter option, consumed by CLI/GUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterEntry {
    pub stage: String,
    pub name: String,
    pub label: String,
    pub description: String,
}

/// All built-in adapters. New adapters register here for GUI/CLI discovery.
pub fn adapter_registry() -> Vec<AdapterEntry> {
    vec![
        // Source
        AdapterEntry {
            stage: "source".into(),
            name: "filesystem".into(),
            label: "Local filesystem".into(),
            description: "Read files from a local directory via glob pattern.".into(),
        },
        // Parse
        AdapterEntry {
            stage: "parse".into(),
            name: "auto".into(),
            label: "Auto-detect".into(),
            description: "Plain text and markdown as-is; PDF/HTML when extension matches.".into(),
        },
        // Chunk
        AdapterEntry {
            stage: "chunk".into(),
            name: "recursive".into(),
            label: "Recursive split".into(),
            description: "Split by size with sentence/paragraph boundaries. Fast, no extra API cost.".into(),
        },
        AdapterEntry {
            stage: "chunk".into(),
            name: "semantic".into(),
            label: "Semantic split".into(),
            description: "Embed sentences and split where topic shifts. Better coherence; uses embed API at ingest.".into(),
        },
        // Embed
        AdapterEntry {
            stage: "embed".into(),
            name: "openai".into(),
            label: "OpenAI".into(),
            description: "OpenAI embedding models (text-embedding-3-small, etc.).".into(),
        },
        AdapterEntry {
            stage: "embed".into(),
            name: "ollama".into(),
            label: "Ollama".into(),
            description: "Local embeddings via Ollama (nomic-embed-text, etc.).".into(),
        },
        // Store
        AdapterEntry {
            stage: "store".into(),
            name: "qdrant".into(),
            label: "Qdrant".into(),
            description: "Qdrant vector database.".into(),
        },
        // LLM / generation
        AdapterEntry {
            stage: "generation".into(),
            name: "openai".into(),
            label: "OpenAI".into(),
            description: "OpenAI chat models (gpt-4o-mini, etc.).".into(),
        },
        AdapterEntry {
            stage: "generation".into(),
            name: "ollama".into(),
            label: "Ollama".into(),
            description: "Local LLM via Ollama.".into(),
        },
        // Search modes
        AdapterEntry {
            stage: "search_mode".into(),
            name: "semantic".into(),
            label: "Semantic (dense)".into(),
            description: "Vector similarity search only.".into(),
        },
        AdapterEntry {
            stage: "search_mode".into(),
            name: "hybrid".into(),
            label: "Hybrid (dense + BM25)".into(),
            description: "Combine vector search with keyword BM25 reranking.".into(),
        },
    ]
}

pub fn list_adapters(stage: Option<&str>) -> Vec<AdapterEntry> {
    adapter_registry()
        .into_iter()
        .filter(|entry| stage.is_none_or(|s| entry.stage == s))
        .collect()
}
