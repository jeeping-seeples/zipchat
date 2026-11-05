use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task::JoinHandle;

use crate::client::{GatewayClient, StreamEvent};
use crate::models::{
    ChatCompletionRequest, ChatMessage, Conversation, HealthResponse, LocalMessage, MessageStatus,
    ModelListResponse,
};
use crate::storage::{AppSettingsData, StorageManager};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected { version: String },
    Error(String),
}

pub enum AppMessage {
    HealthCheckResult(Result<HealthResponse, String>),
    ModelListResult(Result<ModelListResponse, String>),
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
    pub gateway_url: String,
    pub status: ConnectionStatus,
    pub available_models: Vec<String>,
    pub selected_model: Option<String>,
    pub stream_enabled: bool,
    pub auto_scroll_enabled: bool,
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

        if loaded_convs.is_empty() {
            let initial_conv =
                Conversation::new("conv-1", "New Chat", saved_settings.selected_model.clone());
            loaded_convs.push(initial_conv);
        }

        let active_id = loaded_convs.first().map(|c| c.id.clone());

        Self {
            gateway_url: saved_settings.gateway_url,
            status: ConnectionStatus::Disconnected,
            available_models: Vec::new(),
            selected_model: saved_settings.selected_model,
            stream_enabled: saved_settings.stream_enabled,
            auto_scroll_enabled: saved_settings.auto_scroll_enabled,
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
        }
    }

    pub fn persist_settings(&self) {
        let settings = AppSettingsData {
            gateway_url: self.gateway_url.clone(),
            selected_model: self.selected_model.clone(),
            stream_enabled: self.stream_enabled,
            auto_scroll_enabled: self.auto_scroll_enabled,
        };
        let _ = self.storage.save_settings(&settings);
    }

    pub fn persist_conversations(&self) {
        let _ = self.storage.save_conversations(&self.conversations);
    }

    pub fn test_connection(&mut self) {
        self.status = ConnectionStatus::Connecting;
        let url = self.gateway_url.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match GatewayClient::new(&url) {
                Ok(client) => {
                    let health_res = client.check_health().await.map_err(|e| e.to_string());
                    let _ = tx.send(AppMessage::HealthCheckResult(health_res));

                    let models_res = client.list_models().await.map_err(|e| e.to_string());
                    let _ = tx.send(AppMessage::ModelListResult(models_res));
                }
                Err(err) => {
                    let _ = tx.send(AppMessage::HealthCheckResult(Err(err.to_string())));
                }
            }
        });
    }

    pub fn create_conversation(&mut self) {
        let next_idx = self.conversations.len() + 1;
        let id = format!("conv-{}", next_idx);
        let title = format!("Chat {}", next_idx);
        let new_conv = Conversation::new(&id, title, self.selected_model.clone());
        self.active_conversation_id = Some(id);
        self.conversations.push(new_conv);
        self.persist_conversations();
    }

    pub fn delete_conversation(&mut self, id: &str) {
        if self.conversations.len() <= 1 {
            // Keep at least one conversation
            self.conversations.clear();
            let new_conv = Conversation::new("conv-1", "New Chat", self.selected_model.clone());
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
        let new_conv = Conversation::new("conv-1", "New Chat", self.selected_model.clone());
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
        };

        let mut history: Vec<ChatMessage> = Vec::new();
        if let Some(conv) = self.active_conversation_mut() {
            if conv.messages.is_empty() {
                let preview: String = text.chars().take(24).collect();
                conv.title = if preview.len() < text.len() {
                    format!("{}...", preview)
                } else {
                    preview
                };
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

        self.persist_conversations();
        self.dispatch_request(active_id, user_msg_id, assistant_msg_id, history);
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

        if let Some(conv) = self.active_conversation_mut() {
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
                });
            }
        }

        if target_content.is_some() {
            if self.auto_scroll_enabled {
                self.scroll_to_bottom = true;
            }
            self.persist_conversations();
            self.dispatch_request(
                active_id,
                user_msg_id.to_string(),
                new_assistant_msg_id,
                history,
            );
        }
    }

    fn dispatch_request(
        &mut self,
        conversation_id: String,
        user_message_id: String,
        assistant_message_id: String,
        messages: Vec<ChatMessage>,
    ) {
        let model = self
            .selected_model
            .clone()
            .unwrap_or_else(|| "mock-gpt".to_string());
        let url = self.gateway_url.clone();
        let stream = self.stream_enabled;
        let tx = self.tx.clone();

        let handle = tokio::spawn(async move {
            let client = match GatewayClient::new(&url) {
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

            let req = ChatCompletionRequest {
                model,
                messages,
                temperature: None,
                max_tokens: None,
                stream: Some(stream),
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
                AppMessage::HealthCheckResult(Ok(health)) => {
                    self.status = ConnectionStatus::Connected {
                        version: health.version,
                    };
                }
                AppMessage::HealthCheckResult(Err(err)) => {
                    self.status = ConnectionStatus::Error(err);
                }
                AppMessage::ModelListResult(Ok(model_list)) => {
                    self.available_models = model_list.data.into_iter().map(|m| m.id).collect();
                    if self.selected_model.is_none() && !self.available_models.is_empty() {
                        self.selected_model = Some(self.available_models[0].clone());
                        self.persist_settings();
                    }
                }
                AppMessage::ModelListResult(Err(_)) => {}
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

impl eframe::App for ZipChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_messages();

        // Keyboard navigation
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.is_generating() {
                self.cancel_generation();
            } else {
                self.composer_input.clear();
            }
        }

        // Left sidebar
        let mut conv_to_delete = None;
        egui::SidePanel::left("sidebar_panel")
            .resizable(true)
            .default_width(260.0)
            .width_range(200.0..=360.0)
            .show(ctx, |ui| {
                ui.heading("ZipChat");
                ui.separator();

                ui.collapsing("Gateway Settings", |ui| {
                    let mut settings_changed = false;
                    ui.label("Base URL:");
                    if ui.text_edit_singleline(&mut self.gateway_url).changed() {
                        settings_changed = true;
                    }

                    ui.add_space(4.0);
                    let button_text = if self.status == ConnectionStatus::Connecting {
                        "Connecting..."
                    } else {
                        "Test Connection"
                    };

                    if ui
                        .add_enabled(
                            self.status != ConnectionStatus::Connecting,
                            egui::Button::new(button_text),
                        )
                        .clicked()
                    {
                        self.test_connection();
                    }

                    ui.add_space(4.0);
                    match &self.status {
                        ConnectionStatus::Disconnected => {
                            ui.colored_label(egui::Color32::GRAY, "Disconnected");
                        }
                        ConnectionStatus::Connecting => {
                            ui.colored_label(egui::Color32::YELLOW, "Connecting...");
                        }
                        ConnectionStatus::Connected { version } => {
                            ui.colored_label(
                                egui::Color32::GREEN,
                                format!("Connected (v{})", version),
                            );
                        }
                        ConnectionStatus::Error(err) => {
                            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        }
                    }

                    if !self.available_models.is_empty() {
                        ui.add_space(4.0);
                        ui.label("Model:");
                        let current = self.selected_model.as_deref().unwrap_or("Select a model");
                        egui::ComboBox::from_id_salt("model_selector")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for model in &self.available_models {
                                    let is_selected =
                                        self.selected_model.as_deref() == Some(model.as_str());
                                    if ui.selectable_label(is_selected, model).clicked() {
                                        self.selected_model = Some(model.clone());
                                        settings_changed = true;
                                    }
                                }
                            });
                    }

                    ui.add_space(6.0);
                    ui.separator();
                    if ui
                        .checkbox(&mut self.stream_enabled, "Enable Streaming (SSE)")
                        .changed()
                    {
                        settings_changed = true;
                    }
                    if ui
                        .checkbox(&mut self.auto_scroll_enabled, "Auto-scroll to bottom")
                        .changed()
                    {
                        settings_changed = true;
                    }

                    if settings_changed {
                        self.persist_settings();
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

        // Main right panel: chat timeline, header actions, and composer
        egui::CentralPanel::default().show(ctx, |ui| {
            let active_conv = self.active_conversation();
            let active_title = active_conv
                .map(|c| c.title.clone())
                .unwrap_or_else(|| "No conversation".to_string());

            ui.horizontal(|ui| {
                ui.heading(&active_title);
                if let Some(model) = &self.selected_model {
                    ui.label(format!("({})", model));
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

            if let Some((msg, exp_time)) = &self.toast_message {
                if ctx.input(|i| i.time) < *exp_time {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, msg);
                }
            }

            ui.separator();

            // Message timeline
            let timeline_height = ui.available_height() - 70.0;
            let mut retry_id = None;

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
                                    ui.label(&msg.content);
                                }
                            });
                            ui.add_space(6.0);
                        }
                    }
                }
            });

            // Pause auto-scroll when user manually scrolled up
            let is_at_bottom = scroll_output.state.offset.y + scroll_output.inner_rect.height()
                >= scroll_output.content_size.y - 10.0;
            if self.scroll_to_bottom && is_at_bottom {
                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                self.scroll_to_bottom = false;
            }

            if let Some(id) = retry_id {
                self.retry_message(&id);
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

        // Confirmation modal for Clear All History
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

        if self.is_generating() || self.status == ConnectionStatus::Connecting {
            ctx.request_repaint();
        }
    }
}
