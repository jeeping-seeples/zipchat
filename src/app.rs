use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task::JoinHandle;

use crate::client::{ApiClient, StreamEvent};
use crate::models::{
    ChatCompletionRequest, ChatMessage, Conversation, LocalMessage, MessageStatus,
    ModelListResponse, Provider,
};
use crate::storage::{AppSettingsData, StorageManager};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Idle,
    Testing(String),
    Success {
        provider_id: String,
        model_count: usize,
    },
    Error {
        provider_id: String,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Providers,
    Models,
    General,
}

pub enum AppMessage {
    ConnectionTestResult {
        provider_id: String,
        result: Result<ModelListResponse, String>,
    },
    BackgroundModelsFetched {
        provider_id: String,
        result: Result<ModelListResponse, String>,
    },
    StreamDelta {
        conversation_id: String,
        assistant_message_id: String,
        delta: String,
    },
    StreamCompleted {
        conversation_id: String,
        user_message_id: String,
        assistant_message_id: String,
    },
    StreamFailed {
        conversation_id: String,
        user_message_id: String,
        assistant_message_id: String,
        error: String,
    },
}

pub struct ZipChatApp {
    pub providers: Vec<Provider>,
    pub api_keys: HashMap<String, String>,
    pub active_provider_id: String,
    pub status: ConnectionStatus,
    pub stream_enabled: bool,
    pub auto_scroll_enabled: bool,
    pub pinned_models: Vec<String>,
    pub global_system_prompt: Option<String>,
    pub global_temperature: Option<f32>,
    pub global_max_tokens: Option<u32>,
    pub show_parameters_drawer: bool,
    pub show_settings_modal: bool,
    pub active_settings_tab: SettingsTab,
    pub model_search_query: String,
    pub manual_model_input: String,
    pub blacklist_input: String,
    pub refreshing_providers: HashSet<String>,
    pub provider_refresh_errors: HashMap<String, String>,
    pub conversations: Vec<Conversation>,
    pub active_conversation_id: Option<String>,
    pub composer_input: String,
    pub show_clear_confirm: bool,
    pub toast_message: Option<(String, f64)>,
    storage: StorageManager,
    active_task: Option<JoinHandle<()>>,
    tx: Sender<AppMessage>,
    rx: Receiver<AppMessage>,
    scroll_to_bottom: bool,
}

