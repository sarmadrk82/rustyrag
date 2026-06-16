//! YAML config loading and validation.

mod env;
mod pipeline;
mod rag;
mod registry;
mod validate;

pub use env::{load_dotenv, substitute_env};
pub use pipeline::{
    ChunkConfig, DistanceMetric, EmbedConfig, IndexConfig, ParseConfig, PipelineConfig,
    SourceConfig, StoreConfig,
};
pub use rag::RagConfig;
pub use registry::{adapter_registry, list_adapters, AdapterEntry};
pub use validate::{validate_pipeline, validate_rag, ValidationReport};

use rustyrag_core::{Error, Result};
use std::path::Path;

pub fn load_pipeline_config(path: &Path) -> Result<PipelineConfig> {
    let raw = std::fs::read_to_string(path)?;
    let substituted = substitute_env(&raw)?;
    let config: PipelineConfig = serde_yaml::from_str(&substituted)
        .map_err(|err| Error::Config(format!("invalid pipeline yaml: {err}")))?;
    validate_pipeline(&config)?;
    Ok(config)
}

pub fn load_rag_config(path: &Path) -> Result<RagConfig> {
    let raw = std::fs::read_to_string(path)?;
    let substituted = substitute_env(&raw)?;
    let config: RagConfig = serde_yaml::from_str(&substituted)
        .map_err(|err| Error::Config(format!("invalid rag yaml: {err}")))?;
    validate_rag(&config)?;
    Ok(config)
}
