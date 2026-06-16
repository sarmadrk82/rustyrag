use async_trait::async_trait;
use rustyrag_core::{Chunk, ChunkerAdapter, Document, Error, Result};
use text_splitter::{ChunkConfig as SplitterConfig, TextSplitter};

pub struct RecursiveChunker {
    chunk_size: usize,
    chunk_overlap: usize,
}

impl RecursiveChunker {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
        }
    }
}

#[async_trait]
impl ChunkerAdapter for RecursiveChunker {
    async fn chunk(&self, document: &Document) -> Result<Vec<Chunk>> {
        if self.chunk_overlap >= self.chunk_size {
            return Err(Error::Config(
                "chunk_overlap must be smaller than chunk_size".into(),
            ));
        }

        let splitter_config = SplitterConfig::new(self.chunk_size)
            .with_overlap(self.chunk_overlap)
            .map_err(|err| Error::Config(err.to_string()))?;
        let splitter = TextSplitter::new(splitter_config);

        let parts: Vec<String> = splitter
            .chunks(&document.parsed_content)
            .map(str::to_owned)
            .collect();

        Ok(parts
            .into_iter()
            .enumerate()
            .map(|(chunk_index, content)| Chunk { chunk_index, content })
            .collect())
    }
}
