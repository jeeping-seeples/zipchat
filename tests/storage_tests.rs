use std::collections::HashMap;
use std::fs;
use zipchat::models::{Conversation, LocalMessage, MessageStatus, Provider};
use zipchat::storage::{AppSettingsData, StorageManager};

#[test]
fn test_storage_settings_atomic_persistence() {
    let temp_dir =
        std::env::temp_dir().join(format!("zipchat_test_settings_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    let storage = StorageManager::new(Some(temp_dir.clone()));

    // Load defaults when file does not exist
    let default_settings = storage.load_settings();
    assert_eq!(default_settings.version, 2);
    assert_eq!(
        default_settings.default_provider_id,
        Some("openai".to_string())
    );
    assert!(default_settings.stream_enabled);

    // Save custom settings
    let custom_settings = AppSettingsData {
        version: 2,
        default_provider_id: Some("groq".to_string()),
        default_model: Some("llama-3.3-70b-versatile".to_string()),
        stream_enabled: true,
        auto_scroll_enabled: false,
        pinned_models: vec!["gpt-4o".to_string()],
        system_prompt: Some("You are helpful.".to_string()),
        temperature: Some(0.8),
        max_tokens: Some(4096),
        providers: vec![Provider::new(
            "groq",
            "Groq",
            "https://api.groq.com/openai/v1",
        )],
        gateway_url: None,
        selected_model: None,
    };
    storage
        .save_settings(&custom_settings)
        .expect("Save settings succeeds");

    // Re-load and verify
    let loaded = storage.load_settings();
    assert_eq!(loaded, custom_settings);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_storage_secrets_atomic_persistence() {
    let temp_dir =
        std::env::temp_dir().join(format!("zipchat_test_secrets_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    let storage = StorageManager::new(Some(temp_dir.clone()));

    let mut secrets = HashMap::new();
    secrets.insert("openai".to_string(), "sk-secret-key-123".to_string());
    secrets.insert("groq".to_string(), "gsk-456".to_string());

    storage
        .save_secrets(&secrets)
        .expect("Save secrets succeeds");

    let loaded = storage.load_secrets();
    assert_eq!(
        loaded.get("openai").map(|s| s.as_str()),
        Some("sk-secret-key-123")
    );
    assert_eq!(loaded.get("groq").map(|s| s.as_str()), Some("gsk-456"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_storage_conversations_atomic_persistence() {
    let temp_dir = std::env::temp_dir().join(format!("zipchat_test_convs_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    let storage = StorageManager::new(Some(temp_dir.clone()));

    // Initially empty
    assert!(storage.load_conversations().is_empty());

    // Create and save conversations
    let mut conv = Conversation::new(
        "conv-101",
        "Project Discussion",
        Some("openai".to_string()),
        Some("gpt-4o".to_string()),
    );
    conv.messages.push(LocalMessage {
        id: "msg-1".to_string(),
        role: "user".to_string(),
        content: "What is Raspberry Pi?".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000000,
        usage: None,
    });
    conv.messages.push(LocalMessage {
        id: "msg-2".to_string(),
        role: "assistant".to_string(),
        content: "Raspberry Pi is a series of small single-board computers.".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000005,
        usage: None,
    });

    let convs = vec![conv];
    storage
        .save_conversations(&convs)
        .expect("Save conversations succeeds");

    // Re-load and verify
    let loaded = storage.load_conversations();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "conv-101");
    assert_eq!(loaded[0].messages.len(), 2);
    assert_eq!(loaded[0].messages[0].content, "What is Raspberry Pi?");
    assert_eq!(loaded[0].messages[1].role, "assistant");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_export_to_markdown() {
    let mut conv = Conversation::new(
        "conv-export",
        "Export Test",
        Some("openai".to_string()),
        Some("gpt-4o".to_string()),
    );
    conv.messages.push(LocalMessage {
        id: "msg-1".to_string(),
        role: "user".to_string(),
        content: "Write a rust hello world: ```rs\nprintln!(\"hi\");\n```".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000000,
        usage: None,
    });
    conv.messages.push(LocalMessage {
        id: "msg-2".to_string(),
        role: "assistant".to_string(),
        content: "Here is your code!".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000002,
        usage: None,
    });

    let md = StorageManager::export_to_markdown(&conv);
    assert!(md.contains("# Export Test"));
    assert!(md.contains("*Model:* gpt-4o"));
    assert!(md.contains("### **User**"));
    assert!(md.contains("Write a rust hello world:"));
    assert!(md.contains("### **Assistant**"));
    assert!(md.contains("Here is your code!"));
}

#[test]
fn test_export_to_json() {
    let mut conv = Conversation::new(
        "conv-json",
        "JSON Test",
        Some("openai".to_string()),
        Some("gpt-4o".to_string()),
    );
    conv.messages.push(LocalMessage {
        id: "m1".to_string(),
        role: "user".to_string(),
        content: "Hello json".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000000,
        usage: None,
    });

    let json_str = StorageManager::export_to_json(&conv).expect("JSON export succeeds");
    let deserialized: Conversation = serde_json::from_str(&json_str).expect("Valid JSON");
    assert_eq!(deserialized.id, "conv-json");
    assert_eq!(deserialized.messages[0].content, "Hello json");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_app_state_provider_and_model_management() {
    let temp_dir =
        std::env::temp_dir().join(format!("zipchat_test_app_state_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    let storage = StorageManager::new(Some(temp_dir.clone()));
    let mut app = zipchat::app::ZipChatApp::with_storage(storage.clone());

    // Initially starts with preset providers
    assert!(!app.providers.is_empty());
    let initial_count = app.providers.len();

    // Test adding custom provider
    app.add_custom_provider();
    assert_eq!(app.providers.len(), initial_count + 1);
    let new_provider_id = app.active_provider_id.clone();
    assert!(new_provider_id.starts_with("custom-"));

    // Test adding manual model
    app.add_manual_model(&new_provider_id, "my-custom-model-v1");
    let provider = app
        .providers
        .iter()
        .find(|p| p.id == new_provider_id)
        .unwrap();
    assert!(provider.models.contains(&"my-custom-model-v1".to_string()));

    // Test pin model
    assert!(!app
        .pinned_models
        .contains(&"my-custom-model-v1".to_string()));
    app.toggle_pin_model("my-custom-model-v1");
    assert!(app
        .pinned_models
        .contains(&"my-custom-model-v1".to_string()));

    // Toggle unpin
    app.toggle_pin_model("my-custom-model-v1");
    assert!(!app
        .pinned_models
        .contains(&"my-custom-model-v1".to_string()));

    // Test removing provider
    app.remove_provider(&new_provider_id);
    assert_eq!(app.providers.len(), initial_count);
    assert!(app.providers.iter().all(|p| p.id != new_provider_id));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_default_presets_include_vercel() {
    let presets = AppSettingsData::default_presets();
    let vercel = presets
        .iter()
        .find(|p| p.id == "vercel")
        .expect("Vercel preset exists");

    assert_eq!(vercel.name, "Vercel AI Gateway");
    assert_eq!(vercel.base_url, "https://ai-gateway.vercel.sh/v1");
    assert!(vercel.models.contains(&"openai/gpt-4o".to_string()));
    assert_eq!(
        vercel.provider_options,
        Some(serde_json::json!({
            "gateway": {
                "serviceTier": "flex"
            }
        }))
    );
}

#[test]
fn test_provider_model_blacklist_filtering() {
    let mut provider = Provider::new("openrouter", "OpenRouter", "https://openrouter.ai/api/v1");
    provider.models = vec![
        "openai/gpt-4o".to_string(),
        "openai/whisper-large".to_string(),
        "meta-llama/llama-3".to_string(),
        "text-embedding-3-small".to_string(),
    ];
    provider.model_blacklist = vec!["whisper".to_string(), "text-embedding".to_string()];

    assert!(!provider.is_model_blacklisted("openai/gpt-4o"));
    assert!(!provider.is_model_blacklisted("meta-llama/llama-3"));
    assert!(provider.is_model_blacklisted("openai/whisper-large"));
    assert!(provider.is_model_blacklisted("text-embedding-3-small"));
}
