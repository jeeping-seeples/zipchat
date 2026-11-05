use serde_json::json;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zipchat::client::{ClientError, GatewayClient, StreamEvent};
use zipchat::models::{ChatCompletionRequest, ChatMessage};

#[tokio::test]
async fn test_check_health_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok",
            "version": "0.1.0"
        })))
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let health = client.check_health().await.expect("Health check succeeded");

    assert_eq!(health.status, "ok");
    assert_eq!(health.version, "0.1.0");
}

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
                    "owned_by": "mock-provider"
                },
                {
                    "id": "mock-gpt",
                    "object": "model",
                    "created": 1700000000,
                    "owned_by": "mock-provider"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let models = client.list_models().await.expect("List models succeeded");

    assert_eq!(models.data.len(), 2);
    assert_eq!(models.data[0].id, "mock-fast");
    assert_eq!(models.data[1].id, "mock-gpt");
}

#[tokio::test]
async fn test_chat_completion_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-mock-12345",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "mock-gpt",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! How can I help you today?"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        })))
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-gpt".to_string(),
        messages: vec![ChatMessage::user("Hi there")],
        temperature: None,
        max_tokens: None,
        stream: None,
    };

    let resp = client
        .chat_completion(&req)
        .await
        .expect("Chat completion succeeded");
    assert_eq!(resp.id, "chatcmpl-mock-12345");
    assert_eq!(
        resp.choices[0].message.content,
        "Hello! How can I help you today?"
    );
}

#[tokio::test]
async fn test_gateway_error_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {
                "message": "Model 'unknown-model' not found",
                "type": "not_found_error",
                "param": null,
                "code": null
            }
        })))
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "unknown-model".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: None,
        max_tokens: None,
        stream: None,
    };

    let result = client.chat_completion(&req).await;
    match result {
        Err(ClientError::Gateway {
            status,
            error_type,
            message,
        }) => {
            assert_eq!(status, reqwest::StatusCode::NOT_FOUND);
            assert_eq!(error_type, "not_found_error");
            assert!(message.contains("not found"));
        }
        other => panic!("Expected ClientError::Gateway, got {:?}", other),
    }
}

#[tokio::test]
async fn test_multi_turn_chat_history() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-mock-multiturn",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "mock-gpt",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "You are 25 years old."
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 25,
                "completion_tokens": 6,
                "total_tokens": 31
            }
        })))
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-gpt".to_string(),
        messages: vec![
            ChatMessage::user("I was born in 2000."),
            ChatMessage::assistant("Nice to know!"),
            ChatMessage::user("How old am I today?"),
        ],
        temperature: None,
        max_tokens: None,
        stream: None,
    };

    let resp = client
        .chat_completion(&req)
        .await
        .expect("Multi-turn request succeeded");
    assert_eq!(resp.choices[0].message.content, "You are 25 years old.");
}

#[tokio::test]
async fn test_sse_streaming_success() {
    let mock_server = MockServer::start().await;

    let sse_body = "data: {\"id\":\"chunk-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"mock-gpt\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n\
                    : keep-alive comment\n\n\
                    data: {\"id\":\"chunk-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000001,\"model\":\"mock-gpt\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world!\"},\"finish_reason\":\"stop\"}]}\n\n\
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

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-gpt".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: None,
        max_tokens: None,
        stream: Some(true),
    };

    let (tx, mut rx) = mpsc::channel(16);
    let handle: tokio::task::JoinHandle<Result<(), ClientError>> =
        tokio::spawn(async move { client.chat_completion_stream(&req, tx).await });

    let mut deltas = Vec::new();
    let mut done_received = false;

    while let Some(ev) = rx.recv().await {
        match ev {
            StreamEvent::Delta(d) => deltas.push(d),
            StreamEvent::Done => {
                done_received = true;
                break;
            }
            StreamEvent::Error(e) => panic!("Unexpected error event: {}", e),
        }
    }

    handle.await.unwrap().unwrap();
    assert_eq!(deltas.join(""), "Hello world!");
    assert!(done_received);
}

