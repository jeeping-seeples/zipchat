use reqwest::StatusCode;
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zipchat::client::{ApiClient, ClientError, StreamEvent};
use zipchat::models::{ChatCompletionRequest, ChatMessage};

#[tokio::test]
async fn test_list_models_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {
                    "id": "mock-fast",
                    "object": "model",
                    "created": 1700000000,
                    "owned_by": "organization-owner"
                },
                {
                    "id": "mock-smart",
                    "object": "model",
                    "created": 1700000001,
                    "owned_by": "organization-owner"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = ApiClient::new(format!("{}/v1", mock_server.uri()), None, HashMap::new())
        .expect("Valid client URL");
    let models = client.list_models().await.expect("List models succeeded");

    assert_eq!(models.data.len(), 2);
    assert_eq!(models.data[0].id, "mock-fast");
    assert_eq!(models.data[1].id, "mock-smart");
}

#[tokio::test]
async fn test_chat_completion_non_streaming() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "mock-fast",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello world from non-streaming!"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        })))
        .mount(&mock_server)
        .await;

    let client = ApiClient::new(format!("{}/v1", mock_server.uri()), None, HashMap::new())
        .expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-fast".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: Some(0.7),
        max_tokens: Some(100),
        stream: Some(false),
        provider_options: None,
    };

    let resp = client
        .chat_completion(&req)
        .await
        .expect("Chat completion succeeded");

    assert_eq!(resp.id.as_deref(), Some("chatcmpl-123"));
    assert_eq!(
        resp.choices[0].message.content,
        "Hello world from non-streaming!"
    );
    assert_eq!(resp.usage.map(|u| u.total_tokens), Some(30));
}

#[tokio::test]
async fn test_chat_completion_error_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": {
                "message": "Internal server upstream error",
                "type": "server_error"
            }
        })))
        .mount(&mock_server)
        .await;

    let client = ApiClient::new(format!("{}/v1", mock_server.uri()), None, HashMap::new())
        .expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-fast".to_string(),
        messages: vec![ChatMessage::user("Trigger error")],
        temperature: None,
        max_tokens: None,
        stream: Some(false),
        provider_options: None,
    };

    let res = client.chat_completion(&req).await;
    match res {
        Err(ClientError::ProviderError {
            status, message, ..
        }) => {
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert!(message.contains("Internal server upstream error"));
        }
        _ => panic!("Expected ProviderError(500), got {:?}", res),
    }
}

#[tokio::test]
async fn test_chat_completion_string_error_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "Unsupported service tier for model"
        })))
        .mount(&mock_server)
        .await;

    let client = ApiClient::new(format!("{}/v1", mock_server.uri()), None, HashMap::new())
        .expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "openai/gpt-4o".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: None,
        max_tokens: None,
        stream: Some(false),
        provider_options: None,
    };

    let res = client.chat_completion(&req).await;
    match res {
        Err(ClientError::ProviderError {
            status, message, ..
        }) => {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(message.contains("Unsupported service tier for model"));
        }
        _ => panic!("Expected ProviderError(400), got {:?}", res),
    }
}

#[tokio::test]
async fn test_chat_completion_stream_success() {
    let mock_server = MockServer::start().await;

    let sse_body = "data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"mock-fast\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello \"},\"finish_reason\":null}]}\n\n\
                    data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"mock-fast\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"world!\"},\"finish_reason\":null}]}\n\n\
                    data: [DONE]\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body),
        )
        .mount(&mock_server)
        .await;

    let client = ApiClient::new(format!("{}/v1", mock_server.uri()), None, HashMap::new())
        .expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-fast".to_string(),
        messages: vec![ChatMessage::user("Hi stream")],
        temperature: None,
        max_tokens: None,
        stream: Some(true),
        provider_options: None,
    };

    let (tx, mut rx) = mpsc::channel(16);
    client
        .chat_completion_stream(&req, tx)
        .await
        .expect("Streaming initiation succeeded");

    let mut collected = String::new();
    while let Some(evt) = rx.recv().await {
        match evt {
            StreamEvent::Delta(d) => collected.push_str(&d),
            StreamEvent::Done => break,
            StreamEvent::Error(err) => panic!("Stream error: {}", err),
        }
    }

    assert_eq!(collected, "Hello world!");
}

#[tokio::test]
async fn test_chat_completion_with_vercel_provider_options() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(wiremock::matchers::body_partial_json(json!({
            "model": "openai/gpt-4o",
            "providerOptions": {
                "gateway": {
                    "serviceTier": "flex"
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-vercel-1",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "openai/gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Analysis complete."
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;

    let client = ApiClient::new(format!("{}/v1", mock_server.uri()), None, HashMap::new())
        .expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "openai/gpt-4o".to_string(),
        messages: vec![ChatMessage::user("Analyze this log file.")],
        temperature: None,
        max_tokens: None,
        stream: Some(false),
        provider_options: Some(json!({
            "gateway": {
                "serviceTier": "flex"
            }
        })),
    };

    let resp = client
        .chat_completion(&req)
        .await
        .expect("Chat completion with providerOptions succeeded");

    assert_eq!(resp.id.as_deref(), Some("chatcmpl-vercel-1"));
    assert_eq!(resp.choices[0].message.content, "Analysis complete.");
}

#[tokio::test]
async fn test_list_models_with_pricing_parsing() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {
                    "id": "openai/gpt-4o",
                    "pricing": {
                        "prompt": "0.0000025",
                        "completion": "0.00001"
                    }
                },
                {
                    "id": "deepseek/deepseek-chat",
                    "pricing": {
                        "input": 0.00000014,
                        "output": 0.00000028
                    }
                },
                {
                    "id": "free/model",
                    "pricing": {
                        "prompt": "0",
                        "completion": "0"
                    }
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = ApiClient::new(format!("{}/v1", mock_server.uri()), None, HashMap::new())
        .expect("Valid client URL");

    let model_list = client.list_models().await.expect("List models succeeded");
    assert_eq!(model_list.data.len(), 3);

    let gpt4o = &model_list.data[0];
    assert_eq!(gpt4o.id, "openai/gpt-4o");
    let summary = gpt4o
        .pricing
        .as_ref()
        .and_then(|p| p.display_summary())
        .expect("Pricing summary present");
    assert_eq!(summary, "in: $2.50/M · out: $10.00/M");

    let deepseek = &model_list.data[1];
    let ds_summary = deepseek
        .pricing
        .as_ref()
        .and_then(|p| p.display_summary())
        .expect("Deepseek pricing present");
    assert_eq!(ds_summary, "in: $0.14/M · out: $0.28/M");

    let free = &model_list.data[2];
    let free_summary = free
        .pricing
        .as_ref()
        .and_then(|p| p.display_summary())
        .expect("Free pricing present");
    assert_eq!(free_summary, "in: Free · out: Free");
}
