use serde::{Deserialize, Serialize};

/// A file read from disk before parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDocument {
    pub id: String,
    pub uri: String,
    pub title: String,
    pub content: String,
    pub content_hash: String,
}

/// A document after the parse stage.
#[derive(Debug, Clone)]
pub struct Document {
    pub raw: RawDocument,
    pub parsed_content: String,
}

/// One text chunk from a document.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub chunk_index: usize,
    pub content: String,
}

/// A chunk returned from vector search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub score: f32,
    pub source_uri: String,
    pub title: String,
    pub chunk_index: usize,
    pub content: String,
}

/// A chunk ready to upsert into the vector store.
#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub point_id: u64,
    pub vector: Vec<f32>,
    pub source_id: String,
    pub source_uri: String,
    pub title: String,
    pub chunk_index: usize,
    pub content: String,
    pub content_hash: String,
}
