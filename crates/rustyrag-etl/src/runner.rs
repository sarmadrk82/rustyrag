use anyhow::{Context, Result};
use rustyrag_adapters::chunk_point_id;
use rustyrag_adapters::build_adapters;
use rustyrag_adapters::estimate_embed_cost_usd;
use rustyrag_config::PipelineConfig;
use rustyrag_core::{ChunkRecord, Document, Error};
use tracing::{info, warn};

use crate::state::IndexState;

pub struct PipelineRunner {
    config: PipelineConfig,
    config_path: std::path::PathBuf,
}

#[derive(Debug, Default)]
pub struct EtlReport {
    pub documents_seen: usize,
    pub documents_skipped: usize,
    pub documents_indexed: usize,
    pub chunks_written: usize,
}

#[derive(Debug, Default)]
pub struct DryRunReport {
    pub documents_seen: usize,
    pub documents_skipped: usize,
    pub documents_to_index: usize,
    pub chunks_total: usize,
    pub total_chars: usize,
    pub estimated_embed_cost_usd: f64,
    pub warnings: Vec<String>,
}

impl PipelineRunner {
    pub fn new(config: PipelineConfig, config_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            config,
            config_path: config_path.into(),
        }
    }

    pub async fn run(&self) -> Result<EtlReport> {
        let adapters = build_adapters(&self.config, &self.config_path)
            .context("failed to build pipeline adapters")?;

        let state_path = IndexState::path_for_pipeline(&self.config.name);
        let mut state = IndexState::load(&state_path).context("failed to load index state")?;

        let raw_docs = adapters.source.read().await.context("source read failed")?;
        let mut report = EtlReport {
            documents_seen: raw_docs.len(),
            ..Default::default()
        };

        let mut pending: Vec<(Document, Vec<rustyrag_core::Chunk>)> = Vec::new();

        for raw in raw_docs {
            if state.is_unchanged(&raw.uri, &raw.content_hash) {
                report.documents_skipped += 1;
                continue;
            }

            let document = adapters
                .parser
                .parse(&raw)
                .await
                .with_context(|| format!("parse failed for {}", raw.uri))?;
            let chunks = adapters
                .chunker
                .chunk(&document)
                .await
                .with_context(|| format!("chunk failed for {}", raw.uri))?;

            if chunks.len() > self.config.index.max_chunks_per_document {
                return Err(Error::Config(format!(
                    "document `{}` produced {} chunks (limit: {})",
                    raw.uri,
                    chunks.len(),
                    self.config.index.max_chunks_per_document
                ))
                .into());
            }

            pending.push((document, chunks));
        }

        if pending.is_empty() {
            info!("nothing to index");
            return Ok(report);
        }

        for (document, chunks) in &pending {
            let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
            let vectors = embed_in_batches(
                adapters.embedder.as_ref(),
                &texts,
                self.config.embed.batch_size,
            )
            .await
            .with_context(|| format!("embed failed for {}", document.raw.uri))?;

            if let Some(first) = vectors.first() {
                adapters
                    .store
                    .ensure_collection(first.len() as u64)
                    .await
                    .context("failed to ensure qdrant collection")?;
            }

            adapters
                .store
                .delete_by_source_uri(&document.raw.uri)
                .await
                .with_context(|| format!("delete failed for {}", document.raw.uri))?;

            let mut records = Vec::with_capacity(chunks.len());
            for (chunk, vector) in chunks.iter().zip(vectors) {
                records.push(ChunkRecord {
                    point_id: chunk_point_id(&document.raw.uri, chunk.chunk_index),
                    vector,
                    source_id: document.raw.id.clone(),
                    source_uri: document.raw.uri.clone(),
                    title: document.raw.title.clone(),
                    chunk_index: chunk.chunk_index,
                    content: chunk.content.clone(),
                    content_hash: document.raw.content_hash.clone(),
                });
            }

            for batch in records.chunks(self.config.index.batch_upsert_size) {
                adapters
                    .store
                    .upsert(batch)
                    .await
                    .context("qdrant upsert failed")?;
                report.chunks_written += batch.len();
            }

            report.documents_indexed += 1;

            if self.config.index.checkpoint {
                state.mark_indexed(document.raw.uri.clone(), document.raw.content_hash.clone());
                state
                    .save(&state_path)
                    .context("failed to save checkpoint")?;
            }
        }

        if !self.config.index.checkpoint {
            for (document, _) in &pending {
                state.mark_indexed(document.raw.uri.clone(), document.raw.content_hash.clone());
            }
            state.save(&state_path).context("failed to save index state")?;
        }

        Ok(report)
    }

    pub async fn dry_run(&self) -> Result<DryRunReport> {
        let adapters = rustyrag_adapters::build_read_adapters(&self.config, &self.config_path)
            .context("failed to build read-stage adapters")?;

        let state_path = IndexState::path_for_pipeline(&self.config.name);
        let state = IndexState::load(&state_path).context("failed to load index state")?;

        let raw_docs = adapters.source.read().await.context("source read failed")?;
        let mut report = DryRunReport {
            documents_seen: raw_docs.len(),
            ..Default::default()
        };

        if self.config.chunk.adapter == "semantic" {
            report.warnings.push(
                "semantic chunking calls the embed API during dry-run to compute breakpoints"
                    .into(),
            );
        }

        for raw in raw_docs {
            if state.is_unchanged(&raw.uri, &raw.content_hash) {
                report.documents_skipped += 1;
                continue;
            }

            let document = adapters.parser.parse(&raw).await?;
            let chunks = adapters.chunker.chunk(&document).await?;

            if chunks.len() > self.config.index.max_chunks_per_document {
                warn!(
                    uri = %raw.uri,
                    chunks = chunks.len(),
                    limit = self.config.index.max_chunks_per_document,
                    "document exceeds max_chunks_per_document"
                );
                report.warnings.push(format!(
                    "{} exceeds max_chunks_per_document ({} > {})",
                    raw.uri,
                    chunks.len(),
                    self.config.index.max_chunks_per_document
                ));
                continue;
            }

            report.documents_to_index += 1;
            report.chunks_total += chunks.len();
            report.total_chars += chunks.iter().map(|c| c.content.len()).sum::<usize>();
        }

        report.estimated_embed_cost_usd =
            estimate_embed_cost_usd(&self.config.embed.model, report.total_chars);

        Ok(report)
    }
}

async fn embed_in_batches(
    embedder: &dyn rustyrag_core::EmbedderAdapter,
    texts: &[String],
    batch_size: usize,
) -> Result<Vec<Vec<f32>>> {
    let mut all = Vec::with_capacity(texts.len());
    for batch in texts.chunks(batch_size.max(1)) {
        let vectors = embedder.embed(batch).await?;
        all.extend(vectors);
    }
    Ok(all)
}
