use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use std::fmt;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

use crate::models::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, GatewayErrorResponse,
    HealthResponse, ModelListResponse,
};

#[derive(Debug)]
pub enum ClientError {
    InvalidUrl(String),
    Network(reqwest::Error),
    Gateway {
        status: StatusCode,
        error_type: String,
        message: String,
    },
    Decoding(String),
}

impl std::error::Error for ClientError {}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::InvalidUrl(msg) => write!(f, "Invalid gateway URL: {}", msg),
            ClientError::Network(err) => write!(f, "Network error: {}", err),
            ClientError::Gateway {
                status,
                error_type,
                message,
            } => write!(f, "Gateway error ({status}, type: {error_type}): {message}"),
            ClientError::Decoding(msg) => write!(f, "Response decoding error: {}", msg),
        }
    }
}

impl From<reqwest::Error> for ClientError {
    fn from(err: reqwest::Error) -> Self {
        ClientError::Network(err)
    }
}

/// Events emitted during streaming chat completions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Delta(String),
    Error(String),
    Done,
}

#[derive(Clone, Debug)]
pub struct GatewayClient {
    base_url: String,
    http: Client,
}

impl GatewayClient {
    pub const DEFAULT_BASE_URL: &'static str = "http://127.0.0.1:9471";

    pub fn new(base_url: impl Into<String>) -> Result<Self, ClientError> {
        let base_url = base_url.into();
        let trimmed = base_url.trim().trim_end_matches('/');
        if trimmed.is_empty()
            || (!trimmed.starts_with("http://") && !trimmed.starts_with("https://"))
        {
            return Err(ClientError::InvalidUrl(
                "Base URL must start with http:// or https://".to_string(),
            ));
        }

        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(ClientError::Network)?;

        Ok(Self {
            base_url: trimmed.to_string(),
            http,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn check_health(&self) -> Result<HealthResponse, ClientError> {
        let url = format!("{}/health", self.base_url);
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(Self::extract_error(resp).await);
        }

        resp.json::<HealthResponse>()
            .await
            .map_err(|e| ClientError::Decoding(e.to_string()))
    }

    pub async fn list_models(&self) -> Result<ModelListResponse, ClientError> {
        let url = format!("{}/v1/models", self.base_url);
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(Self::extract_error(resp).await);
        }

        resp.json::<ModelListResponse>()
            .await
            .map_err(|e| ClientError::Decoding(e.to_string()))
    }

    pub async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ClientError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let resp = self.http.post(&url).json(request).send().await?;

        if !resp.status().is_success() {
            return Err(Self::extract_error(resp).await);
        }

        resp.json::<ChatCompletionResponse>()
            .await
            .map_err(|e| ClientError::Decoding(e.to_string()))
    }

    pub async fn chat_completion_stream(
        &self,
        request: &ChatCompletionRequest,
        event_tx: Sender<StreamEvent>,
    ) -> Result<(), ClientError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let resp = self.http.post(&url).json(request).send().await?;

        if !resp.status().is_success() {
            return Err(Self::extract_error(resp).await);
        }

        let is_json = resp
            .headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .map(|ct| ct.contains("application/json"))
            .unwrap_or(false);

        if is_json {
            let comp = resp
                .json::<ChatCompletionResponse>()
                .await
                .map_err(|e| ClientError::Decoding(e.to_string()))?;

            if let Some(choice) = comp.choices.first() {
                let _ = event_tx
                    .send(StreamEvent::Delta(choice.message.content.clone()))
                    .await;
            }
            let _ = event_tx.send(StreamEvent::Done).await;
            return Ok(());
        }

        let mut byte_stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = byte_stream.next().await {
            let bytes = chunk_res?;
            let chunk_str = String::from_utf8_lossy(&bytes);
            buffer.push_str(&chunk_str);

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches("\r").to_string();
                buffer.drain(..=pos);

                Self::process_sse_line(&line, &event_tx).await;
            }
        }

        let trimmed_rem = buffer.trim_end_matches("\r").to_string();
        if !trimmed_rem.is_empty() {
            Self::process_sse_line(&trimmed_rem, &event_tx).await;
        }

        let _ = event_tx.send(StreamEvent::Done).await;
        Ok(())
    }

    async fn process_sse_line(line: &str, tx: &Sender<StreamEvent>) {
        let line = line.trim();
        if line.is_empty() || line.starts_with(':') {
            return;
        }

        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if data == "[DONE]" {
                let _ = tx.send(StreamEvent::Done).await;
            } else if data.starts_with("[ERROR:") {
                let err_msg = data
                    .strip_prefix("[ERROR:")
                    .and_then(|s| s.strip_suffix(']'))
                    .unwrap_or(data)
                    .trim()
                    .to_string();
                let _ = tx.send(StreamEvent::Error(err_msg)).await;
            } else if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        if !content.is_empty() {
                            let _ = tx.send(StreamEvent::Delta(content.clone())).await;
                        }
                    }
                }
            }
        }
    }

    async fn extract_error(response: reqwest::Response) -> ClientError {
        let status = response.status();
        match response.json::<GatewayErrorResponse>().await {
            Ok(err_body) => ClientError::Gateway {
                status,
                error_type: err_body.error.error_type,
                message: err_body.error.message,
            },
            Err(_) => ClientError::Gateway {
                status,
                error_type: "unknown".to_string(),
                message: format!("HTTP error {}", status),
            },
        }
    }
}
