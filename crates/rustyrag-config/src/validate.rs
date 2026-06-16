use rustyrag_core::{Error, Result};

use crate::{PipelineConfig, RagConfig};

#[derive(Debug, Default)]
pub struct ValidationReport {
    pub pipeline_name: String,
    pub warnings: Vec<String>,
}

pub fn validate_pipeline(config: &PipelineConfig) -> Result<()> {
    if config.version != 1 {
        return Err(Error::Config(format!(
            "unsupported pipeline version {} (expected 1)",
            config.version
        )));
    }

    ensure_adapter("source", &config.source.adapter, &["filesystem"])?;
    ensure_adapter("parse", &config.parse.adapter, &["auto"])?;
    ensure_adapter("chunk", &config.chunk.adapter, &["recursive", "semantic"])?;
    ensure_adapter("embed", &config.embed.adapter, &["openai", "ollama"])?;
    ensure_adapter("store", &config.store.adapter, &["qdrant"])?;

    if config.chunk.adapter == "recursive" {
        if config.chunk.chunk_overlap >= config.chunk.chunk_size {
            return Err(Error::Config(
                "chunk_overlap must be smaller than chunk_size for recursive chunker".into(),
            ));
        }
    }

    if config.chunk.adapter == "semantic" {
        if !(0.0..=1.0).contains(&config.chunk.similarity_threshold) {
            return Err(Error::Config(
                "chunk.similarity_threshold must be between 0.0 and 1.0".into(),
            ));
        }
    }

    if config.index.idempotency_key != "content_hash" {
        return Err(Error::Config(
            "only idempotency_key=content_hash is supported".into(),
        ));
    }

    Ok(())
}

pub fn validate_rag(config: &RagConfig) -> Result<()> {
    if config.version != 1 {
        return Err(Error::Config(format!(
            "unsupported rag version {} (expected 1)",
            config.version
        )));
    }

    ensure_adapter("generation", &config.generation.adapter, &["openai", "ollama"])?;
    ensure_adapter(
        "retrieval.embed",
        &config.retrieval.embed_adapter,
        &["openai", "ollama"],
    )?;
    ensure_adapter(
        "retrieval.search_mode",
        &config.retrieval.search_mode,
        &["semantic", "hybrid"],
    )?;

    if config.retrieval.search_mode == "hybrid"
        && !(0.0..=1.0).contains(&config.retrieval.hybrid_dense_weight)
    {
        return Err(Error::Config(
            "retrieval.hybrid_dense_weight must be between 0.0 and 1.0".into(),
        ));
    }

    Ok(())
}

fn ensure_adapter(stage: &str, actual: &str, allowed: &[&str]) -> Result<()> {
    if allowed.contains(&actual) {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "{stage}.adapter `{actual}` is not supported (allowed: {})",
            allowed.join(", ")
        )))
    }
}
