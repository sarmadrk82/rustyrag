use rustyrag_config::{PipelineConfig, RagConfig};
use rustyrag_core::{
    ChunkerAdapter, EmbedderAdapter, LlmAdapter, ParserAdapter, Result, RetrievedChunk,
    SourceAdapter, VectorStoreAdapter,
};
use std::path::Path;
use std::sync::Arc;

use crate::bm25::{bm25_scores, normalize_scores};
use crate::chunker::RecursiveChunker;
use crate::embedder::OpenAiEmbedder;
use crate::llm::OpenAiLlm;
use crate::ollama::{OllamaEmbedder, OllamaLlm};
use crate::parser::AutoParser;
use crate::semantic_chunker::SemanticChunker;
use crate::source::{resolve_path, FilesystemSource};
use crate::store::QdrantStore;
use rustyrag_config::DistanceMetric;

pub struct PipelineAdapters {
    pub source: Arc<dyn SourceAdapter>,
    pub parser: Arc<dyn ParserAdapter>,
    pub chunker: Arc<dyn ChunkerAdapter>,
    pub embedder: Arc<dyn EmbedderAdapter>,
    pub store: Arc<dyn VectorStoreAdapter>,
}

pub struct ReadStageAdapters {
    pub source: Arc<dyn SourceAdapter>,
    pub parser: Arc<dyn ParserAdapter>,
    pub chunker: Arc<dyn ChunkerAdapter>,
}

pub fn build_embedder(config: &PipelineConfig) -> Result<Arc<dyn EmbedderAdapter>> {
    match config.embed.adapter.as_str() {
        "openai" => Ok(Arc::new(OpenAiEmbedder::new(config.embed.model.clone())?)),
        "ollama" => Ok(Arc::new(OllamaEmbedder::new(
            config.embed.model.clone(),
            config.embed.base_url.clone(),
        ))),
        other => Err(rustyrag_core::Error::Config(format!(
            "unsupported embed adapter `{other}`"
        ))),
    }
}

pub fn build_chunker(
    config: &PipelineConfig,
    embedder: Arc<dyn EmbedderAdapter>,
) -> Result<Arc<dyn ChunkerAdapter>> {
    match config.chunk.adapter.as_str() {
        "recursive" => Ok(Arc::new(RecursiveChunker::new(
            config.chunk.chunk_size,
            config.chunk.chunk_overlap,
        ))),
        "semantic" => Ok(Arc::new(SemanticChunker::new(
            embedder,
            config.chunk.chunk_size,
            config.chunk.similarity_threshold,
        ))),
        other => Err(rustyrag_core::Error::Config(format!(
            "unsupported chunk adapter `{other}`"
        ))),
    }
}

pub fn build_read_adapters(
    config: &PipelineConfig,
    config_path: &Path,
) -> Result<ReadStageAdapters> {
    let config_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    let source_base = resolve_path(config_dir, &config.source.path);
    let embedder = build_embedder(config)?;

    Ok(ReadStageAdapters {
        source: Arc::new(FilesystemSource::new(
            source_base,
            config.source.glob.clone(),
            config.source.max_file_size_bytes,
        )),
        parser: Arc::new(AutoParser),
        chunker: build_chunker(config, embedder)?,
    })
}

pub fn build_adapters(config: &PipelineConfig, config_path: &Path) -> Result<PipelineAdapters> {
    let embedder = build_embedder(config)?;
    let read = build_read_adapters(config, config_path)?;

    let store: Arc<dyn VectorStoreAdapter> = Arc::new(QdrantStore::new(
        config.store.url.clone(),
        config.store.collection.clone(),
        config.store.distance.clone(),
    )?);

    Ok(PipelineAdapters {
        source: read.source,
        parser: read.parser,
        chunker: read.chunker,
        embedder,
        store,
    })
}

pub struct RagService {
    pub config: RagConfig,
    pub embedder: Arc<dyn EmbedderAdapter>,
    pub store: Arc<dyn VectorStoreAdapter>,
    pub llm: Arc<dyn LlmAdapter>,
}

impl RagService {
    pub async fn retrieve(&self, query: &str) -> Result<Vec<RetrievedChunk>> {
        let vectors = self.embedder.embed(&[query.to_string()]).await?;
        let query_vector = vectors
            .into_iter()
            .next()
            .ok_or_else(|| rustyrag_core::Error::Adapter {
                adapter: "embedder".into(),
                message: "embedder returned no vector for query".into(),
            })?;

        let fetch_k = match self.config.retrieval.search_mode.as_str() {
            "hybrid" => self.config.retrieval.top_k * 3,
            _ => self.config.retrieval.top_k,
        };

        let mut chunks = self.store.search(&query_vector, fetch_k).await?;

        if self.config.retrieval.search_mode == "hybrid" {
            chunks = hybrid_rerank(query, chunks, self.config.retrieval.top_k, self.config.retrieval.hybrid_dense_weight);
        }

        Ok(chunks)
    }

