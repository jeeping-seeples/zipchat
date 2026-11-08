use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use crate::models::{Conversation, Provider};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettingsData {
    #[serde(default = "default_version")]
    pub version: u32,
    pub default_provider_id: Option<String>,
    pub default_model: Option<String>,
    #[serde(default = "default_true")]
    pub stream_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_scroll_enabled: bool,
    #[serde(default)]
    pub pinned_models: Vec<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub providers: Vec<Provider>,

    // Legacy fields for v1 migration
    #[serde(skip_serializing, default)]
    pub gateway_url: Option<String>,
    #[serde(skip_serializing, default)]
    pub selected_model: Option<String>,
}

fn default_version() -> u32 {
    2
}

fn default_true() -> bool {
    true
}

impl Default for AppSettingsData {
    fn default() -> Self {
        Self {
            version: 2,
            default_provider_id: Some("openai".to_string()),
            default_model: Some("gpt-4o".to_string()),
            stream_enabled: true,
            auto_scroll_enabled: true,
            pinned_models: Vec::new(),
            system_prompt: None,
            temperature: Some(0.7),
            max_tokens: None,
            providers: Self::default_presets(),
            gateway_url: None,
            selected_model: None,
        }
    }
}

impl AppSettingsData {
    pub fn default_presets() -> Vec<Provider> {
        let mut openrouter =
            Provider::new("openrouter", "OpenRouter", "https://openrouter.ai/api/v1");
        openrouter.headers.insert(
            "HTTP-Referer".to_string(),
            "https://github.com/jeeping-seeples/zipchat".to_string(),
        );
        openrouter
            .headers
            .insert("X-Title".to_string(), "ZipChat".to_string());

        let mut vercel = Provider::new(
            "vercel",
            "Vercel AI Gateway",
            "https://ai-gateway.vercel.sh/v1",
        );
        vercel.models = vec!["openai/gpt-4o".to_string(), "openai/o3-mini".to_string()];

        vec![
            Provider::new("openai", "OpenAI", "https://api.openai.com/v1"),
            openrouter,
            vercel,
            Provider::new("groq", "Groq", "https://api.groq.com/openai/v1"),
            Provider::new("cerebras", "Cerebras", "https://api.cerebras.ai/v1"),
            Provider::new("together", "Together AI", "https://api.together.xyz/v1"),
            Provider::new("mistral", "Mistral", "https://api.mistral.ai/v1"),
            Provider::new("deepseek", "DeepSeek", "https://api.deepseek.com/v1"),
            Provider::new(
                "gemini",
                "Gemini (OpenAI Endpoint)",
                "https://generativelanguage.googleapis.com/v1beta/openai",
            ),
            Provider::new("ollama", "Ollama (Local)", "http://localhost:11434/v1"),
            Provider::new("lmstudio", "LM Studio (Local)", "http://localhost:1234/v1"),
            Provider::new(
                "gateway",
                "Universal AI Gateway",
                "http://127.0.0.1:9471/v1",
            ),
        ]
    }

    pub fn migrate_if_needed(&mut self) -> bool {
        let mut changed = false;

        if self.version < 2 || self.gateway_url.is_some() {
            let legacy_url = self
                .gateway_url
                .take()
                .unwrap_or_else(|| "http://127.0.0.1:9471".to_string());
            let legacy_model = self.selected_model.take();

            if self.providers.is_empty() {
                self.providers = Self::default_presets();
            }

            let formatted_base = format!("{}/v1", legacy_url.trim_end_matches('/'));
            if let Some(gw) = self.providers.iter_mut().find(|p| p.id == "gateway") {
                gw.base_url = formatted_base;
                gw.enabled = true;
            } else {
                self.providers.push(Provider::new(
                    "gateway",
                    "Universal AI Gateway",
                    formatted_base,
                ));
            }

            if self.default_provider_id.is_none() {
                self.default_provider_id = Some("gateway".to_string());
            }
            if self.default_model.is_none() {
                self.default_model = legacy_model;
            }

            self.version = 2;
            changed = true;
        }

        if self.providers.is_empty() {
            self.providers = Self::default_presets();
            changed = true;
        }

        changed
    }
}

#[derive(Debug, Clone)]
pub struct StorageManager {
    data_dir: PathBuf,
}

impl StorageManager {
    pub fn new(custom_dir: Option<PathBuf>) -> Self {
        let data_dir = custom_dir.unwrap_or_else(Self::detect_data_dir);
        let _ = fs::create_dir_all(&data_dir);
        Self { data_dir }
    }

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

    pub fn secrets_path(&self) -> PathBuf {
        self.data_dir.join("secrets.json")
    }

    pub fn atomic_write(
        &self,
        path: &Path,
        content: &[u8],
        permissions_0600: bool,
    ) -> io::Result<()> {
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
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);

            #[cfg(unix)]
            if permissions_0600 {
                options.mode(0o600);
            }

            let mut file = options.open(&tmp_path)?;
            file.write_all(content)?;
            file.flush()?;
            file.sync_all()?;
        }

        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn load_settings(&self) -> AppSettingsData {
        let path = self.settings_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut settings) = serde_json::from_str::<AppSettingsData>(&content) {
                    if settings.migrate_if_needed() {
                        let _ = self.save_settings(&settings);
                    }
                    return settings;
                }
            }
        }
        let settings = AppSettingsData::default();
        let _ = self.save_settings(&settings);
        settings
    }

    pub fn save_settings(&self, settings: &AppSettingsData) -> io::Result<()> {
        let json_bytes = serde_json::to_vec_pretty(settings)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.atomic_write(&self.settings_path(), &json_bytes, false)
    }

    pub fn load_conversations(&self) -> Vec<Conversation> {
        let path = self.conversations_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut convs) = serde_json::from_str::<Vec<Conversation>>(&content) {
                    let mut modified = false;
                    for conv in &mut convs {
                        if conv.provider_id.is_none() {
                            conv.provider_id = Some("gateway".to_string());
                            modified = true;
                        }
                    }
                    if modified {
                        let _ = self.save_conversations(&convs);
                    }
                    return convs;
                }
            }
        }
        Vec::new()
    }

    pub fn save_conversations(&self, conversations: &[Conversation]) -> io::Result<()> {
        let json_bytes = serde_json::to_vec_pretty(conversations)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.atomic_write(&self.conversations_path(), &json_bytes, false)
    }

    pub fn load_secrets(&self) -> HashMap<String, String> {
        let path = self.secrets_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(secrets) = serde_json::from_str::<HashMap<String, String>>(&content) {
                    return secrets;
                }
            }
        }
        HashMap::new()
    }

    pub fn save_secrets(&self, secrets: &HashMap<String, String>) -> io::Result<()> {
        let json_bytes = serde_json::to_vec_pretty(secrets)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.atomic_write(&self.secrets_path(), &json_bytes, true)
    }

    pub fn export_to_markdown(conv: &Conversation) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", conv.title));
        if let Some(provider) = &conv.provider_id {
            md.push_str(&format!("*Provider:* {}\n", provider));
        }
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

    pub fn export_to_json(conv: &Conversation) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(conv)
    }
}
