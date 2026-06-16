//! Shared types and adapter traits for RustyRAG.

pub mod document;
pub mod error;
pub mod traits;

pub use document::{Chunk, ChunkRecord, Document, RawDocument, RetrievedChunk};
pub use error::{Error, Result};
pub use traits::{
    ChunkerAdapter, EmbedderAdapter, LlmAdapter, ParserAdapter, SourceAdapter, VectorStoreAdapter,
};