    pub fn build_context(&self, chunks: &[RetrievedChunk]) -> String {
        let max_chars = self.config.context.max_tokens.saturating_mul(4);
        let mut context = String::new();

        for chunk in chunks {
            let entry = format!(
                "[source: {} | chunk {}]\n{}\n\n",
                chunk.title, chunk.chunk_index, chunk.content
            );
            if context.len() + entry.len() > max_chars {
                break;
            }
            context.push_str(&entry);
        }

        context
    }

    pub async fn query(&self, question: &str) -> Result<(String, Vec<RetrievedChunk>)> {
        let chunks = self.retrieve(question).await?;
        let context = self.build_context(&chunks);
        let user_prompt = format!(
            "Use the context below to answer the question.\n\nContext:\n{context}\n\nQuestion:\n{question}"
        );
        let answer = self
            .llm
            .complete(&self.config.generation.system_prompt, &user_prompt)
            .await?;

        Ok((answer, chunks))
    }

    pub async fn query_stream(
        &self,
        question: &str,
    ) -> Result<(tokio_stream::wrappers::ReceiverStream<Result<String>>, Vec<RetrievedChunk>)> {
        let chunks = self.retrieve(question).await?;
        let context = self.build_context(&chunks);
        let user_prompt = format!(
            "Use the context below to answer the question.\n\nContext:\n{context}\n\nQuestion:\n{question}"
        );
        let stream = self
            .llm
            .complete_stream(&self.config.generation.system_prompt, &user_prompt)
            .await?;

        Ok((stream, chunks))
    }
}

fn hybrid_rerank(
    query: &str,
    mut chunks: Vec<RetrievedChunk>,
    top_k: usize,
    dense_weight: f32,
) -> Vec<RetrievedChunk> {
    if chunks.is_empty() {
        return chunks;
    }

    let contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
    let bm25_raw = bm25_scores(query, &contents);
    let bm25_norm = normalize_scores(&bm25_raw);

    let dense_norm = normalize_scores(&chunks.iter().map(|c| c.score).collect::<Vec<_>>());
    let bm25_weight = 1.0 - dense_weight;

    for (chunk, (&dense, &bm25)) in chunks.iter_mut().zip(dense_norm.iter().zip(bm25_norm.iter())) {
        chunk.score = dense_weight * dense + bm25_weight * bm25;
    }

    chunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    chunks.truncate(top_k);
    chunks
}

pub fn build_rag_embedder(config: &RagConfig) -> Result<Arc<dyn EmbedderAdapter>> {
    match config.retrieval.embed_adapter.as_str() {
        "openai" => Ok(Arc::new(OpenAiEmbedder::new(
            config.retrieval.embed_model.clone(),
        )?)),
        "ollama" => Ok(Arc::new(OllamaEmbedder::new(
            config.retrieval.embed_model.clone(),
            config.retrieval.embed_base_url.clone(),
        ))),
        other => Err(rustyrag_core::Error::Config(format!(
            "unsupported retrieval embed adapter `{other}`"
        ))),
    }
}

pub fn build_rag_llm(config: &RagConfig) -> Result<Arc<dyn LlmAdapter>> {
    match config.generation.adapter.as_str() {
        "openai" => Ok(Arc::new(OpenAiLlm::new(config.generation.model.clone())?)),
        "ollama" => Ok(Arc::new(OllamaLlm::new(
            config.generation.model.clone(),
            config.generation.base_url.clone(),
        ))),
        other => Err(rustyrag_core::Error::Config(format!(
            "unsupported generation adapter `{other}`"
        ))),
    }
}

pub fn build_rag_service(config: RagConfig) -> Result<RagService> {
    let embedder = build_rag_embedder(&config)?;
    let store: Arc<dyn VectorStoreAdapter> = Arc::new(QdrantStore::new(
        config.retrieval.store_url.clone(),
        config.retrieval.collection.clone(),
        DistanceMetric::Cosine,
    )?);
    let llm = build_rag_llm(&config)?;

    Ok(RagService {
        config,
        embedder,
        store,
        llm,
    })
}

/// Rough embed cost estimate (USD) for dry-run reporting.
pub fn estimate_embed_cost_usd(model: &str, total_chars: usize) -> f64 {
    let tokens = (total_chars / 4).max(1) as f64;
    let per_1k: f64 = match model {
        "text-embedding-3-small" => 0.00002,
        "text-embedding-3-large" => 0.00013,
        "text-embedding-ada-002" => 0.0001,
        _ if model.contains("nomic") || model.contains("ollama") => 0.0,
        _ => 0.00002,
    };
    tokens / 1000.0 * per_1k
}
