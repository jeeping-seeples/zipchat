use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::models::Conversation;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettingsData {
    pub gateway_url: String,
    pub selected_model: Option<String>,
    pub stream_enabled: bool,
    pub auto_scroll_enabled: bool,
}

impl Default for AppSettingsData {
    fn default() -> Self {
        Self {
            gateway_url: "http://127.0.0.1:9471".to_string(),
            selected_model: None,
            stream_enabled: false,
            auto_scroll_enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageManager {
    data_dir: PathBuf,
}

impl StorageManager {
    /// Create a storage manager with an explicit path or detect platform defaults
    pub fn new(custom_dir: Option<PathBuf>) -> Self {
        let data_dir = custom_dir.unwrap_or_else(Self::detect_data_dir);
        let _ = fs::create_dir_all(&data_dir);
        Self { data_dir }
    }

    /// Detect platform default configuration and data directory:
    /// - Linux/Raspberry Pi ARM64: $XDG_CONFIG_HOME/zipchat or ~/.config/zipchat
    /// - macOS: ~/Library/Application Support/zipchat
    /// - Windows: %APPDATA%\zipchat
    /// - Fallback: ./.zipchat
    pub fn detect_data_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(app_data) = env::var("APPDATA") {
                if !app_data.trim().is_empty() {
                    return PathBuf::from(app_data).join("zipchat");
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = env::var("HOME") {
                if !home.trim().is_empty() {
                    return PathBuf::from(home)
                        .join("Library")
                        .join("Application Support")
                        .join("zipchat");
                }
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
                if !xdg.trim().is_empty() {
                    return PathBuf::from(xdg).join("zipchat");
                }
            }
            if let Ok(home) = env::var("HOME") {
                if !home.trim().is_empty() {
                    return PathBuf::from(home).join(".config").join("zipchat");
                }
            }
        }

        PathBuf::from(".zipchat")
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    pub fn conversations_path(&self) -> PathBuf {
        self.data_dir.join("conversations.json")
    }

    /// Safely atomic write to destination file by writing to a temporary file in the same directory first
    pub fn atomic_write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_file_name = format!(
            ".{}.tmp.{}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
            std::process::id()
        );
        let tmp_path = path.with_file_name(tmp_file_name);

        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(content)?;
            file.flush()?;
            file.sync_all()?;
        }

        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Load settings from settings.json or return default
    pub fn load_settings(&self) -> AppSettingsData {
        let path = self.settings_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str::<AppSettingsData>(&content) {
                    return settings;
                }
            }
        }
        AppSettingsData::default()
    }

    /// Save settings atomically to settings.json
    pub fn save_settings(&self, settings: &AppSettingsData) -> io::Result<()> {
        let json_bytes = serde_json::to_vec_pretty(settings)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.atomic_write(&self.settings_path(), &json_bytes)
    }

    /// Load conversations from conversations.json or return empty
    pub fn load_conversations(&self) -> Vec<Conversation> {
        let path = self.conversations_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(convs) = serde_json::from_str::<Vec<Conversation>>(&content) {
                    return convs;
                }
            }
        }
        Vec::new()
    }

    /// Save conversations atomically to conversations.json
    pub fn save_conversations(&self, conversations: &[Conversation]) -> io::Result<()> {
        let json_bytes = serde_json::to_vec_pretty(conversations)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.atomic_write(&self.conversations_path(), &json_bytes)
    }

    /// Export a conversation to clean Markdown string
    pub fn export_to_markdown(conv: &Conversation) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", conv.title));
        if let Some(model) = &conv.model {
            md.push_str(&format!("*Model:* {}\n", model));
        }
        md.push_str(&format!(
            "*Exported at Unix:* {}\n\n---\n\n",
            conv.created_at_epoch_secs
        ));

        for msg in &conv.messages {
            let role_display = if msg.role == "user" {
                "**User**"
            } else {
                "**Assistant**"
            };

            md.push_str(&format!("### {}\n\n", role_display));
            md.push_str(&msg.content);
            md.push_str("\n\n---\n\n");
        }

        md
    }

    /// Export a conversation to formatted JSON string
    pub fn export_to_json(conv: &Conversation) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(conv)
    }
}
