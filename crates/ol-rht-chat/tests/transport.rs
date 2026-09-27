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

fn usage_chunk(delta: &Value, reason: &Value) -> String {
    format!(
        "data: {}\n\n",
        json!({
            "choices": [{"index":0, "delta":delta, "finish_reason":reason}],
            "usage": {"prompt_tokens":10, "completion_tokens":3, "total_tokens":13}
        })
    )
}

fn with_usage(wire: &str, reason: &str) -> String {
    wire.replace(
        "data: [DONE]\n\n",
        &(usage_chunk(&json!({"role":"assistant", "content":""}), &json!(reason))
            + "data: [DONE]\n\n"),
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
        + &chunk(&json!({"reasoning":"…"}), &Value::Null)
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
    assert_eq!(output[2]["text"], "…");
    assert_eq!(output[3]["text"], "Answer 😀");
    assert_eq!(
        output.last().unwrap(),
        &json!({"type":"finish","reason":"length"})
    );
    model_task.abort();
    api_task.abort();
}

#[tokio::test]
async fn final_usage_chunks_preserve_the_answer_and_finish_reason() {
    for reason in ["stop", "length"] {
        for trailer in [
            usage_chunk(&json!({"content":"", "role":"assistant"}), &json!(reason)),
            usage_chunk(&json!({"content":null, "tool_calls":[]}), &json!(reason)),
            "data: {\"choices\":[],\"usage\":{\"total_tokens\":13}}\n\n".to_owned(),
        ] {
            let wire = chunk(&json!({"content":"Answer."}), &json!(reason))
                + &trailer
                + "data: [DONE]\n\n";
            let (model, model_task, _) = scripted_model(vec![wire]).await;
            let (api, api_task) = serve(
                ol_rht_chat::router(Config::local(&format!("{model}/v1"), "test").unwrap())
                    .unwrap(),
            )
            .await;
            let output = events(&api, request()).await;
            assert_eq!(output.len(), 3);
            assert_eq!(output[1]["text"], "Answer.");
            assert_eq!(output[2], json!({"type":"finish", "reason":reason}));
            model_task.abort();
            api_task.abort();
        }
    }
}

#[tokio::test]
async fn usage_chunks_do_not_hide_late_output_conflicting_finishes_or_errors() {
    let mut trailers = vec![
        usage_chunk(&json!({}), &json!("length")),
        chunk(&json!({}), &json!("stop")),
        "data: {\"error\":{\"message\":\"provider failure\"},\"choices\":[]}\n\n".to_owned(),
    ];
    for delta in [
        json!({"content":"late text"}),
        json!({"content":"\n"}),
        json!({"reasoning":"late reasoning"}),
        json!({"reasoning_content":"late reasoning"}),
        json!({"tool_calls":[{"index":0, "id":"late"}]}),
        json!({"function_call":{"name":"search"}}),
    ] {
        trailers.push(usage_chunk(&delta, &json!("stop")));
    }
    for trailer in trailers {
        let wire =
            chunk(&json!({"content":"Answer."}), &json!("stop")) + &trailer + "data: [DONE]\n\n";
        let (model, model_task, _) = scripted_model(vec![wire]).await;
        let (api, api_task) = serve(
            ol_rht_chat::router(Config::local(&format!("{model}/v1"), "test").unwrap()).unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        assert_eq!(output.len(), 3);
        assert_eq!(output[1]["text"], "Answer.");
        assert_eq!(output[2]["type"], "error");
        model_task.abort();
        api_task.abort();
    }
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
        assert!(
            Config::local("http://127.0.0.1:8081/v1", "test")
                .unwrap()
                .with_corpus(url)
                .is_err(),
            "{url}"
        );
    }
    assert!(Config::local("http://[::1]:8081/v1", "test").is_ok());
}

#[test]
fn openrouter_requires_a_key_and_keeps_corpus_on_loopback() {
    for key in ["", "   ", "secret\r\ninjected: header"] {
        let error = Config::openrouter(ol_rht_chat::OPENROUTER_MODEL, key).unwrap_err();
        assert!(!error.contains("secret"));
    }
    assert!(Config::openrouter(" ", "test-key").is_err());
    assert!(
        Config::openrouter(ol_rht_chat::OPENROUTER_MODEL, "test-key")
            .unwrap()
            .with_corpus("https://example.com")
            .is_err()
    );
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

type RequestLog = std::sync::Arc<std::sync::Mutex<Vec<Value>>>;

async fn scripted_model(wires: Vec<String>) -> (String, tokio::task::JoinHandle<()>, RequestLog) {
    let log = RequestLog::default();
    let captured = log.clone();
    let wires = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
        wires,
    )));
    let (url, task) = serve(Router::new().route(
        "/v1/chat/completions",
        post(move |Json(body): Json<Value>| {
            captured.lock().unwrap().push(body);
            let wire = wires
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected extra model call");
            async move { ([("content-type", "text/event-stream")], wire) }
        }),
    ))
    .await;
    (url, task, log)
}

