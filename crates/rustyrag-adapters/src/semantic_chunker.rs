use async_trait::async_trait;
use rustyrag_core::{Document, EmbedderAdapter, Error, Result, Chunk, ChunkerAdapter};
use std::sync::Arc;

pub struct SemanticChunker {
    embedder: Arc<dyn EmbedderAdapter>,
    chunk_size: usize,
    similarity_threshold: f32,
}

impl SemanticChunker {
    pub fn new(
        embedder: Arc<dyn EmbedderAdapter>,
        chunk_size: usize,
        similarity_threshold: f32,
    ) -> Self {
        Self {
            embedder,
            chunk_size,
            similarity_threshold,
        }
    }
}

#[async_trait]
impl ChunkerAdapter for SemanticChunker {
    async fn chunk(&self, document: &Document) -> Result<Vec<Chunk>> {
        let sentences = split_sentences(&document.parsed_content);
        if sentences.is_empty() {
            return Ok(Vec::new());
        }
        if sentences.len() == 1 {
            return Ok(vec![Chunk {
                chunk_index: 0,
                content: sentences[0].clone(),
            }]);
        }

        let embeddings = self
            .embedder
            .embed(&sentences)
            .await
            .map_err(|err| Error::Adapter {
                adapter: "semantic".into(),
                message: format!("sentence embedding failed: {err}"),
            })?;

        let breakpoints = find_breakpoints(&embeddings, self.similarity_threshold);
        let segments = merge_segments(&sentences, &breakpoints, self.chunk_size);

        Ok(segments
            .into_iter()
            .enumerate()
            .map(|(chunk_index, content)| Chunk { chunk_index, content })
            .collect())
    }
}

fn split_sentences(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"(?<=[.!?])\s+|\n{2,}").expect("valid regex");
    re.split(text)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

fn find_breakpoints(embeddings: &[Vec<f32>], threshold: f32) -> Vec<usize> {
    let mut breakpoints = Vec::new();
    for i in 0..embeddings.len().saturating_sub(1) {
        let sim = cosine_similarity(&embeddings[i], &embeddings[i + 1]);
        if sim < threshold {
            breakpoints.push(i + 1);
        }
    }
    breakpoints
}

fn merge_segments(sentences: &[String], breakpoints: &[usize], max_size: usize) -> Vec<String> {
    let mut groups: Vec<Vec<&str>> = Vec::new();
    let mut start = 0;
    for &bp in breakpoints {
        groups.push(sentences[start..bp].iter().map(String::as_str).collect());
        start = bp;
    }
    groups.push(sentences[start..].iter().map(String::as_str).collect());

    let mut merged: Vec<String> = Vec::new();
    for group in groups {
        let mut current = String::new();
        for sentence in group {
            let candidate = if current.is_empty() {
                sentence.to_string()
            } else {
                format!("{current} {sentence}")
            };
            if candidate.len() > max_size && !current.is_empty() {
                merged.push(current);
                current = sentence.to_string();
            } else {
                current = candidate;
            }
        }
        if !current.is_empty() {
            merged.push(current);
        }
    }
    merged
}
