use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider descriptor configured in settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub enabled: bool,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub models_fetched_at: Option<u64>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<serde_json::Value>,
    #[serde(default)]
    pub model_pricing: HashMap<String, String>,
    #[serde(default)]
    pub model_blacklist: Vec<String>,
}

impl Provider {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into(),
            enabled: true,
            models: Vec::new(),
            models_fetched_at: None,
            headers: HashMap::new(),
            provider_options: None,
            model_pricing: HashMap::new(),
            model_blacklist: Vec::new(),
        }
    }

    pub fn is_model_blacklisted(&self, model: &str) -> bool {
        let m_lower = model.to_lowercase();
        self.model_blacklist.iter().any(|b| {
            let b_lower = b.trim().to_lowercase();
            !b_lower.is_empty() && (m_lower == b_lower || m_lower.contains(&b_lower))
        })
    }
}

/// Flexible pricing metadata parsed from GET {base}/models
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelPricing {
    #[serde(default)]
    pub prompt: Option<serde_json::Value>,
    #[serde(default)]
    pub completion: Option<serde_json::Value>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    #[serde(default)]
    pub input_cost_per_token: Option<serde_json::Value>,
    #[serde(default)]
    pub output_cost_per_token: Option<serde_json::Value>,
}

impl ModelPricing {
    pub fn display_summary(&self) -> Option<String> {
        let p_in = self
            .prompt
            .as_ref()
            .or(self.input.as_ref())
            .or(self.input_cost_per_token.as_ref());
        let p_out = self
            .completion
            .as_ref()
            .or(self.output.as_ref())
            .or(self.output_cost_per_token.as_ref());

        if p_in.is_none() && p_out.is_none() {
            return None;
        }

        let fmt_val = |v: &serde_json::Value| -> Option<String> {
            let num: Option<f64> = if let Some(n) = v.as_f64() {
                Some(n)
            } else if let Some(s) = v.as_str() {
                s.parse::<f64>().ok()
            } else {
                None
            };

            num.map(|val| {
                if val == 0.0 {
                    "Free".to_string()
                } else if val > 0.0 && val < 0.001 {
                    let per_m = val * 1_000_000.0;
                    if per_m >= 0.01 {
                        format!("${:.2}/M", per_m)
                    } else {
                        format!("${:.4}/M", per_m)
                    }
                } else {
                    format!("${:.4}", val)
                }
            })
        };

        match (p_in.and_then(fmt_val), p_out.and_then(fmt_val)) {
            (Some(i), Some(o)) => Some(format!("in: {} · out: {}", i, o)),
            (Some(i), None) => Some(format!("in: {}", i)),
            (None, Some(o)) => Some(format!("out: {}", o)),
            (None, None) => None,
        }
    }
}

/// Model descriptor in GET {base}/models response
/// Tolerant parsing: only `id` is required
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCard {
    pub id: String,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub created: Option<u64>,
    #[serde(default)]
    pub owned_by: Option<String>,
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
}

/// Response from GET {base}/models
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelListResponse {
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub data: Vec<ModelCard>,
}

/// Message element in chat completions API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            name: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
            name: None,
        }
    }
}

/// Request payload for POST {base}/chat/completions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(
        rename = "providerOptions",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub provider_options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

/// Response payload from POST {base}/chat/completions (non-streaming)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatCompletionResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub created: Option<u64>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChatChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Chunk delta for streaming chat completions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Choice entry in streaming chunk
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: ChunkDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Streaming chunk event payload from POST {base}/chat/completions (stream: true)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatCompletionChunk {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub created: Option<u64>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChunkChoice>,
}

/// Structured error details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ApiErrorDetail {
    #[serde(default)]
    pub message: String,
    #[serde(rename = "type", default)]
    pub error_type: Option<String>,
    pub param: Option<String>,
    pub code: Option<String>,
}

/// Structured error response wrapper
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiErrorResponse {
    pub error: ApiErrorDetail,
}

/// Delivery status of a message in UI
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    Delivered,
    Sending,
    Streaming,
    Cancelled,
    Failed(String),
}

/// An individual message within a conversation session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub status: MessageStatus,
    pub timestamp_epoch_secs: u64,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// A conversation session containing message history
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    pub messages: Vec<LocalMessage>,
    pub created_at_epoch_secs: u64,
}

impl Conversation {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        provider_id: Option<String>,
        model: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: id.into(),
            title: title.into(),
            provider_id,
            model,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            messages: Vec::new(),
            created_at_epoch_secs: now,
        }
    }
}