impl ZipChatApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_storage(StorageManager::new(None))
    }

    pub fn with_storage(storage: StorageManager) -> Self {
        let (tx, rx) = channel();
        let saved_settings = storage.load_settings();
        let mut loaded_convs = storage.load_conversations();
        let loaded_secrets = storage.load_secrets();

        let initial_provider_id = saved_settings
            .default_provider_id
            .clone()
            .unwrap_or_else(|| "openai".to_string());

        if loaded_convs.is_empty() {
            let initial_conv = Conversation::new(
                "conv-1",
                "New Chat",
                Some(initial_provider_id.clone()),
                saved_settings.default_model.clone(),
            );
            loaded_convs.push(initial_conv);
        }

        let active_id = loaded_convs.first().map(|c| c.id.clone());

        let mut app = Self {
            providers: saved_settings.providers,
            api_keys: loaded_secrets,
            active_provider_id: initial_provider_id,
            status: ConnectionStatus::Idle,
            stream_enabled: saved_settings.stream_enabled,
            auto_scroll_enabled: saved_settings.auto_scroll_enabled,
            pinned_models: saved_settings.pinned_models,
            global_system_prompt: saved_settings.system_prompt,
            global_temperature: saved_settings.temperature,
            global_max_tokens: saved_settings.max_tokens,
            show_parameters_drawer: false,
            show_settings_modal: false,
            active_settings_tab: SettingsTab::Providers,
            model_search_query: String::new(),
            manual_model_input: String::new(),
            blacklist_input: String::new(),
            refreshing_providers: HashSet::new(),
            provider_refresh_errors: HashMap::new(),
            conversations: loaded_convs,
            active_conversation_id: active_id,
            composer_input: String::new(),
            show_clear_confirm: false,
            toast_message: None,
            storage,
            active_task: None,
            tx,
            rx,
            scroll_to_bottom: false,
        };

        app.fetch_all_enabled_models_background();
        app
    }

    pub fn fetch_all_enabled_models_background(&mut self) {
        let enabled_ids: Vec<String> = self
            .providers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.id.clone())
            .collect();
        for id in enabled_ids {
            self.trigger_background_model_fetch(&id);
        }
    }

    pub fn trigger_background_model_fetch(&mut self, provider_id: &str) {
        let provider = match self.providers.iter().find(|p| p.id == provider_id) {
            Some(p) => p.clone(),
            None => return,
        };

        self.refreshing_providers.insert(provider_id.to_string());
        self.provider_refresh_errors.remove(provider_id);

        let key = self.api_keys.get(provider_id).cloned();
        let headers = provider.headers.clone();
        let tx = self.tx.clone();
        let pid = provider_id.to_string();

        tokio::spawn(async move {
            let res = match ApiClient::new(&provider.base_url, key, headers) {
                Ok(client) => client.list_models().await.map_err(|e| e.to_string()),
                Err(err) => Err(err.to_string()),
            };

            let _ = tx.send(AppMessage::BackgroundModelsFetched {
                provider_id: pid,
                result: res,
            });
        });
    }

    pub fn current_provider(&self) -> Option<&Provider> {
        self.providers
            .iter()
            .find(|p| p.id == self.active_provider_id)
    }

    pub fn current_provider_mut(&mut self) -> Option<&mut Provider> {
        self.providers
            .iter_mut()
            .find(|p| p.id == self.active_provider_id)
    }

    pub fn persist_settings(&self) {
        let default_model = self.active_conversation().and_then(|c| c.model.clone());
        let settings = AppSettingsData {
            version: 2,
            default_provider_id: Some(self.active_provider_id.clone()),
            default_model,
            stream_enabled: self.stream_enabled,
            auto_scroll_enabled: self.auto_scroll_enabled,
            pinned_models: self.pinned_models.clone(),
            system_prompt: self.global_system_prompt.clone(),
            temperature: self.global_temperature,
            max_tokens: self.global_max_tokens,
            providers: self.providers.clone(),
            gateway_url: None,
            selected_model: None,
        };
        let _ = self.storage.save_settings(&settings);
    }

    pub fn persist_secrets(&self) {
        let _ = self.storage.save_secrets(&self.api_keys);
    }

    pub fn persist_conversations(&self) {
        let _ = self.storage.save_conversations(&self.conversations);
    }

    pub fn test_provider_connection(&mut self, provider_id: &str) {
        self.status = ConnectionStatus::Testing(provider_id.to_string());

        let provider = match self.providers.iter().find(|p| p.id == provider_id) {
            Some(p) => p.clone(),
            None => return,
        };

        let key = self.api_keys.get(provider_id).cloned();
        let headers = provider.headers.clone();
        let tx = self.tx.clone();
        let pid = provider_id.to_string();

        tokio::spawn(async move {
            let res = match ApiClient::new(&provider.base_url, key, headers) {
                Ok(client) => client.list_models().await.map_err(|e| e.to_string()),
                Err(err) => Err(err.to_string()),
            };

            let _ = tx.send(AppMessage::ConnectionTestResult {
                provider_id: pid,
                result: res,
            });
        });
    }

    pub fn add_custom_provider(&mut self) {
        let count = self.providers.len() + 1;
        let id = format!("custom-{}", count);
        let name = format!("Custom Provider {}", count);
        let provider = Provider::new(&id, name, "https://api.example.com/v1");
        self.active_provider_id = id;
        self.providers.push(provider);
        self.persist_settings();
    }

    pub fn remove_provider(&mut self, provider_id: &str) {
        if self.providers.len() <= 1 {
            return;
        }
        self.providers.retain(|p| p.id != provider_id);
        self.api_keys.remove(provider_id);
        if self.active_provider_id == provider_id {
            if let Some(first) = self.providers.first() {
                self.active_provider_id = first.id.clone();
            }
        }
        self.persist_settings();
        self.persist_secrets();
    }

    pub fn add_manual_model(&mut self, provider_id: &str, model_id: &str) {
        let trimmed = model_id.trim();
        if trimmed.is_empty() {
            return;
        }

        if let Some(p) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            if !p.models.contains(&trimmed.to_string()) {
                p.models.push(trimmed.to_string());
                self.persist_settings();
            }
        }
        self.manual_model_input.clear();
    }

    pub fn add_blacklist_model(&mut self, provider_id: &str, pattern: &str) {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            return;
        }

        if let Some(p) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            if !p.model_blacklist.contains(&trimmed.to_string()) {
                p.model_blacklist.push(trimmed.to_string());
                self.persist_settings();
            }
        }
        self.blacklist_input.clear();
    }

    pub fn remove_blacklist_model(&mut self, provider_id: &str, pattern: &str) {
        if let Some(p) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            p.model_blacklist.retain(|b| b != pattern);
            self.persist_settings();
        }
    }

    pub fn toggle_pin_model(&mut self, model: &str) {
        if let Some(pos) = self.pinned_models.iter().position(|m| m == model) {
            self.pinned_models.remove(pos);
        } else {
            self.pinned_models.push(model.to_string());
        }
        self.persist_settings();
    }

    pub fn create_conversation(&mut self) {
        let next_idx = self.conversations.len() + 1;
        let id = format!("conv-{}", next_idx);
        let title = format!("Chat {}", next_idx);

        let default_model = self
            .current_provider()
            .and_then(|p| p.models.first().cloned())
            .or_else(|| Some("gpt-4o".to_string()));

        let new_conv = Conversation::new(
            &id,
            title,
            Some(self.active_provider_id.clone()),
            default_model,
        );
        self.active_conversation_id = Some(id);
        self.conversations.push(new_conv);
        self.persist_conversations();
    }

    pub fn delete_conversation(&mut self, id: &str) {
        if self.conversations.len() <= 1 {
            self.conversations.clear();
            let new_conv = Conversation::new(
                "conv-1",
                "New Chat",
                Some(self.active_provider_id.clone()),
                Some("gpt-4o".to_string()),
            );
            self.active_conversation_id = Some(new_conv.id.clone());
            self.conversations.push(new_conv);
        } else {
            self.conversations.retain(|c| c.id != id);
            if self.active_conversation_id.as_deref() == Some(id) {
                self.active_conversation_id = self.conversations.first().map(|c| c.id.clone());
            }
        }
        self.persist_conversations();
    }

    pub fn clear_all_history(&mut self) {
        self.conversations.clear();
        let new_conv = Conversation::new(
            "conv-1",
            "New Chat",
            Some(self.active_provider_id.clone()),
            Some("gpt-4o".to_string()),
        );
        self.active_conversation_id = Some(new_conv.id.clone());
        self.conversations.push(new_conv);
        self.show_clear_confirm = false;
        self.persist_conversations();
    }

    pub fn active_conversation(&self) -> Option<&Conversation> {
        let active_id = self.active_conversation_id.as_deref()?;
        self.conversations.iter().find(|c| c.id == active_id)
    }

    pub fn active_conversation_mut(&mut self) -> Option<&mut Conversation> {
        let active_id = self.active_conversation_id.clone()?;
        self.conversations.iter_mut().find(|c| c.id == active_id)
    }

    pub fn is_generating(&self) -> bool {
        self.active_task.as_ref().is_some_and(|h| !h.is_finished())
    }

    pub fn cancel_generation(&mut self) {
        if let Some(task) = self.active_task.take() {
            task.abort();
        }

        if let Some(conv) = self.active_conversation_mut() {
            for m in &mut conv.messages {
                if m.status == MessageStatus::Sending || m.status == MessageStatus::Streaming {
                    m.status = MessageStatus::Cancelled;
                }
            }
        }
        self.persist_conversations();
    }

    pub fn regenerate_last_response(&mut self) {
        if self.is_generating() {
            return;
        }

        let active_id = match &self.active_conversation_id {
            Some(id) => id.clone(),
            None => return,
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let new_assistant_msg_id = format!("msg-a-{}-{}", now, fastrand());
        let stream = self.stream_enabled;
        let mut history: Vec<ChatMessage> = Vec::new();
        let mut last_user_id = None;
        let mut model = "gpt-4o".to_string();
        let mut provider_id = self.active_provider_id.clone();
        let mut temp = self.global_temperature;
        let mut max_tok = self.global_max_tokens;
        let mut sys_prompt = self.global_system_prompt.clone();

        if let Some(conv) = self.active_conversation_mut() {
            if let Some(m) = &conv.model {
                model = m.clone();
            }
            if let Some(pid) = &conv.provider_id {
                provider_id = pid.clone();
            }
            if conv.temperature.is_some() {
                temp = conv.temperature;
            }
            if conv.max_tokens.is_some() {
                max_tok = conv.max_tokens;
            }
            if conv.system_prompt.is_some() {
                sys_prompt = conv.system_prompt.clone();
            }

            if let Some(last) = conv.messages.last() {
                if last.role == "assistant" {
                    conv.messages.pop();
                }
            }

            if let Some(last_user) = conv.messages.iter().rev().find(|m| m.role == "user") {
                last_user_id = Some(last_user.id.clone());
            }

            for m in &conv.messages {
                if !matches!(
                    m.status,
                    MessageStatus::Failed(_) | MessageStatus::Cancelled
                ) {
                    history.push(ChatMessage {
                        role: m.role.clone(),
                        content: m.content.clone(),
                        name: None,
                    });
                }
            }

            if last_user_id.is_some() {
                conv.messages.push(LocalMessage {
                    id: new_assistant_msg_id.clone(),
                    role: "assistant".to_string(),
                    content: String::new(),
                    status: if stream {
                        MessageStatus::Streaming
                    } else {
                        MessageStatus::Sending
                    },
                    timestamp_epoch_secs: now,
                    usage: None,
                });
            }
        }

        if let Some(user_id) = last_user_id {
            let mut final_messages = Vec::new();
            if let Some(sp) = sys_prompt {
                if !sp.trim().is_empty() {
                    final_messages.push(ChatMessage::system(sp));
                }
            }
            final_messages.extend(history);

            if self.auto_scroll_enabled {
                self.scroll_to_bottom = true;
            }
            self.persist_conversations();
            self.dispatch_request(
                active_id,
                user_id,
                new_assistant_msg_id,
                provider_id,
                model,
                temp,
                max_tok,
                final_messages,
            );
        }
    }

    pub fn send_message(&mut self) {
        let text = self.composer_input.trim().to_string();
        if text.is_empty() || self.is_generating() {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let active_id = match &self.active_conversation_id {
            Some(id) => id.clone(),
            None => return,
        };

        let user_msg_id = format!("msg-u-{}-{}", now, fastrand());
        let assistant_msg_id = format!("msg-a-{}-{}", now, fastrand());

        let user_msg = LocalMessage {
            id: user_msg_id.clone(),
            role: "user".to_string(),
            content: text.clone(),
            status: MessageStatus::Sending,
            timestamp_epoch_secs: now,
            usage: None,
        };

        let assistant_msg = LocalMessage {
            id: assistant_msg_id.clone(),
            role: "assistant".to_string(),
            content: String::new(),
            status: if self.stream_enabled {
                MessageStatus::Streaming
            } else {
                MessageStatus::Sending
            },
            timestamp_epoch_secs: now,
            usage: None,
        };

        let mut history: Vec<ChatMessage> = Vec::new();
        let mut model = "gpt-4o".to_string();
        let mut provider_id = self.active_provider_id.clone();
        let mut temp = self.global_temperature;
        let mut max_tok = self.global_max_tokens;
        let mut sys_prompt = self.global_system_prompt.clone();

        if let Some(conv) = self.active_conversation_mut() {
            if conv.messages.is_empty() {
                let preview: String = text.chars().take(24).collect();
                conv.title = if preview.len() < text.len() {
                    format!("{}...", preview)
                } else {
                    preview
                };
            }

            if let Some(m) = &conv.model {
                model = m.clone();
            }
            if let Some(pid) = &conv.provider_id {
                provider_id = pid.clone();
            }
            if conv.temperature.is_some() {
                temp = conv.temperature;
            }
            if conv.max_tokens.is_some() {
                max_tok = conv.max_tokens;
            }
            if conv.system_prompt.is_some() {
                sys_prompt = conv.system_prompt.clone();
            }

            for m in &conv.messages {
                if !matches!(
                    m.status,
                    MessageStatus::Failed(_) | MessageStatus::Cancelled
                ) {
                    history.push(ChatMessage {
                        role: m.role.clone(),
                        content: m.content.clone(),
                        name: None,
                    });
                }
            }

            history.push(ChatMessage::user(&text));
            conv.messages.push(user_msg);
            conv.messages.push(assistant_msg);
        }

        self.composer_input.clear();
        if self.auto_scroll_enabled {
            self.scroll_to_bottom = true;
        }

        let mut final_messages = Vec::new();
        if let Some(sp) = sys_prompt {
            if !sp.trim().is_empty() {
                final_messages.push(ChatMessage::system(sp));
            }
        }
        final_messages.extend(history);

        self.persist_conversations();
        self.dispatch_request(
            active_id,
            user_msg_id,
            assistant_msg_id,
            provider_id,
            model,
            temp,
            max_tok,
            final_messages,
        );
    }

    pub fn retry_message(&mut self, user_msg_id: &str) {
        if self.is_generating() {
            return;
        }

        let active_id = match &self.active_conversation_id {
            Some(id) => id.clone(),
            None => return,
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let new_assistant_msg_id = format!("msg-a-{}-{}", now, fastrand());
        let stream = self.stream_enabled;
        let mut history: Vec<ChatMessage> = Vec::new();
        let mut target_content = None;
        let mut model = "gpt-4o".to_string();
        let mut provider_id = self.active_provider_id.clone();
        let mut temp = self.global_temperature;
        let mut max_tok = self.global_max_tokens;
        let mut sys_prompt = self.global_system_prompt.clone();

        if let Some(conv) = self.active_conversation_mut() {
            if let Some(m) = &conv.model {
                model = m.clone();
            }
            if let Some(pid) = &conv.provider_id {
                provider_id = pid.clone();
            }
            if conv.temperature.is_some() {
                temp = conv.temperature;
            }
            if conv.max_tokens.is_some() {
                max_tok = conv.max_tokens;
            }
            if conv.system_prompt.is_some() {
                sys_prompt = conv.system_prompt.clone();
            }

            for m in &mut conv.messages {
                if m.id == user_msg_id {
                    m.status = MessageStatus::Sending;
                    target_content = Some(m.content.clone());
                    history.push(ChatMessage::user(&m.content));
                    break;
                } else if !matches!(
                    m.status,
                    MessageStatus::Failed(_) | MessageStatus::Cancelled
                ) {
                    history.push(ChatMessage {
                        role: m.role.clone(),
                        content: m.content.clone(),
                        name: None,
                    });
                }
            }

            if target_content.is_some() {
                conv.messages.push(LocalMessage {
                    id: new_assistant_msg_id.clone(),
                    role: "assistant".to_string(),
                    content: String::new(),
                    status: if stream {
                        MessageStatus::Streaming
                    } else {
                        MessageStatus::Sending
                    },
                    timestamp_epoch_secs: now,
                    usage: None,
                });
            }
        }

        if target_content.is_some() {
            let mut final_messages = Vec::new();
            if let Some(sp) = sys_prompt {
                if !sp.trim().is_empty() {
                    final_messages.push(ChatMessage::system(sp));
                }
            }
            final_messages.extend(history);

            if self.auto_scroll_enabled {
                self.scroll_to_bottom = true;
            }
            self.persist_conversations();
            self.dispatch_request(
                active_id,
                user_msg_id.to_string(),
                new_assistant_msg_id,
                provider_id,
                model,
                temp,
                max_tok,
                final_messages,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_request(
        &mut self,
        conversation_id: String,
        user_message_id: String,
        assistant_message_id: String,
        provider_id: String,
        model: String,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        messages: Vec<ChatMessage>,
    ) {
        let provider = self.providers.iter().find(|p| p.id == provider_id).cloned();
        let base_url = provider
            .as_ref()
            .map(|p| p.base_url.clone())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let headers = provider
            .as_ref()
            .map(|p| p.headers.clone())
            .unwrap_or_default();
        let api_key = self.api_keys.get(&provider_id).cloned();
        let stream = self.stream_enabled;
        let tx = self.tx.clone();

        let handle = tokio::spawn(async move {
            let client = match ApiClient::new(&base_url, api_key, headers) {
                Ok(c) => c,
                Err(err) => {
                    let _ = tx.send(AppMessage::StreamFailed {
                        conversation_id,
                        user_message_id,
                        assistant_message_id,
                        error: err.to_string(),
                    });
                    return;
                }
            };

            let provider_options = provider
                .as_ref()
                .and_then(|p| p.provider_options.clone())
                .or_else(|| {
                    if provider_id == "vercel" || base_url.contains("ai-gateway.vercel.sh") {
                        Some(serde_json::json!({
                            "gateway": {
                                "serviceTier": "flex"
                            }
                        }))
                    } else {
                        None
                    }
                });

            let req = ChatCompletionRequest {
                model,
                messages,
                temperature,
                max_tokens,
                stream: Some(stream),
                provider_options,
            };

            if stream {
                let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
                let conv_id = conversation_id.clone();
                let user_id = user_message_id.clone();
                let asst_id = assistant_message_id.clone();
                let tx_forward = tx.clone();

                let receiver_task = tokio::spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        match event {
                            StreamEvent::Delta(delta) => {
                                let _ = tx_forward.send(AppMessage::StreamDelta {
                                    conversation_id: conv_id.clone(),
                                    assistant_message_id: asst_id.clone(),
                                    delta,
                                });
                            }
                            StreamEvent::Error(err) => {
                                let _ = tx_forward.send(AppMessage::StreamFailed {
                                    conversation_id: conv_id.clone(),
                                    user_message_id: user_id.clone(),
                                    assistant_message_id: asst_id.clone(),
                                    error: err,
                                });
                                return;
                            }
                            StreamEvent::Done => {
                                let _ = tx_forward.send(AppMessage::StreamCompleted {
                                    conversation_id: conv_id.clone(),
                                    user_message_id: user_id.clone(),
                                    assistant_message_id: asst_id.clone(),
                                });
                                return;
                            }
                        }
                    }
                });

                if let Err(err) = client.chat_completion_stream(&req, event_tx).await {
                    let _ = tx.send(AppMessage::StreamFailed {
                        conversation_id,
                        user_message_id,
                        assistant_message_id,
                        error: err.to_string(),
                    });
                }
                let _ = receiver_task.await;
            } else {
                match client.chat_completion(&req).await {
                    Ok(resp) => {
                        let content = resp
                            .choices
                            .first()
                            .map(|c| c.message.content.clone())
                            .unwrap_or_default();
                        let _ = tx.send(AppMessage::StreamDelta {
                            conversation_id: conversation_id.clone(),
                            assistant_message_id: assistant_message_id.clone(),
                            delta: content,
                        });
                        let _ = tx.send(AppMessage::StreamCompleted {
                            conversation_id,
                            user_message_id,
                            assistant_message_id,
                        });
                    }
                    Err(err) => {
                        let _ = tx.send(AppMessage::StreamFailed {
                            conversation_id,
                            user_message_id,
                            assistant_message_id,
                            error: err.to_string(),
                        });
                    }
                }
            }
        });

        self.active_task = Some(handle);
    }

    pub fn poll_messages(&mut self) {
        let mut save_needed = false;

        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMessage::ConnectionTestResult {
                    provider_id,
                    result,
                } => match result {
                    Ok(model_list) => {
                        let count = model_list.data.len();
                        if let Some(p) = self.providers.iter_mut().find(|p| p.id == provider_id) {
                            p.models = model_list.data.iter().map(|m| m.id.clone()).collect();
                            p.model_pricing.clear();
                            for m in &model_list.data {
                                if let Some(summary) =
                                    m.pricing.as_ref().and_then(|pr| pr.display_summary())
                                {
                                    p.model_pricing.insert(m.id.clone(), summary);
                                }
                            }
                            p.models_fetched_at = Some(
                                SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            );
                        }
                        self.status = ConnectionStatus::Success {
                            provider_id,
                            model_count: count,
                        };
                        save_needed = true;
                    }
                    Err(err) => {
                        self.status = ConnectionStatus::Error {
                            provider_id,
                            error: err,
                        };
                    }
                },
                AppMessage::BackgroundModelsFetched {
                    provider_id,
                    result,
                } => {
                    self.refreshing_providers.remove(&provider_id);
                    match result {
                        Ok(model_list) => {
                            if let Some(p) = self.providers.iter_mut().find(|p| p.id == provider_id)
                            {
                                p.models = model_list.data.iter().map(|m| m.id.clone()).collect();
                                p.model_pricing.clear();
                                for m in &model_list.data {
                                    if let Some(summary) =
                                        m.pricing.as_ref().and_then(|pr| pr.display_summary())
                                    {
                                        p.model_pricing.insert(m.id.clone(), summary);
                                    }
                                }
                                p.models_fetched_at = Some(
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs(),
                                );
                            }
                            save_needed = true;
                        }
                        Err(err) => {
                            self.provider_refresh_errors.insert(provider_id, err);
                        }
                    }
                }
                AppMessage::StreamDelta {
                    conversation_id,
                    assistant_message_id,
                    delta,
                } => {
                    if let Some(conv) = self
                        .conversations
                        .iter_mut()
                        .find(|c| c.id == conversation_id)
                    {
                        if let Some(msg) = conv
                            .messages
                            .iter_mut()
                            .find(|m| m.id == assistant_message_id)
                        {
                            msg.content.push_str(&delta);
                            msg.status = MessageStatus::Streaming;
                        }
                    }
                    if self.auto_scroll_enabled {
                        self.scroll_to_bottom = true;
                    }
                }
                AppMessage::StreamCompleted {
                    conversation_id,
                    user_message_id,
                    assistant_message_id,
                } => {
                    if let Some(conv) = self
                        .conversations
                        .iter_mut()
                        .find(|c| c.id == conversation_id)
                    {
                        if let Some(user_msg) =
                            conv.messages.iter_mut().find(|m| m.id == user_message_id)
                        {
                            user_msg.status = MessageStatus::Delivered;
                        }
                        if let Some(asst_msg) = conv
                            .messages
                            .iter_mut()
                            .find(|m| m.id == assistant_message_id)
                        {
                            asst_msg.status = MessageStatus::Delivered;
                        }
                    }
                    self.active_task = None;
                    save_needed = true;
                }
                AppMessage::StreamFailed {
                    conversation_id,
                    user_message_id,
                    assistant_message_id,
                    error,
                } => {
                    if let Some(conv) = self
                        .conversations
                        .iter_mut()
                        .find(|c| c.id == conversation_id)
                    {
                        if let Some(user_msg) =
                            conv.messages.iter_mut().find(|m| m.id == user_message_id)
                        {
                            user_msg.status = MessageStatus::Failed(error.clone());
                        }
                        if let Some(asst_msg) = conv
                            .messages
                            .iter_mut()
                            .find(|m| m.id == assistant_message_id)
                        {
                            asst_msg.status = MessageStatus::Failed(error);
                        }
                    }
                    self.active_task = None;
                    save_needed = true;
                }
            }
        }

        if save_needed {
            self.persist_settings();
            self.persist_conversations();
        }
    }
}

