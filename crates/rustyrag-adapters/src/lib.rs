//! Built-in adapter implementations for RustyRAG v0.

mod bm25;
mod chunker;
mod embedder;
mod factory;
mod llm;
mod ollama;
mod parser;
mod semantic_chunker;
mod source;
mod store;

pub use factory::{
    build_adapters, build_rag_service, build_read_adapters, estimate_embed_cost_usd,
    PipelineAdapters, RagService, ReadStageAdapters,
};
pub use source::chunk_point_id;
