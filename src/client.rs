use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, StatusCode};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

use crate::models::{
    ApiErrorResponse, ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse,
    ModelListResponse,
};

#[derive(Debug)]
pub enum ClientError {
    InvalidUrl(String),
    Network(reqwest::Error),
    AuthError(String),
    NotFound(String),
    RateLimit {
        retry_after: Option<String>,
        message: String,
    },
    ProviderError {
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
            ClientError::InvalidUrl(msg) => write!(f, "Invalid base URL: {}", msg),
            ClientError::Network(err) => {
                if err.is_timeout() {
                    write!(f, "Request timed out")
                } else if err.is_connect() {
                    write!(f, "Connection failed or refused")
                } else {
                    write!(f, "Network error: {}", err)
                }
            }
            ClientError::AuthError(msg) => write!(f, "Authentication failed (401/403): {}", msg),
            ClientError::NotFound(msg) => {
                write!(f, "Not found (404 - check base URL or model): {}", msg)
            }
            ClientError::RateLimit {
                message,
                retry_after,
            } => {
                if let Some(ra) = retry_after {
                    write!(f, "Rate limit exceeded (retry after {}s): {}", ra, message)
                } else {
                    write!(f, "Rate limit exceeded (429): {}", message)
                }
            }
            ClientError::ProviderError {
                status,
                error_type,
                message,
            } => write!(
                f,
                "Provider error ({status}, type: {error_type}): {message}"
            ),
            ClientError::Decoding(msg) => write!(f, "Response decoding error: {}", msg),
        }
    }
}

impl From<reqwest::Error> for ClientError {
    fn from(err: reqwest::Error) -> Self {
        ClientError::Network(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Delta(String),
    Error(String),
    Done,
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    api_key: Option<String>,
    extra_headers: HashMap<String, String>,
    http: Client,
}

impl fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiClient")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("extra_headers", &self.extra_headers)
            .finish()
    }
}

impl ApiClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        extra_headers: HashMap<String, String>,
    ) -> Result<Self, ClientError> {
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

        let key = api_key.and_then(|k| {
            let t = k.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });

        Ok(Self {
            base_url: trimmed.to_string(),
            api_key: key,
            extra_headers,
            http,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn apply_headers(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(key) = &self.api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }

        let mut headers = HeaderMap::new();
        for (k, v) in &self.extra_headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(name, val);
            }
        }
        builder.headers(headers)
    }

    pub async fn list_models(&self) -> Result<ModelListResponse, ClientError> {
        let url = format!("{}/models", self.base_url);
        let req = self.apply_headers(self.http.get(&url));
        let resp = req.send().await?;

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
        let url = format!("{}/chat/completions", self.base_url);
        let req = self.apply_headers(self.http.post(&url)).json(request);
        let resp = req.send().await?;

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
        let url = format!("{}/chat/completions", self.base_url);
        let req = self.apply_headers(self.http.post(&url)).json(request);
        let resp = req.send().await?;

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
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer.drain(..=pos);

                Self::process_sse_line(&line, &event_tx).await;
            }
        }

        let trimmed_rem = buffer.trim_end_matches('\r').to_string();
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
        let headers = response.headers().clone();
        let retry_after = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let (error_type, message) = match response.json::<ApiErrorResponse>().await {
            Ok(body) => (
                body.error.error_type.unwrap_or_else(|| "error".to_string()),
                if body.error.message.is_empty() {
                    format!("HTTP status {}", status)
                } else {
                    body.error.message
                },
            ),
            Err(_) => ("unknown".to_string(), format!("HTTP error {}", status)),
        };

        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ClientError::AuthError(message),
            StatusCode::NOT_FOUND => ClientError::NotFound(message),
            StatusCode::TOO_MANY_REQUESTS => ClientError::RateLimit {
                retry_after,
                message,
            },
            _ => ClientError::ProviderError {
                status,
                error_type,
                message,
            },
        }
    }
}