fn fastrand() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
}

fn render_message_content(ui: &mut egui::Ui, content: &str) {
    if !content.contains("```") {
        ui.label(content);
        return;
    }

    let mut remaining = content;
    while let Some(start_idx) = remaining.find("```") {
        let before = &remaining[..start_idx];
        if !before.is_empty() {
            ui.label(before);
        }

        let after_start = &remaining[start_idx + 3..];
        if let Some(end_idx) = after_start.find("```") {
            let code_block = &after_start[..end_idx];
            let mut lines = code_block.lines();
            let first_line = lines.next().unwrap_or("");
            let (lang, code) = if !first_line.is_empty()
                && !first_line.contains(' ')
                && code_block.starts_with(first_line)
            {
                let rest_code = &code_block[first_line.len()..]
                    .trim_start_matches('\n')
                    .trim_start_matches('\r');
                (first_line.trim(), rest_code.trim())
            } else {
                ("", code_block.trim())
            };

            ui.add_space(4.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(30, 30, 35))
                .inner_margin(8.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if !lang.is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(180, 180, 200),
                                egui::RichText::new(lang).monospace().size(11.0),
                            );
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(180, 180, 200),
                                egui::RichText::new("code").monospace().size(11.0),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("📋 Copy").clicked() {
                                ui.output_mut(|o| o.copied_text = code.to_string());
                            }
                        });
                    });
                    ui.add_space(2.0);
                    ui.add(egui::Label::new(egui::RichText::new(code).monospace()).wrap());
                });
            ui.add_space(4.0);

            remaining = &after_start[end_idx + 3..];
        } else {
            ui.label(remaining);
            return;
        }
    }

    if !remaining.is_empty() {
        ui.label(remaining);
    }
}

