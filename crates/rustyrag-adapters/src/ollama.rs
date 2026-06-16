use async_trait::async_trait;
use futures_util::StreamExt;
use rustyrag_core::{EmbedderAdapter, Error, LlmAdapter, Result};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub struct OllamaEmbedder {
    client: reqwest::Client,
    model: String,
    base_url: String,
}

impl OllamaEmbedder {
    pub fn new(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[async_trait]
impl EmbedderAdapter for OllamaEmbedder {
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
            .post(format!("{}/api/embed", self.base_url))
            .json(&Request {
                model: &self.model,
                input: texts,
            })
            .send()
            .await
            .map_err(|err| embed_err(err.to_string()))?
            .error_for_status()
            .map_err(|err| embed_err(err.to_string()))?
            .json::<EmbedResponse>()
            .await
            .map_err(|err| embed_err(err.to_string()))?;

        Ok(response.embeddings)
    }
}

pub struct OllamaLlm {
    client: reqwest::Client,
    model: String,
    base_url: String,
}

impl OllamaLlm {
    pub fn new(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    message: StreamMessage,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct StreamMessage {
    content: String,
}

#[async_trait]
impl LlmAdapter for OllamaLlm {
    async fn complete(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        #[derive(serde::Serialize)]
        struct Request<'a> {
            model: &'a str,
            messages: [Message<'a>; 2],
            stream: bool,
        }

        #[derive(serde::Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&Request {
                model: &self.model,
                messages: [
                    Message {
                        role: "system",
                        content: system_prompt,
                    },
                    Message {
                        role: "user",
                        content: user_prompt,
                    },
                ],
                stream: false,
            })
            .send()
            .await
            .map_err(|err| llm_err(err.to_string()))?
            .error_for_status()
            .map_err(|err| llm_err(err.to_string()))?
            .json::<ChatResponse>()
            .await
            .map_err(|err| llm_err(err.to_string()))?;

        Ok(response.message.content)
    }

    async fn complete_stream(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<ReceiverStream<Result<String>>> {
        #[derive(serde::Serialize)]
        struct Request<'a> {
            model: &'a str,
            messages: [Message<'a>; 2],
            stream: bool,
        }

        #[derive(serde::Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&Request {
                model: &self.model,
                messages: [
                    Message {
                        role: "system",
                        content: system_prompt,
                    },
                    Message {
                        role: "user",
                        content: user_prompt,
                    },
                ],
                stream: true,
            })
            .send()
            .await
            .map_err(|err| llm_err(err.to_string()))?
            .error_for_status()
            .map_err(|err| llm_err(err.to_string()))?;

        let (tx, rx) = mpsc::channel(32);
        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();
            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(err) => {
                        let _ = tx.send(Err(llm_err(err.to_string()))).await;
                        return;
                    }
                };
                buffer.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();
                    if line.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<StreamChunk>(&line) {
                        Ok(chunk) => {
                            if !chunk.message.content.is_empty() {
                                if tx.send(Ok(chunk.message.content)).await.is_err() {
                                    return;
                                }
                            }
                            if chunk.done {
                                return;
                            }
                        }
                        Err(err) => {
                            let _ = tx
                                .send(Err(llm_err(format!("stream parse error: {err}"))))
                                .await;
                            return;
                        }
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}

fn embed_err(message: String) -> Error {
    Error::Adapter {
        adapter: "ollama".into(),
        message,
    }
}

fn llm_err(message: String) -> Error {
    Error::Adapter {
        adapter: "ollama".into(),
        message,
    }
}
