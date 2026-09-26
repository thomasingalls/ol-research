//! Real loopback sockets, a deterministic fake model, and no model downloads.

use axum::{Json, Router, http::StatusCode, routing::post};
use ol_rht_chat::{Config, DEV_KEY};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{address}"), task)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}
fn request() -> Value {
    json!({ "messages": [{ "id": "user-1", "role": "user", "content": [{ "type": "text", "text": "Hello" }] }] })
}
fn chunk(delta: &Value, reason: &Value) -> String {
    format!(
        "data: {}\r\n\r\n",
        json!({ "choices": [{ "delta": delta, "finish_reason": reason }] })
    )
}
async fn events(url: &str, request: Value) -> Vec<Value> {
    let response = client()
        .post(format!("{url}/v1/chat"))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/x-ndjson");
    response
        .text()
        .await
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn preserves_interleaved_parts_and_selected_history() {
    let (sent, received) = tokio::sync::oneshot::channel();
    let sent = std::sync::Arc::new(std::sync::Mutex::new(Some(sent)));
    let wire = chunk(&json!({"reasoning_content":"Thinking λ"}), &Value::Null)
        + &chunk(&json!({"reasoning_content":"…"}), &Value::Null)
        + &chunk(&json!({"content":"Answer 😀"}), &Value::Null)
        + &chunk(&json!({"reasoning_content":"Check"}), &Value::Null)
        + &chunk(&json!({"content":"Done"}), &json!("length"))
        + "data: [DONE]\n\n";
    let (model, model_task) = serve(Router::new().route(
        "/v1/chat/completions",
        post(
            move |headers: axum::http::HeaderMap, Json(body): Json<Value>| {
                let wire = wire.clone();
                let sent = sent.clone();
                async move {
                    assert_eq!(headers["authorization"], format!("Bearer {DEV_KEY}"));
                    sent.lock().unwrap().take().unwrap().send(body).unwrap();
                    ([("content-type", "text/event-stream")], wire)
                }
            },
        ),
    ))
    .await;
    let (api, api_task) = serve(
        ol_rht_chat::router(Config::local(&format!("{model}/v1"), "test-model").unwrap()).unwrap(),
    )
    .await;
    let history = json!({"messages":[
        {"id":"u1","role":"user","content":[{"type":"text","text":"First question"}]},
        {"id":"a1","role":"assistant","content":[{"type":"reasoning","text":"private to history"},{"type":"text","text":"First answer"}]},
        {"id":"u2","role":"user","content":[{"type":"text","text":"Follow-up"}]}
    ]});
    let output = events(&api, history).await;
    let forwarded = received.await.unwrap();
    assert_eq!(forwarded["model"], "test-model");
    assert_eq!(forwarded["stream"], true);
    assert_eq!(
        forwarded["messages"][2],
        json!({"role":"assistant","content":"First answer"})
    );
    assert_eq!(forwarded["messages"][3]["content"], "Follow-up");
    assert_eq!(output[0]["type"], "start");
    assert_eq!(output[1]["part_id"], output[2]["part_id"]);
    assert_ne!(output[2]["part_id"], output[3]["part_id"]);
    assert_ne!(output[3]["part_id"], output[4]["part_id"]);
    assert_eq!(output[1]["text"], "Thinking λ");
    assert_eq!(output[3]["text"], "Answer 😀");
    assert_eq!(
        output.last().unwrap(),
        &json!({"type":"finish","reason":"length"})
    );
    model_task.abort();
    api_task.abort();
}

#[tokio::test]
async fn upstream_errors_and_early_eof_are_explicit() {
    let cases = [
        (
            StatusCode::UNAUTHORIZED,
            "application/json",
            "{}".to_owned(),
        ),
        (StatusCode::OK, "text/html", "wrong endpoint".to_owned()),
        (
            StatusCode::OK,
            "text/event-stream",
            chunk(&json!({"content":"partial"}), &Value::Null),
        ),
        (
            StatusCode::OK,
            "text/event-stream",
            "data: broken\n\n".to_owned(),
        ),
        (
            StatusCode::OK,
            "text/event-stream",
            "data: [DONE]\n\n".to_owned(),
        ),
        (
            StatusCode::OK,
            "text/event-stream",
            chunk(&json!({"tool_calls":[{}]}), &json!("tool_calls")),
        ),
    ];
    for (status, content_type, wire) in cases {
        let (model, model_task) = serve(Router::new().route(
            "/v1/chat/completions",
            post(move || {
                let wire = wire.clone();
                async move { (status, [("content-type", content_type)], wire) }
            }),
        ))
        .await;
        let (api, api_task) = serve(
            ol_rht_chat::router(Config::local(&format!("{model}/v1"), "test").unwrap()).unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        assert_eq!(output.last().unwrap()["type"], "error");
        assert!(!output.iter().any(|event| event["type"] == "finish"));
        model_task.abort();
        api_task.abort();
    }
}

#[tokio::test]
async fn bad_requests_are_refused_before_model_access() {
    let (api, task) = serve(
        ol_rht_chat::router(Config::local("http://127.0.0.1:1/v1", "test").unwrap()).unwrap(),
    )
    .await;
    for body in [
        json!({"messages":[]}),
        json!({"messages":[],"model":"remote"}),
        json!({"messages":[{"id":"x","role":"user","content":[{"type":"text","text":"  "}]}]}),
        json!({"messages":[{"id":"x","role":"tool","content":[]}]}),
        json!({"messages":[{"id":"x","role":"user","content":[{"type":"image","image":"anything"}]}]}),
    ] {
        let response = client()
            .post(format!("{api}/v1/chat"))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_client_error());
    }
    let response = client()
        .post(format!("{api}/v1/chat"))
        .header("content-type", "application/json")
        .body(" ".repeat(256 * 1024 + 1))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    task.abort();
}

#[test]
fn development_config_cannot_target_a_remote_provider() {
    for url in [
        "https://api.openai.com/v1",
        "http://example.com/v1",
        "http://127.0.0.1@evil.example/v1",
        "http://key@127.0.0.1/v1",
        "http://127.0.0.1/v1?secret=x",
    ] {
        assert!(Config::local(url, "test").is_err(), "{url}");
    }
    assert!(Config::local("http://[::1]:8081/v1", "test").is_ok());
}

#[tokio::test]
async fn dropping_response_closes_model_socket_before_headers_and_during_generation() {
    for after_headers in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (started, seen) = tokio::sync::oneshot::channel();
        let model = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).await.unwrap();
                request.push(byte[0]);
            }
            let header = String::from_utf8(request).unwrap();
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .unwrap()
                .parse()
                .unwrap();
            socket.read_exact(&mut vec![0; length]).await.unwrap();
            if after_headers {
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
                let data = chunk(&json!({"content":"one token"}), &Value::Null);
                socket
                    .write_all(format!("{:x}\r\n{data}\r\n", data.len()).as_bytes())
                    .await
                    .unwrap();
            }
            started.send(()).unwrap();
            let read = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut byte))
                .await
                .expect("upstream request survived cancellation");
            assert!(matches!(read, Ok(0) | Err(_)), "model socket must close");
        });
        let (api, api_task) = serve(
            ol_rht_chat::router(Config::local(&format!("http://{address}/v1"), "test").unwrap())
                .unwrap(),
        )
        .await;
        let mut response = client()
            .post(format!("{api}/v1/chat"))
            .json(&request())
            .send()
            .await
            .unwrap();
        seen.await.unwrap();
        if after_headers {
            loop {
                let bytes = response.chunk().await.unwrap().unwrap();
                if String::from_utf8_lossy(&bytes).contains("delta") {
                    break;
                }
            }
        }
        drop(response);
        model.await.unwrap();
        api_task.abort();
    }
}
