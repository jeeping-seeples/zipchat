use std::fs;
use zipchat::models::{Conversation, LocalMessage, MessageStatus};
use zipchat::storage::{AppSettingsData, StorageManager};

#[test]
fn test_storage_settings_atomic_persistence() {
    let temp_dir =
        std::env::temp_dir().join(format!("zipchat_test_settings_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    let storage = StorageManager::new(Some(temp_dir.clone()));

    // Load defaults when file does not exist
    let default_settings = storage.load_settings();
    assert_eq!(default_settings.gateway_url, "http://127.0.0.1:9471");
    assert!(!default_settings.stream_enabled);

    // Save custom settings
    let custom_settings = AppSettingsData {
        gateway_url: "http://192.168.1.100:9471".to_string(),
        selected_model: Some("custom-rpi-model".to_string()),
        stream_enabled: true,
        auto_scroll_enabled: false,
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
        Some("mock-gpt".to_string()),
    );
    conv.messages.push(LocalMessage {
        id: "msg-1".to_string(),
        role: "user".to_string(),
        content: "What is Raspberry Pi?".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000000,
    });
    conv.messages.push(LocalMessage {
        id: "msg-2".to_string(),
        role: "assistant".to_string(),
        content: "Raspberry Pi is a series of small single-board computers.".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000005,
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
    let mut conv = Conversation::new("conv-export", "Export Test", Some("mock-gpt".to_string()));
    conv.messages.push(LocalMessage {
        id: "msg-1".to_string(),
        role: "user".to_string(),
        content: "Write a rust hello world: ```rs\nprintln!(\"hi\");\n```".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000000,
    });
    conv.messages.push(LocalMessage {
        id: "msg-2".to_string(),
        role: "assistant".to_string(),
        content: "Here is your code!".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000002,
    });

    let md = StorageManager::export_to_markdown(&conv);
    assert!(md.contains("# Export Test"));
    assert!(md.contains("*Model:* mock-gpt"));
    assert!(md.contains("### **User**"));
    assert!(md.contains("Write a rust hello world:"));
    assert!(md.contains("### **Assistant**"));
    assert!(md.contains("Here is your code!"));
}

#[test]
fn test_export_to_json() {
    let mut conv = Conversation::new("conv-json", "JSON Test", Some("mock-gpt".to_string()));
    conv.messages.push(LocalMessage {
        id: "m1".to_string(),
        role: "user".to_string(),
        content: "Hello json".to_string(),
        status: MessageStatus::Delivered,
        timestamp_epoch_secs: 1700000000,
    });

    let json_str = StorageManager::export_to_json(&conv).expect("JSON export succeeds");
    let deserialized: Conversation = serde_json::from_str(&json_str).expect("Valid JSON");
    assert_eq!(deserialized.id, "conv-json");
    assert_eq!(deserialized.messages[0].content, "Hello json");
}