fn search_call(args: &Value) -> String {
    let args = serde_json::to_string(args).unwrap();
    chunk(&json!({"content":"Looking."}), &Value::Null)
        + &chunk(
            &json!({"tool_calls":[{"index":0,"id":"model-call","type":"function","function":{"name":"sea","arguments":&args[..1]}}]}),
            &Value::Null,
        )
        + &chunk(
            &json!({"tool_calls":[{"index":0,"function":{"name":"rch","arguments":&args[1..]}}]}),
            &json!("tool_calls"),
        )
        + "data: [DONE]\n\n"
}

fn answer() -> String {
    chunk(&json!({"content":"Found the papers."}), &json!("stop")) + "data: [DONE]\n\n"
}

#[tokio::test]
async fn search_round_trip_and_followup_preserve_results_without_reexecuting_history() {
    let call = search_call(&json!({"query":"protein & λ","limit":2}));
    let reply = with_usage(&answer(), "stop");
    let (model, model_task, requests) =
        scripted_model(vec![with_usage(&call, "tool_calls"), reply, answer()]).await;
    let (sent, received) = tokio::sync::oneshot::channel();
    let sent = std::sync::Arc::new(std::sync::Mutex::new(Some(sent)));
    let (corpus, corpus_task) = serve(Router::new().route("/v1/search", axum::routing::get(move |axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>, headers: axum::http::HeaderMap| {
        assert!(!headers.contains_key("authorization"));
        sent.lock().unwrap().take().expect("historic search must not execute again").send(query).unwrap();
        async { Json(json!({"total":20,"answered":{"epoch":8,"batch":7,"at":"2026-09-19","sources":["pubmed"],"attribution":["NLM attribution"]},"hits":[
            {"public_id":"ol:paper","title":"Protein λ","summary":{"authors":["A","B","C","D","E","F","G"],"venue":"Journal","published":{"year":2026},"retracted_by":["pubmed"],"corrected_by":[],"concern_by":["crossref"]}}
        ]})) }
    }))).await;
    let (api, api_task) = serve(
        ol_rht_chat::router(
            Config::local(&format!("{model}/v1"), "test")
                .unwrap()
                .with_corpus(&corpus)
                .unwrap(),
        )
        .unwrap(),
    )
    .await;
    let output = events(&api, request()).await;
    assert_eq!(
        received.await.unwrap(),
        std::collections::HashMap::from([
            ("query".to_owned(), "protein & λ".to_owned()),
            ("limit".to_owned(), "2".to_owned()),
            ("hydrate".to_owned(), "true".to_owned()),
            ("facet_values".to_owned(), "1".to_owned())
        ])
    );
    assert_eq!(
        output
            .iter()
            .map(|e| e["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "start",
            "delta",
            "tool_call",
            "tool_result",
            "delta",
            "finish"
        ]
    );
    let id = &output[2]["part_id"];
    let result = &output[3]["result"]["data"];
    assert_eq!(&output[3]["part_id"], id);
    assert_ne!(output[1]["part_id"], output[4]["part_id"]);
    assert_eq!(result["hits"][0]["authors"].as_array().unwrap().len(), 6);
    assert_eq!(result["hits"][0]["retracted_by"], json!(["pubmed"]));
    assert_eq!(result["hits"][0]["concern_by"], json!(["crossref"]));
    assert_eq!(
        result["answered"]["attribution"],
        json!(["NLM attribution"])
    );
    {
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["tools"][0]["function"]["name"], "search");
        let messages = requests[1]["messages"].as_array().unwrap();
        assert_eq!(&messages[2]["tool_calls"][0]["id"], id);
        assert_eq!(messages[3]["role"], "tool");
        assert_eq!(&messages[3]["tool_call_id"], id);
        assert_eq!(
            &serde_json::from_str::<Value>(messages[3]["content"].as_str().unwrap()).unwrap(),
            result
        );
    }
    let history = json!({"messages":[
        request()["messages"][0],
        {"id":"a1","role":"assistant","content":[
            {"type":"text","text":"Looking."},
            {"type":"tool_call","tool_call_id":id,"call":output[2]["call"],"result":output[3]["result"]},
            {"type":"text","text":"Found the papers."},
            {"type":"tool_call","tool_call_id":"cancelled","call":{"name":"search","args":{"query":"cancelled","limit":5}},"result":null}
        ]},
        {"id":"u2","role":"user","content":[{"type":"text","text":"Which is retracted?"}]}
    ]});
    assert_eq!(
        events(&api, history).await.last().unwrap()["type"],
        "finish"
    );
    let captured = requests.lock().unwrap();
    let messages = captured[2]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 6);
    assert_eq!(&messages[2]["tool_calls"][0]["id"], id);
    assert_eq!(
        &serde_json::from_str::<Value>(messages[3]["content"].as_str().unwrap()).unwrap(),
        result
    );
    assert_eq!(messages[4]["content"], "Found the papers.");
    assert_eq!(messages[5]["content"], "Which is retracted?");
    for task in [model_task, corpus_task, api_task] {
        task.abort();
    }
}

