use async_trait::async_trait;

use crate::{Chunk, Document, RawDocument, Result};

/// Reads raw documents from a source (filesystem, S3, etc.).
#[async_trait]
pub trait SourceAdapter: Send + Sync {
    async fn read(&self) -> Result<Vec<RawDocument>>;
}

/// Converts raw file bytes/text into normalized document text.
#[async_trait]
pub trait ParserAdapter: Send + Sync {
    async fn parse(&self, raw: &RawDocument) -> Result<Document>;
}

/// Splits a document into chunks. Recursive in v0; semantic in a later release.
#[async_trait]
pub trait ChunkerAdapter: Send + Sync {
    async fn chunk(&self, document: &Document) -> Result<Vec<Chunk>>;
}

/// Turns text into embedding vectors.
#[async_trait]
pub trait EmbedderAdapter: Send + Sync {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// Writes chunk vectors to a vector database.
#[async_trait]
pub trait VectorStoreAdapter: Send + Sync {
    async fn ensure_collection(&self, vector_size: u64) -> Result<()>;

    async fn delete_by_source_uri(&self, source_uri: &str) -> Result<()>;

    async fn upsert(&self, records: &[crate::ChunkRecord]) -> Result<()>;

    async fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<crate::RetrievedChunk>>;
}

/// Generates a natural-language answer from a prompt.
#[async_trait]
pub trait LlmAdapter: Send + Sync {
    async fn complete(&self, system_prompt: &str, user_prompt: &str) -> Result<String>;

    /// Stream tokens as they arrive. Default falls back to a single complete() call.
    async fn complete_stream(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<tokio_stream::wrappers::ReceiverStream<Result<String>>> {
        let text = self.complete(system_prompt, user_prompt).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx.send(Ok(text)).await;
        Ok(tokio_stream::wrappers::ReceiverStream::new(rx))
    }
}