impl eframe::App for ZipChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_messages();

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.is_generating() {
                self.cancel_generation();
            } else {
                self.composer_input.clear();
            }
        }

        let mut conv_to_delete = None;
        let mut remove_provider_id = None;

        egui::SidePanel::left("sidebar_panel")
            .resizable(true)
            .default_width(280.0)
            .width_range(220.0..=380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("ZipChat");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("⚙ Settings").clicked() {
                            self.show_settings_modal = true;
                        }
                    });
                });
                ui.separator();

                ui.collapsing("Quick Settings", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Provider:");
                        let active_name = self
                            .current_provider()
                            .map(|p| p.name.clone())
                            .unwrap_or_else(|| "Select Provider".to_string());

                        egui::ComboBox::from_id_salt("quick_provider_select")
                            .selected_text(active_name)
                            .show_ui(ui, |ui| {
                                for p in &self.providers {
                                    let is_active = p.id == self.active_provider_id;
                                    if ui.selectable_label(is_active, &p.name).clicked() {
                                        self.active_provider_id = p.id.clone();
                                    }
                                }
                            });
                    });

                    let mut settings_changed = false;
                    let mut secrets_changed = false;
                    let current_pid = self.active_provider_id.clone();

                    if let Some(_provider) = self.providers.iter_mut().find(|p| p.id == current_pid)
                    {
                        let key_entry = self.api_keys.entry(current_pid.clone()).or_default();
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            if ui
                                .add(egui::TextEdit::singleline(key_entry).password(true))
                                .changed()
                            {
                                secrets_changed = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            let is_refreshing = self.refreshing_providers.contains(&current_pid);
                            if ui
                                .add_enabled(!is_refreshing, egui::Button::new("Refresh Models"))
                                .clicked()
                            {
                                self.trigger_background_model_fetch(&current_pid);
                            }

                            if ui.button("Manage...").clicked() {
                                self.show_settings_modal = true;
                            }
                        });
                    }

                    ui.add_space(4.0);
                    if ui
                        .checkbox(&mut self.stream_enabled, "Streaming (SSE)")
                        .changed()
                    {
                        settings_changed = true;
                    }
                    if ui
                        .checkbox(&mut self.auto_scroll_enabled, "Auto-scroll")
                        .changed()
                    {
                        settings_changed = true;
                    }

                    if settings_changed {
                        self.persist_settings();
                    }
                    if secrets_changed {
                        self.persist_secrets();
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.heading("Conversations");
                    if ui.button("+ New").clicked() {
                        self.create_conversation();
                    }
                });
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("conv_list")
                    .show(ui, |ui| {
                        let active_id = self.active_conversation_id.clone();
                        for conv in &self.conversations {
                            ui.horizontal(|ui| {
                                let is_active = active_id.as_deref() == Some(&conv.id);
                                if ui.selectable_label(is_active, &conv.title).clicked() {
                                    self.active_conversation_id = Some(conv.id.clone());
                                }

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("🗑").clicked() {
                                            conv_to_delete = Some(conv.id.clone());
                                        }
                                    },
                                );
                            });
                        }
                    });

                ui.add_space(8.0);
                ui.separator();
                if ui.button("Clear All History").clicked() {
                    self.show_clear_confirm = true;
                }
            });

        if let Some(id) = conv_to_delete {
            self.delete_conversation(&id);
        }

        // Main right panel: chat timeline, model selection, parameters, and composer
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut conv_modified = false;
            let active_conv = self.active_conversation();
            let active_title = active_conv
                .map(|c| c.title.clone())
                .unwrap_or_else(|| "No conversation".to_string());

            ui.horizontal(|ui| {
                ui.heading(&active_title);

                let active_pid = self.active_provider_id.clone();
                let current_model_str = self
                    .active_conversation()
                    .and_then(|c| c.model.clone())
                    .unwrap_or_else(|| "Select Model".to_string());
                let mut new_selected_model = None;
                let mut toggle_pin = None;

                ui.label("| Model:");
                egui::ComboBox::from_id_salt("chat_model_selector")
                    .selected_text(&current_model_str)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("🔍");
                            ui.text_edit_singleline(&mut self.model_search_query);
                        });
                        ui.separator();

                        let query = self.model_search_query.to_lowercase();
                        let active_provider_opt =
                            self.providers.iter().find(|p| p.id == active_pid);

                        if !self.pinned_models.is_empty() {
                            let mut pinned_header_drawn = false;
                            for pinned in &self.pinned_models {
                                if let Some(p) = active_provider_opt {
                                    if p.is_model_blacklisted(pinned) {
                                        continue;
                                    }
                                }
                                if query.is_empty() || pinned.to_lowercase().contains(&query) {
                                    if !pinned_header_drawn {
                                        ui.label(
                                            egui::RichText::new("★ Pinned").strong().size(11.0),
                                        );
                                        pinned_header_drawn = true;
                                    }
                                    ui.horizontal(|ui| {
                                        if ui.small_button("★").clicked() {
                                            toggle_pin = Some(pinned.clone());
                                        }
                                        let is_selected = current_model_str == *pinned;
                                        if ui.selectable_label(is_selected, pinned).clicked() {
                                            new_selected_model = Some(pinned.clone());
                                        }
                                        if let Some(p) = active_provider_opt {
                                            if let Some(price) = p.model_pricing.get(pinned) {
                                                ui.label(
                                                    egui::RichText::new(price).weak().size(10.0),
                                                );
                                            }
                                        }
                                    });
                                }
                            }
                            if pinned_header_drawn {
                                ui.separator();
                            }
                        }

                        for provider in &self.providers {
                            if !provider.enabled && provider.id != active_pid {
                                continue;
                            }

                            let matching_models: Vec<&String> = provider
                                .models
                                .iter()
                                .filter(|m| !provider.is_model_blacklisted(m))
                                .filter(|m| query.is_empty() || m.to_lowercase().contains(&query))
                                .collect();

                            if !matching_models.is_empty() {
                                ui.label(egui::RichText::new(&provider.name).weak().size(11.0));
                                for m in matching_models {
                                    ui.horizontal(|ui| {
                                        let is_pinned = self.pinned_models.contains(m);
                                        let pin_icon = if is_pinned { "★" } else { "☆" };
                                        if ui.small_button(pin_icon).clicked() {
                                            toggle_pin = Some(m.clone());
                                        }
                                        let is_selected = current_model_str == **m;
                                        if ui.selectable_label(is_selected, m).clicked() {
                                            new_selected_model = Some(m.clone());
                                        }
                                        if let Some(price) = provider.model_pricing.get(m) {
                                            ui.label(egui::RichText::new(price).weak().size(10.0));
                                        }
                                    });
                                }
                            }
                        }

                        let exists = self
                            .providers
                            .iter()
                            .any(|p| p.models.contains(&current_model_str));
                        if !exists && current_model_str != "Select Model" {
                            ui.separator();
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                format!("Current: {} (unlisted)", current_model_str),
                            );
                        }
                    });

                if let Some(model_name) = new_selected_model {
                    if let Some(conv) = self.active_conversation_mut() {
                        conv.model = Some(model_name);
                        conv.provider_id = Some(active_pid);
                        conv_modified = true;
                    }
                }

                if let Some(m) = toggle_pin {
                    self.toggle_pin_model(&m);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.is_generating()
                        && ui
                            .button(
                                egui::RichText::new("Stop Generating").color(egui::Color32::RED),
                            )
                            .clicked()
                    {
                        self.cancel_generation();
                    }

                    if !self.is_generating() && ui.button("↺ Regenerate").clicked() {
                        self.regenerate_last_response();
                    }

                    if ui
                        .selectable_label(self.show_parameters_drawer, "⚙ Parameters")
                        .clicked()
                    {
                        self.show_parameters_drawer = !self.show_parameters_drawer;
                    }

                    let mut export_toast = None;
                    if let Some(conv) = self.active_conversation() {
                        if ui.button("Export .md").clicked() {
                            let md = StorageManager::export_to_markdown(conv);
                            let export_name = format!("export_{}.md", conv.id);
                            let path = self.storage.data_dir().join(&export_name);
                            if std::fs::write(&path, md).is_ok() {
                                export_toast = Some((
                                    format!("Exported to {}", path.display()),
                                    ctx.input(|i| i.time) + 4.0,
                                ));
                            }
                        }

                        if ui.button("Export .json").clicked() {
                            if let Ok(json_str) = StorageManager::export_to_json(conv) {
                                let export_name = format!("export_{}.json", conv.id);
                                let path = self.storage.data_dir().join(&export_name);
                                if std::fs::write(&path, json_str).is_ok() {
                                    export_toast = Some((
                                        format!("Exported to {}", path.display()),
                                        ctx.input(|i| i.time) + 4.0,
                                    ));
                                }
                            }
                        }
                    }
                    if let Some(toast) = export_toast {
                        self.toast_message = Some(toast);
                    }
                });
            });

            if conv_modified {
                self.persist_conversations();
            }

            if let Some((msg, exp_time)) = &self.toast_message {
                if ctx.input(|i| i.time) < *exp_time {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, msg);
                }
            }

            if self.show_parameters_drawer {
                let mut params_changed = false;
                ui.group(|ui| {
                    ui.heading("Parameters (Active Conversation)");
                    if let Some(conv) = self.active_conversation_mut() {
                        ui.horizontal(|ui| {
                            ui.label("System Prompt:");
                            let mut sys = conv.system_prompt.clone().unwrap_or_default();
                            if ui.text_edit_singleline(&mut sys).changed() {
                                conv.system_prompt = if sys.trim().is_empty() {
                                    None
                                } else {
                                    Some(sys)
                                };
                                params_changed = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Temperature:");
                            let mut temp = conv.temperature.unwrap_or(0.7);
                            if ui
                                .add(egui::Slider::new(&mut temp, 0.0..=2.0).step_by(0.1))
                                .changed()
                            {
                                conv.temperature = Some(temp);
                                params_changed = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Max Tokens:");
                            let mut max_tok = conv.max_tokens.unwrap_or(0);
                            if ui
                                .add(egui::DragValue::new(&mut max_tok).range(0..=128000))
                                .changed()
                            {
                                conv.max_tokens = if max_tok == 0 { None } else { Some(max_tok) };
                                params_changed = true;
                            }
                        });
                    }
                });
                if params_changed {
                    self.persist_conversations();
                }
            }

            ui.separator();

            // Timeline
            let timeline_height = ui.available_height() - 70.0;
            let mut retry_id = None;
            let mut trigger_regenerate = false;

            let scroll_area = egui::ScrollArea::vertical()
                .id_salt("timeline_scroll")
                .auto_shrink([false; 2])
                .max_height(timeline_height);

            let scroll_output = scroll_area.show(ui, |ui| {
                if let Some(conv) = self.active_conversation() {
                    if conv.messages.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("No messages yet. Send a message to start!")
                                    .italics(),
                            );
                        });
                    } else {
                        for msg in &conv.messages {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    let is_user = msg.role == "user";
                                    let role_color = if is_user {
                                        egui::Color32::from_rgb(100, 160, 255)
                                    } else {
                                        egui::Color32::from_rgb(120, 220, 140)
                                    };
                                    ui.colored_label(
                                        role_color,
                                        egui::RichText::new(&msg.role).strong(),
                                    );

                                    if let Some(usage) = &msg.usage {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} tokens",
                                                usage.total_tokens
                                            ))
                                            .weak()
                                            .size(11.0),
                                        );
                                    }

                                    match &msg.status {
                                        MessageStatus::Delivered => {
                                            ui.label(
                                                egui::RichText::new("Delivered").weak().size(11.0),
                                            );
                                        }
                                        MessageStatus::Sending => {
                                            ui.colored_label(
                                                egui::Color32::YELLOW,
                                                egui::RichText::new("Sending...").size(11.0),
                                            );
                                        }
                                        MessageStatus::Streaming => {
                                            ui.colored_label(
                                                egui::Color32::LIGHT_BLUE,
                                                egui::RichText::new("Streaming...").size(11.0),
                                            );
                                        }
                                        MessageStatus::Cancelled => {
                                            ui.colored_label(
                                                egui::Color32::GRAY,
                                                egui::RichText::new("Cancelled").size(11.0),
                                            );
                                        }
                                        MessageStatus::Failed(err) => {
                                            ui.colored_label(
                                                egui::Color32::RED,
                                                egui::RichText::new(format!("Failed: {}", err))
                                                    .size(11.0),
                                            );
                                            if is_user && ui.small_button("Retry").clicked() {
                                                retry_id = Some(msg.id.clone());
                                            }
                                        }
                                    }
                                });

                                ui.add_space(4.0);
                                if msg.content.is_empty() && msg.status == MessageStatus::Streaming
                                {
                                    ui.label(egui::RichText::new("Thinking...").italics());
                                } else {
                                    render_message_content(ui, &msg.content);
                                }

                                if msg.role != "user"
                                    && msg.status == MessageStatus::Delivered
                                    && !self.is_generating()
                                {
                                    ui.horizontal(|ui| {
                                        if ui.small_button("↺ Regenerate").clicked() {
                                            trigger_regenerate = true;
                                        }
                                    });
                                }
                            });
                            ui.add_space(6.0);
                        }
                    }
                }
            });

            let is_at_bottom = scroll_output.state.offset.y + scroll_output.inner_rect.height()
                >= scroll_output.content_size.y - 10.0;
            if self.scroll_to_bottom && is_at_bottom {
                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                self.scroll_to_bottom = false;
            }

            if let Some(id) = retry_id {
                self.retry_message(&id);
            }
            if trigger_regenerate {
                self.regenerate_last_response();
            }

            ui.separator();

            // Composer
            ui.horizontal(|ui| {
                let response = ui.add_sized(
                    [ui.available_width() - 80.0, 48.0],
                    egui::TextEdit::multiline(&mut self.composer_input)
                        .hint_text("Type your message... (Shift+Enter for newline, Enter to send)"),
                );

                let is_enter_pressed =
                    ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                let can_send = !self.is_generating() && !self.composer_input.trim().is_empty();
                let send_clicked = ui
                    .add_enabled(can_send, egui::Button::new("Send"))
                    .clicked();

                if (send_clicked || (response.lost_focus() && is_enter_pressed)) && can_send {
                    self.send_message();
                }
            });
        });

        // Dedicated Settings Window Modal
        if self.show_settings_modal {
            egui::Window::new("⚙ Settings")
                .collapsible(false)
                .resizable(true)
                .default_size([640.0, 480.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.active_settings_tab, SettingsTab::Providers, "Providers");
                        ui.selectable_value(&mut self.active_settings_tab, SettingsTab::Models, "Models");
                        ui.selectable_value(&mut self.active_settings_tab, SettingsTab::General, "General & Defaults");
                    });
                    ui.separator();

                    let mut settings_changed = false;
                    let mut secrets_changed = false;

                    match self.active_settings_tab {
                        SettingsTab::Providers => {
                            ui.heading("Manage Providers");
                            ui.label("Configure OpenAI-compatible providers, local servers, and custom endpoints.");
                            ui.add_space(8.0);

                            if ui.button("+ Add Custom Provider").clicked() {
                                self.add_custom_provider();
                            }
                            ui.add_space(8.0);

                            egui::ScrollArea::vertical().id_salt("settings_providers_scroll").show(ui, |ui| {
                                let mut to_remove = None;
                                let mut to_test = None;
                                for p in &mut self.providers {
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.checkbox(&mut p.enabled, "");
                                            ui.text_edit_singleline(&mut p.name);
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.small_button("Delete").clicked() {
                                                    to_remove = Some(p.id.clone());
                                                }
                                            });
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Base URL:");
                                            if ui.text_edit_singleline(&mut p.base_url).changed() {
                                                settings_changed = true;
                                            }
                                        });

                                        let key_entry = self.api_keys.entry(p.id.clone()).or_default();
                                        ui.horizontal(|ui| {
                                            ui.label("API Key:");
                                            if ui.add(egui::TextEdit::singleline(key_entry).password(true)).changed() {
                                                secrets_changed = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            let is_testing = matches!(&self.status, ConnectionStatus::Testing(id) if id == &p.id);
                                            let btn_txt = if is_testing { "Testing..." } else { "Test Connection" };
                                            if ui.add_enabled(!is_testing, egui::Button::new(btn_txt)).clicked() {
                                                to_test = Some(p.id.clone());
                                            }

                                            if let Some(ts) = p.models_fetched_at {
                                                let _ = ts;
                                                ui.label(format!("{} models cached", p.models.len()));
                                            }
                                        });
                                    });
                                    ui.add_space(4.0);
                                }
                                if let Some(id) = to_remove {
                                    remove_provider_id = Some(id);
                                }
                                if let Some(id) = to_test {
                                    self.test_provider_connection(&id);
                                }
                            });
                        }
                        SettingsTab::Models => {
                            ui.heading("Manage Models");
                            ui.label("Pull, refresh, or manually add models per provider.");
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                ui.label("Provider:");
                                let active_name = self.current_provider().map(|p| p.name.clone()).unwrap_or_default();
                                egui::ComboBox::from_id_salt("settings_provider_picker")
                                    .selected_text(active_name)
                                    .show_ui(ui, |ui| {
                                        for p in &self.providers {
                                            if ui.selectable_label(p.id == self.active_provider_id, &p.name).clicked() {
                                                self.active_provider_id = p.id.clone();
                                            }
                                        }
                                    });

                                let current_pid = self.active_provider_id.clone();
                                let is_refreshing = self.refreshing_providers.contains(&current_pid);
                                if ui.add_enabled(!is_refreshing, egui::Button::new("Refresh Models")).clicked() {
                                    self.trigger_background_model_fetch(&current_pid);
                                }
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label("Add Model ID:");
                                ui.text_edit_singleline(&mut self.manual_model_input);
                                if ui.button("+ Add").clicked() && !self.manual_model_input.trim().is_empty() {
                                    let model_id = self.manual_model_input.clone();
                                    let pid = self.active_provider_id.clone();
                                    self.add_manual_model(&pid, &model_id);
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Hide / Blacklist Pattern:");
                                ui.text_edit_singleline(&mut self.blacklist_input);
                                if ui.button("+ Blacklist").clicked() && !self.blacklist_input.trim().is_empty() {
                                    let pattern = self.blacklist_input.clone();
                                    let pid = self.active_provider_id.clone();
                                    self.add_blacklist_model(&pid, &pattern);
                                }
                            });

                            let current_pid = self.active_provider_id.clone();
                            let mut remove_blacklist_target = None;
                            if let Some(p) = self.providers.iter().find(|p| p.id == current_pid) {
                                if !p.model_blacklist.is_empty() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(egui::RichText::new("Blacklisted patterns:").weak().size(11.0));
                                        for b in &p.model_blacklist {
                                            if ui.small_button(format!("✕ {}", b)).on_hover_text("Click to remove from blacklist").clicked() {
                                                remove_blacklist_target = Some(b.clone());
                                            }
                                        }
                                    });
                                }
                            }
                            if let Some(b) = remove_blacklist_target {
                                self.remove_blacklist_model(&current_pid, &b);
                            }

                            ui.separator();
                            ui.label("Available & Cached Models:");
                            let mut toggle_pin_target = None;
                            let mut toggle_blacklist_target = None;
                            if let Some(p) = self.providers.iter().find(|p| p.id == current_pid) {
                                egui::ScrollArea::vertical().id_salt("settings_models_scroll").show(ui, |ui| {
                                    for m in &p.models {
                                        let is_blacklisted = p.is_model_blacklisted(m);
                                        ui.horizontal(|ui| {
                                            let is_pinned = self.pinned_models.contains(m);
                                            let pin_icon = if is_pinned { "★ Pinned" } else { "☆ Pin" };
                                            if ui.small_button(pin_icon).clicked() {
                                                toggle_pin_target = Some(m.clone());
                                            }

                                            let hide_label = if is_blacklisted { "⊘ Hidden" } else { "Hide" };
                                            if ui.small_button(hide_label).clicked() {
                                                toggle_blacklist_target = Some((m.clone(), !is_blacklisted));
                                            }

                                            let text_widget = if is_blacklisted {
                                                egui::RichText::new(m).strikethrough().weak()
                                            } else {
                                                egui::RichText::new(m)
                                            };
                                            ui.label(text_widget);

                                            if let Some(price) = p.model_pricing.get(m) {
                                                ui.label(egui::RichText::new(price).weak().size(11.0));
                                            }
                                        });
                                    }
                                });
                            }
                            if let Some(m) = toggle_pin_target {
                                self.toggle_pin_model(&m);
                            }
                            if let Some((pattern, should_add)) = toggle_blacklist_target {
                                if should_add {
                                    self.add_blacklist_model(&current_pid, &pattern);
                                } else {
                                    self.remove_blacklist_model(&current_pid, &pattern);
                                }
                            }
                        }
                        SettingsTab::General => {
                            ui.heading("General Settings");
                            ui.add_space(8.0);

                            if ui.checkbox(&mut self.stream_enabled, "Enable Streaming (SSE)").changed() {
                                settings_changed = true;
                            }
                            if ui.checkbox(&mut self.auto_scroll_enabled, "Auto-scroll to bottom").changed() {
                                settings_changed = true;
                            }

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label("Global Temperature:");
                                let mut temp = self.global_temperature.unwrap_or(0.7);
                                if ui.add(egui::Slider::new(&mut temp, 0.0..=2.0).step_by(0.1)).changed() {
                                    self.global_temperature = Some(temp);
                                    settings_changed = true;
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Global System Prompt:");
                                let mut sys = self.global_system_prompt.clone().unwrap_or_default();
                                if ui.text_edit_singleline(&mut sys).changed() {
                                    self.global_system_prompt = if sys.trim().is_empty() { None } else { Some(sys) };
                                    settings_changed = true;
                                }
                            });
                        }
                    }

                    if settings_changed {
                        self.persist_settings();
                    }
                    if secrets_changed {
                        self.persist_secrets();
                    }

                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            self.show_settings_modal = false;
                        }
                    });
                });
        }

        if let Some(id) = remove_provider_id {
            self.remove_provider(&id);
        }

        if self.show_clear_confirm {
            egui::Window::new("Confirm Clear History")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Are you sure you want to delete all conversations? This action cannot be undone.");
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new("Yes, Delete All").color(egui::Color32::RED)).clicked() {
                            self.clear_all_history();
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_clear_confirm = false;
                        }
                    });
                });
        }

        if self.is_generating()
            || matches!(self.status, ConnectionStatus::Testing(_))
            || !self.refreshing_providers.is_empty()
        {
            ctx.request_repaint();
        }
    }
}
