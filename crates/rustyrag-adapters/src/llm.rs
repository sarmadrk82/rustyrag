use async_trait::async_trait;
use futures_util::StreamExt;
use rustyrag_core::{Error, LlmAdapter, Result};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub struct OpenAiLlm {
    client: reqwest::Client,
    model: String,
    api_key: String,
}

impl OpenAiLlm {
    pub fn new(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            Error::Config(
                "OPENAI_API_KEY must be set for openai generation adapter. \
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
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct StreamDeltaResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: String,
}

#[async_trait]
impl LlmAdapter for OpenAiLlm {
    async fn complete(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        #[derive(serde::Serialize)]
        struct Request<'a> {
            model: &'a str,
            messages: [Message<'a>; 2],
        }

        #[derive(serde::Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
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
            })
            .send()
            .await
            .map_err(|err| adapter_err(err.to_string()))?
            .error_for_status()
            .map_err(|err| adapter_err(err.to_string()))?
            .json::<ChatResponse>()
            .await
            .map_err(|err| adapter_err(err.to_string()))?;

        response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| adapter_err("openai returned no choices".into()))
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
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
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
            .map_err(|err| adapter_err(err.to_string()))?
            .error_for_status()
            .map_err(|err| adapter_err(err.to_string()))?;

        let (tx, rx) = mpsc::channel(32);
        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();
            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(err) => {
                        let _ = tx.send(Err(adapter_err(err.to_string()))).await;
                        return;
                    }
                };
                buffer.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();
                    if !line.starts_with("data: ") {
                        continue;
                    }
                    let data = line.trim_start_matches("data: ").trim();
                    if data == "[DONE]" {
                        return;
                    }
                    match serde_json::from_str::<StreamDeltaResponse>(data) {
                        Ok(parsed) => {
                            for choice in parsed.choices {
                                if !choice.delta.content.is_empty() {
                                    if tx.send(Ok(choice.delta.content)).await.is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}

fn adapter_err(message: String) -> Error {
    Error::Adapter {
        adapter: "openai".into(),
        message,
    }
}
