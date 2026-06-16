//! Simple BM25 scoring for hybrid search reranking.

use std::collections::HashMap;

const K1: f32 = 1.2;
const B: f32 = 0.75;

/// Score documents against a query using BM25. Returns scores in input order.
pub fn bm25_scores(query: &str, documents: &[&str]) -> Vec<f32> {
    if documents.is_empty() {
        return Vec::new();
    }

    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return vec![0.0; documents.len()];
    }

    let doc_tokens: Vec<Vec<String>> = documents.iter().map(|d| tokenize(d)).collect();
    let avg_dl = doc_tokens.iter().map(|t| t.len() as f32).sum::<f32>() / documents.len() as f32;

    let mut df: HashMap<String, usize> = HashMap::new();
    for tokens in &doc_tokens {
        let unique: std::collections::HashSet<_> = tokens.iter().collect();
        for term in unique {
            *df.entry(term.clone()).or_default() += 1;
        }
    }

    let n = documents.len() as f32;
    doc_tokens
        .iter()
        .map(|tokens| {
            let dl = tokens.len() as f32;
            let mut score = 0.0_f32;
            for term in &query_terms {
                let tf = tokens.iter().filter(|t| *t == term).count() as f32;
                if tf == 0.0 {
                    continue;
                }
                let doc_freq = *df.get(term).unwrap_or(&0) as f32;
                let idf = ((n - doc_freq + 0.5) / (doc_freq + 0.5) + 1.0).ln();
                let tf_norm = (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avg_dl));
                score += idf * tf_norm;
            }
            score
        })
        .collect()
}

/// Normalize scores to 0.0–1.0 range.
pub fn normalize_scores(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let min = scores.iter().copied().fold(f32::INFINITY, f32::min);
    let range = max - min;
    if range <= f32::EPSILON {
        return vec![1.0; scores.len()];
    }
    scores.iter().map(|s| (s - min) / range).collect()
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1)
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_ranks_relevant_doc_higher() {
        let docs = [
            "How to run the pipeline with cargo",
            "Unrelated weather forecast for tomorrow",
        ];
        let scores = bm25_scores("run pipeline", &docs);
        assert!(scores[0] > scores[1]);
    }
}