#[tokio::test]
async fn search_errors_and_no_matches_are_tool_results_the_model_can_use() {
    for (status, body, expected) in [
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "private corpus error",
            "error",
        ),
        (StatusCode::BAD_REQUEST, "private query error", "error"),
        (StatusCode::OK, "not JSON", "error"),
        (
            StatusCode::OK,
            r#"{"total":0,"hits":[],"answered":null}"#,
            "ok",
        ),
    ] {
        let (model, model_task, requests) =
            scripted_model(vec![search_call(&json!({"query":"protein"})), answer()]).await;
        let (corpus, corpus_task) = serve(Router::new().route(
            "/v1/search",
            axum::routing::get(move || async move { (status, body) }),
        ))
        .await;
        let (api, api_task) = serve(
            ol_rht_chat::router(
                Config::local(&format!("{model}/v1"), "test")
                    .unwrap()
                    .with_corpus(&corpus)
                    .unwrap(),
            )
            .unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        assert_eq!(output[2]["call"]["args"]["limit"], 5);
        assert_eq!(output[3]["result"]["data"]["status"], expected);
        assert_eq!(output.last().unwrap()["type"], "finish");
        assert!(!output[3].to_string().contains("private"));
        assert_eq!(requests.lock().unwrap().len(), 2);
        for task in [model_task, corpus_task, api_task] {
            task.abort();
        }
    }
}