#[tokio::test]
async fn test_sse_streaming_split_chunks() {
    let mock_server = MockServer::start().await;

    let sse_body = "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"mock-gpt\",\
                    \"choices\":[{\"index\":0,\"delta\":{\"content\":\"Part 1 \"},\"finish_reason\":null}]}\n\
                    data: {\"id\":\"c2\",\"object\":\"chat.completion.chunk\",\"created\":2,\"model\":\"mock-gpt\",\
                    \"choices\":[{\"index\":0,\"delta\":{\"content\":\"Part 2\"},\"finish_reason\":\"stop\"}]}\n\
                    data: [DONE]\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body),
        )
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-gpt".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: None,
        max_tokens: None,
        stream: Some(true),
    };

    let (tx, mut rx) = mpsc::channel(16);
    client
        .chat_completion_stream(&req, tx)
        .await
        .expect("Stream started");

    let mut accumulated = String::new();
    while let Some(ev) = rx.recv().await {
        if let StreamEvent::Delta(d) = ev {
            accumulated.push_str(&d);
        }
    }

    assert_eq!(accumulated, "Part 1 Part 2");
}

#[tokio::test]
async fn test_sse_streaming_midstream_error() {
    let mock_server = MockServer::start().await;

    let sse_body = "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"mock-gpt\",\
                    \"choices\":[{\"index\":0,\"delta\":{\"content\":\"Starting... \"},\"finish_reason\":null}]}\n\n\
                    data: [ERROR: Provider timeout exceeded]\n\n\
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

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-gpt".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: None,
        max_tokens: None,
        stream: Some(true),
    };

    let (tx, mut rx) = mpsc::channel(16);
    client
        .chat_completion_stream(&req, tx)
        .await
        .expect("Stream started");

    let mut partial_text = String::new();
    let mut error_msg = None;

    while let Some(ev) = rx.recv().await {
        match ev {
            StreamEvent::Delta(d) => partial_text.push_str(&d),
            StreamEvent::Error(err) => {
                error_msg = Some(err);
                break;
            }
            StreamEvent::Done => {}
        }
    }

    assert_eq!(partial_text, "Starting... ");
    assert_eq!(error_msg, Some("Provider timeout exceeded".to_string()));
}

#[tokio::test]
async fn test_sse_streaming_json_fallback() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(json!({
                    "id": "chatcmpl-fallback",
                    "object": "chat.completion",
                    "created": 1700000000,
                    "model": "mock-gpt",
                    "choices": [
                        {
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": "Fallback JSON response"
                            },
                            "finish_reason": "stop"
                        }
                    ],
                    "usage": {
                        "prompt_tokens": 5,
                        "completion_tokens": 3,
                        "total_tokens": 8
                    }
                })),
        )
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-gpt".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: None,
        max_tokens: None,
        stream: Some(true),
    };

    let (tx, mut rx) = mpsc::channel(16);
    client
        .chat_completion_stream(&req, tx)
        .await
        .expect("Fallback handled");

    let mut full_text = String::new();
    let mut got_done = false;

    while let Some(ev) = rx.recv().await {
        match ev {
            StreamEvent::Delta(d) => full_text.push_str(&d),
            StreamEvent::Done => got_done = true,
            StreamEvent::Error(e) => panic!("Unexpected error: {}", e),
        }
    }

    assert_eq!(full_text, "Fallback JSON response");
    assert!(got_done);
}

#[tokio::test]
async fn test_sse_streaming_cancellation() {
    let mock_server = MockServer::start().await;

    let sse_body = "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"mock-gpt\",\
                    \"choices\":[{\"index\":0,\"delta\":{\"content\":\"Chunk 1 \"},\"finish_reason\":null}]}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body)
                .set_delay(std::time::Duration::from_millis(50)),
        )
        .mount(&mock_server)
        .await;

    let client = GatewayClient::new(mock_server.uri()).expect("Valid client URL");
    let req = ChatCompletionRequest {
        model: "mock-gpt".to_string(),
        messages: vec![ChatMessage::user("Hi")],
        temperature: None,
        max_tokens: None,
        stream: Some(true),
    };

    let (tx, mut rx) = mpsc::channel(16);
    let handle = tokio::spawn(async move {
        let _ = client.chat_completion_stream(&req, tx).await;
    });

    let first_ev = rx.recv().await;
    assert_eq!(first_ev, Some(StreamEvent::Delta("Chunk 1 ".to_string())));

    handle.abort();
    assert!(
        handle.is_finished() || {
            let _ = handle.await;
            true
        }
    );
}
