use async_trait::async_trait;
use rustyrag_core::{EmbedderAdapter, Error, Result};
use serde::Deserialize;

pub struct OpenAiEmbedder {
    client: reqwest::Client,
    model: String,
    api_key: String,
}

impl OpenAiEmbedder {
    pub fn new(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            Error::Config(
                "OPENAI_API_KEY must be set for openai embed adapter. \
                 Add it to `.env` in the project root or export it in your shell."
                    .into(),
            )
        })?;

        Ok(Self {
            client: reqwest::Client::new(),
            model: model.into(),
            api_key,
        })
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[async_trait]
impl EmbedderAdapter for OpenAiEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        #[derive(serde::Serialize)]
        struct Request<'a> {
            model: &'a str,
            input: &'a [String],
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&Request {
                model: &self.model,
                input: texts,
            })
            .send()
            .await
            .map_err(|err| adapter_err(err.to_string()))?
            .error_for_status()
            .map_err(|err| adapter_err(err.to_string()))?
            .json::<EmbeddingsResponse>()
            .await
            .map_err(|err| adapter_err(err.to_string()))?;

        Ok(response.data.into_iter().map(|row| row.embedding).collect())
    }
}

fn adapter_err(message: String) -> Error {
    Error::Adapter {
        adapter: "openai".into(),
        message,
    }
}