#[tokio::test]
async fn tool_budget_allows_thirty_calls_then_requires_an_answer() {
    let budget = 30;
    for ignores_budget in [false, true] {
        let call = search_call(&json!({"query":"protein"}));
        let mut wires = vec![call.clone(); budget];
        wires.push(if ignores_budget { call } else { answer() });
        let (model, model_task, requests) = scripted_model(wires).await;
        let (corpus, corpus_task) = serve(Router::new().route(
            "/v1/search",
            axum::routing::get(|| async { Json(json!({"total":0,"hits":[],"answered":null})) }),
        ))
        .await;
        let (api, api_task) = serve(
            ol_rht_chat::router(
                Config::local(&format!("{model}/v1"), "test")
                    .unwrap()
                    .with_corpus(&corpus)
                    .unwrap(),
            )
            .unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        assert_eq!(
            output.iter().filter(|e| e["type"] == "tool_result").count(),
            budget
        );
        assert_eq!(
            output.last().unwrap()["type"],
            if ignores_budget { "error" } else { "finish" }
        );
        {
            let requests = requests.lock().unwrap();
            assert_eq!(requests[budget - 1]["tool_choice"], "auto");
            assert_eq!(
                requests[budget - 1]["messages"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["role"],
                "tool"
            );
            assert_eq!(requests[budget]["tool_choice"], "none");
            let final_instruction = requests[budget]["messages"]
                .as_array()
                .unwrap()
                .last()
                .unwrap();
            assert_eq!(final_instruction["role"], "user");
            assert!(
                final_instruction["content"]
                    .as_str()
                    .unwrap()
                    .contains("tool budget for this reply is now exhausted")
            );
            assert!(
                final_instruction["content"]
                    .as_str()
                    .unwrap()
                    .contains("Answer the user now using those results")
            );
            assert!(!output.iter().any(|event| {
                event["text"].as_str().is_some_and(|text| {
                    text.contains("tool budget for this reply is now exhausted")
                })
            }));
        }
        for task in [model_task, corpus_task, api_task] {
            task.abort();
        }
    }
}

#[tokio::test]
async fn incomplete_or_invalid_calls_never_execute() {
    let good = search_call(&json!({"query":"protein"}));
    let mut wires = vec![
        good.replace("data: [DONE]\n\n", ""),
        with_usage(&good, "tool_calls").replace("data: [DONE]\n\n", ""),
        good.replace("tool_calls\"}]", "stop\"}]"),
        good.replace("tool_calls\"}]", "length\"}]"),
        good.replace("rch", "rch_unknown"),
    ];
    for args in [
        json!({"query":""}),
        json!({"query":"a","limit":11}),
        json!({"query":"a","limit":0}),
        json!({"query":"a","url":"http://example.com"}),
        json!({"query":"λ".repeat(257)}),
    ] {
        wires.push(search_call(&args));
    }
    for wire in wires {
        let length = wire.contains("\"finish_reason\":\"length\"");
        let (model, model_task, _) = scripted_model(vec![wire]).await;
        let (api, api_task) = serve(
            ol_rht_chat::router(
                Config::local(&format!("{model}/v1"), "test")
                    .unwrap()
                    .with_corpus("http://127.0.0.1:1")
                    .unwrap(),
            )
            .unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        assert!(!output.iter().any(|e| e["type"] == "tool_call"));
        assert_eq!(
            output.last().unwrap()["type"],
            if length { "finish" } else { "error" }
        );
        model_task.abort();
        api_task.abort();
    }
}

#[tokio::test]
async fn dropping_response_cancels_the_search_socket() {
    let (model, model_task, _) =
        scripted_model(vec![search_call(&json!({"query":"protein"}))]).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (started, seen) = tokio::sync::oneshot::channel();
    let corpus_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).await.unwrap();
            request.push(byte[0]);
        }
        started.send(()).unwrap();
        let read = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut byte))
            .await
            .expect("search survived cancellation");
        assert!(matches!(read, Ok(0) | Err(_)));
    });
    let (api, api_task) = serve(
        ol_rht_chat::router(
            Config::local(&format!("{model}/v1"), "test")
                .unwrap()
                .with_corpus(&format!("http://{address}"))
                .unwrap(),
        )
        .unwrap(),
    )
    .await;
    let response = client()
        .post(format!("{api}/v1/chat"))
        .json(&request())
        .send()
        .await
        .unwrap();
    seen.await.unwrap();
    drop(response);
    corpus_task.await.unwrap();
    model_task.abort();
    api_task.abort();
}

