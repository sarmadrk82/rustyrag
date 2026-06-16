use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IndexState {
    pub documents: HashMap<String, String>,
}

impl IndexState {
    pub fn path_for_pipeline(pipeline_name: &str) -> PathBuf {
        PathBuf::from(".rustyrag").join(format!("{pipeline_name}.json"))
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    pub fn is_unchanged(&self, source_uri: &str, content_hash: &str) -> bool {
        self.documents
            .get(source_uri)
            .is_some_and(|existing| existing == content_hash)
    }

    pub fn mark_indexed(&mut self, source_uri: impl Into<String>, content_hash: impl Into<String>) {
        self.documents
            .insert(source_uri.into(), content_hash.into());
    }
}
