use async_trait::async_trait;
use glob::glob;
use rustyrag_core::{Error, RawDocument, Result, SourceAdapter};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::warn;

pub struct FilesystemSource {
    base_path: PathBuf,
    pattern: String,
    max_file_size_bytes: u64,
}

impl FilesystemSource {
    pub fn new(
        base_path: impl Into<PathBuf>,
        pattern: impl Into<String>,
        max_file_size_bytes: u64,
    ) -> Self {
        Self {
            base_path: base_path.into(),
            pattern: pattern.into(),
            max_file_size_bytes,
        }
    }
}

#[async_trait]
impl SourceAdapter for FilesystemSource {
    async fn read(&self) -> Result<Vec<RawDocument>> {
        let search = self.base_path.join(&self.pattern);
        let search = search.to_string_lossy().into_owned();

        let mut documents = Vec::new();
        for entry in glob(&search).map_err(|err| adapter_err("filesystem", err.to_string()))? {
            let path = entry.map_err(|err| adapter_err("filesystem", err.to_string()))?;
            if !path.is_file() {
                continue;
            }

            if !is_supported_doc(&path) {
                continue;
            }

            let metadata = std::fs::metadata(&path)?;
            if metadata.len() > self.max_file_size_bytes {
                warn!(
                    path = %path.display(),
                    size = metadata.len(),
                    limit = self.max_file_size_bytes,
                    "skipping file exceeding max_file_size_bytes"
                );
                continue;
            }

            let (content, content_hash) = read_file_content(&path)?;
            let uri = path.to_string_lossy().into_owned();
            let title = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| uri.clone());

            documents.push(RawDocument {
                id: content_hash.clone(),
                uri,
                title,
                content,
                content_hash,
            });
        }

        documents.sort_by(|a, b| a.uri.cmp(&b.uri));
        Ok(documents)
    }
}

fn read_file_content(path: &Path) -> Result<(String, String)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some("pdf") => {
            let bytes = std::fs::read(path)?;
            let content_hash = hex::encode(Sha256::digest(&bytes));
            Ok((format!("__pdf__:{path}", path = path.display()), content_hash))
        }
        _ => {
            let content = std::fs::read_to_string(path)?;
            Ok((content.clone(), hash_content(&content)))
        }
    }
}

fn is_supported_doc(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md") | Some("txt") | Some("html") | Some("htm") | Some("pdf")
    )
}

pub fn hash_content(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    hex::encode(digest)
}

/// Stable numeric point ID for Qdrant (string IDs must be UUIDs; we use a hash instead).
pub fn chunk_point_id(source_uri: &str, chunk_index: usize) -> u64 {
    let key = format!("{source_uri}:{chunk_index}");
    let digest = Sha256::digest(key.as_bytes());
    u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
}

fn adapter_err(adapter: &str, message: String) -> Error {
    Error::Adapter {
        adapter: adapter.into(),
        message,
    }
}

pub fn resolve_path(_base: &Path, relative: &str) -> PathBuf {
    let path = PathBuf::from(relative);
    if path.is_absolute() {
        path
    } else {
        // Paths in YAML are relative to where you run the CLI (project root).
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}