fn paper_call(id: &str) -> String {
    chunk(
        &json!({"tool_calls":[{"index":0,"id":"model-paper","type":"function","function":{"name":"get_paper","arguments":json!({"id":id}).to_string()}}]}),
        &json!("tool_calls"),
    ) + "data: [DONE]\n\n"
}

fn paper_fixture() -> Value {
    json!({"answered":{"epoch":8,"batch":7,"at":"2026-09-19","sources":["pubmed"],"attribution":["NLM attribution"]}, "work":{
        "public_id":"ol:paper", "stub":false, "title":"Paper title", "authors":["First Author","Second Author"], "venue":"Journal",
        "published":{"year":2026,"month":9,"day":1}, "abstract":"Available abstract.", "open_access":true,
        "retracted_by":["pubmed"], "corrected_by":[], "concern_by":[],
        "names":[
            {"namespace":"pmc","value":"PMCwrong","standing":"alias"},
            {"namespace":"pmc","value":"PMC123456","standing":"promoted"},
            {"namespace":"doi","value":"10.1000/paper?a#b%","standing":"promoted"}
        ]
    }})
}

#[tokio::test]
async fn paper_lookup_accepts_all_supported_id_names_and_supplies_citation_data() {
    for id in [
        "ol:paper",
        "doi:10.1000/paper?a#b%",
        "pmid:123",
        "pmc:PMC123456",
        "openalex:W123",
        "reg1016:S123456789",
    ] {
        let (model, model_task, requests) = scripted_model(vec![paper_call(id), answer()]).await;
        let (corpus, corpus_task) = serve(Router::new().route(
            "/v1/work",
            axum::routing::get(
                move |axum::extract::Query(query): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >,
                      headers: axum::http::HeaderMap| async move {
                    assert_eq!(
                        query,
                        std::collections::HashMap::from([("work".to_owned(), id.to_owned())])
                    );
                    assert!(!headers.contains_key("authorization"));
                    Json(paper_fixture())
                },
            ),
        ))
        .await;
        let (api, api_task) = serve(
            ol_rht_chat::router(
                Config::local(&format!("{model}/v1"), "test")
                    .unwrap()
                    .with_corpus(&corpus)
                    .unwrap(),
            )
            .unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        assert_eq!(
            output[1]["call"],
            json!({"name":"get_paper","args":{"id":id}})
        );
        let result = &output[2]["result"]["data"];
        let paper = &result["paper"];
        assert_eq!(paper["public_id"], "ol:paper");
        assert_eq!(paper["authors"], json!(["First Author", "Second Author"]));
        assert_eq!(paper["published"]["year"], 2026);
        assert_eq!(paper["abstract_text"], "Available abstract.");
        assert_eq!(
            paper["full_text_url"],
            "https://pmc.ncbi.nlm.nih.gov/articles/PMC123456/"
        );
        assert_eq!(
            paper["publisher_url"],
            "https://doi.org/10.1000/paper%3Fa%23b%25"
        );
        assert_eq!(paper["retracted_by"], json!(["pubmed"]));
        assert_eq!(result["answered"]["epoch"], 8);
        assert_eq!(output.last().unwrap()["type"], "finish");
        let captured = requests.lock().unwrap();
        assert_eq!(
            captured[1]["messages"][2]["tool_calls"][0]["function"]["name"],
            "get_paper"
        );
        let received: Value =
            serde_json::from_str(captured[1]["messages"][3]["content"].as_str().unwrap()).unwrap();
        assert_eq!(&received, result);
        assert!(
            captured[0]["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("preferring its ol: public_id")
        );
        for task in [model_task, corpus_task, api_task] {
            task.abort();
        }
    }
}

#[tokio::test]
async fn paper_links_require_promoted_names_and_truncation_is_explicit() {
    for stub in [false, true] {
        let (model, model_task, _) = scripted_model(vec![paper_call("ol:paper"), answer()]).await;
        let mut fixture = paper_fixture();
        fixture["work"]["stub"] = json!(stub);
        fixture["work"]["names"] = if stub {
            json!([])
        } else {
            json!([
                {"namespace":"pmc","value":"PMC123","standing":"shared"},
                {"namespace":"doi","value":"10.1000/real","standing":"promoted"}
            ])
        };
        fixture["work"]["abstract"] = json!(if stub {
            String::new()
        } else {
            "λ".repeat(8100)
        });
        fixture["work"]["authors"] = if stub {
            json!([])
        } else {
            json!((0..51).map(|n| format!("Author {n}")).collect::<Vec<_>>())
        };
        let (corpus, corpus_task) = serve(Router::new().route(
            "/v1/work",
            axum::routing::get(move || {
                let body = fixture.clone();
                async move { Json(body) }
            }),
        ))
        .await;
        let (api, api_task) = serve(
            ol_rht_chat::router(
                Config::local(&format!("{model}/v1"), "test")
                    .unwrap()
                    .with_corpus(&corpus)
                    .unwrap(),
            )
            .unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        let paper = &output[2]["result"]["data"]["paper"];
        assert_eq!(paper["stub"], stub);
        assert!(paper["full_text_url"].is_null());
        assert_eq!(paper["publisher_url"].is_null(), stub);
        assert_eq!(paper["abstract_truncated"], !stub);
        assert_eq!(paper["authors_truncated"], !stub);
        if !stub {
            assert_eq!(
                paper["abstract_text"].as_str().unwrap().chars().count(),
                8000
            );
            assert_eq!(paper["authors"].as_array().unwrap().len(), 50);
        }
        for task in [model_task, corpus_task, api_task] {
            task.abort();
        }
    }
}

#[tokio::test]
async fn absent_shared_retired_and_unavailable_papers_are_explicit_tool_errors() {
    for (status, body, message) in [
        (StatusCode::NOT_FOUND, "private", "No single paper"),
        (
            StatusCode::BAD_REQUEST,
            r#"{"code":"failed_precondition","message":"private"}"#,
            "shared identifier",
        ),
        (
            StatusCode::BAD_REQUEST,
            r#"{"code":"invalid_argument","message":"private"}"#,
            "syntax",
        ),
        (StatusCode::SERVICE_UNAVAILABLE, "private", "failed"),
        (
            StatusCode::OK,
            r#"{"work":null,"answered":null}"#,
            "No paper",
        ),
    ] {
        let (model, model_task, _) = scripted_model(vec![paper_call("ol:unknown"), answer()]).await;
        let (corpus, corpus_task) = serve(Router::new().route(
            "/v1/work",
            axum::routing::get(move || async move { (status, body) }),
        ))
        .await;
        let (api, api_task) = serve(
            ol_rht_chat::router(
                Config::local(&format!("{model}/v1"), "test")
                    .unwrap()
                    .with_corpus(&corpus)
                    .unwrap(),
            )
            .unwrap(),
        )
        .await;
        let output = events(&api, request()).await;
        let result = &output[2]["result"]["data"];
        assert_eq!(result["status"], "error");
        assert!(result["message"].as_str().unwrap().contains(message));
        assert!(!result.to_string().contains("private"));
        assert_eq!(output.last().unwrap()["type"], "finish");
        for task in [model_task, corpus_task, api_task] {
            task.abort();
        }
    }
}
