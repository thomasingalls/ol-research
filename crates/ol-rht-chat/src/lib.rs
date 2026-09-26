//! First chat transport: one selected conversation branch in, structured
//! deltas out. The HTTP response owns the upstream request; no detached
//! generation task survives a browser disconnect. Local development only.

pub mod protocol;
mod sse;

use std::collections::HashSet;
use std::convert::Infallible;
use std::net::IpAddr;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use protocol::{ChatEvent, ChatRequest, ContentPart, FinishReason, PartKind, Role};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_stream::{Stream, StreamExt};

/// An inert value shared with `just chat-model`, never read from a secret store.
pub const DEV_KEY: &str = "local-dev-only";
const SYSTEM: &str = "You are a helpful local development assistant. You have no access to the application's literature database or tools. Do not claim to search it.";
static NEXT_MESSAGE: AtomicU64 = AtomicU64::new(1);

/// Model settings belong to the server, not to arbitrary browser requests.
#[derive(Debug, Clone)]
pub struct Config {
    endpoint: reqwest::Url,
    model: String,
}

impl Config {
    /// An OpenAI-compatible `/v1` base URL on loopback, with a model alias.
    ///
    /// # Errors
    /// Non-local URLs, embedded credentials and empty model names are refused.
    pub fn local(base: &str, model: &str) -> Result<Self, String> {
        let endpoint =
            reqwest::Url::parse(&format!("{}/chat/completions", base.trim_end_matches('/')))
                .map_err(|e| e.to_string())?;
        let loopback = endpoint.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .trim_matches(['[', ']'])
                    .parse::<IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if endpoint.scheme() != "http"
            || !loopback
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err("The development model URL must be HTTP on loopback, without credentials, query or fragment.".to_owned());
        }
        if model.trim().is_empty() {
            return Err("A model name is required.".to_owned());
        }
        Ok(Self {
            endpoint,
            model: model.to_owned(),
        })
    }
}

#[derive(Debug)]
struct App {
    config: Config,
    client: reqwest::Client,
}

/// The standalone chat router. The corpus service need not be running.
///
/// # Errors
/// An HTTP client could not be constructed.
pub fn router(config: Config) -> Result<Router, reqwest::Error> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(120))
        .build()?;
    Ok(Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/chat", post(chat))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(Arc::new(App { config, client })))
}

#[allow(clippy::unused_async)]
async fn chat(
    State(app): State<Arc<App>>,
    request: Result<Json<ChatRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => {
            return (
                error.status(),
                Json(json!({ "message": error.body_text() })),
            )
                .into_response();
        }
    };
    let messages = match model_messages(&request) {
        Ok(messages) => messages,
        Err(message) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "message": message }))).into_response();
        }
    };
    let events = model_events(app, messages);
    // Own the entire request inside the response body. This also makes Stop
    // work before upstream headers arrive (during model queueing/prefill).
    let body = async_stream::stream! {
        let mut events = pin!(events);
        while let Some(event) = events.next().await {
            let failed = event.is_err();
            let event = event.unwrap_or_else(|message| ChatEvent::Error { message });
            let mut line = serde_json::to_string(&event).expect("chat events contain only JSON-safe values");
            line.push('\n');
            yield Ok::<_, Infallible>(line);
            if failed { break; }
        }
    };
    (
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
            (header::HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        Body::from_stream(body),
    )
        .into_response()
}

fn model_messages(request: &ChatRequest) -> Result<Vec<Value>, &'static str> {
    if request.messages.is_empty() || request.messages.len() > 128 {
        return Err("Send between 1 and 128 messages.");
    }
    if request.messages.last().is_none_or(|m| m.role != Role::User) {
        return Err("The selected branch must end with a user message.");
    }
    let mut ids = HashSet::new();
    let mut bytes = 0;
    let mut out = vec![json!({ "role": "system", "content": SYSTEM })];
    for message in &request.messages {
        if message.id.is_empty() || message.id.len() > 128 || !ids.insert(&message.id) {
            return Err("Messages need distinct, nonempty ids of at most 128 bytes.");
        }
        if message.content.is_empty() || message.content.len() > 128 {
            return Err("Messages need between 1 and 128 content parts.");
        }
        let mut text = String::new();
        for part in &message.content {
            match part {
                ContentPart::Text { text: value } => {
                    bytes += value.len();
                    text.push_str(value);
                }
                ContentPart::Reasoning { text: value } => {
                    if message.role != Role::Assistant {
                        return Err("Only assistant messages may contain reasoning.");
                    }
                    // Retained in the application's history, but not replayed
                    // as user-visible content to templates without support.
                    bytes += value.len();
                }
            }
        }
        if bytes > 64 * 1024 {
            return Err("Conversation exceeds the 64 KiB text limit. Start a new chat.");
        }
        if text.trim().is_empty() {
            // A stopped thinking-only reply contributes no answer to history.
            if message.role == Role::Assistant {
                continue;
            }
            return Err("User messages need nonempty text.");
        }
        let role = if message.role == Role::User {
            "user"
        } else {
            "assistant"
        };
        out.push(json!({ "role": role, "content": text }));
    }
    Ok(out)
}

fn model_events(
    app: Arc<App>,
    messages: Vec<Value>,
) -> impl Stream<Item = Result<ChatEvent, String>> + Send {
    async_stream::try_stream! {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let id = format!("chat-{time:x}-{:x}", NEXT_MESSAGE.fetch_add(1, Ordering::Relaxed));
        yield ChatEvent::Start { message_id: id.clone() };
        let mut response = app.client.post(app.config.endpoint.clone())
            .bearer_auth(DEV_KEY)
            .json(&json!({ "model": app.config.model, "messages": messages, "stream": true, "n": 1, "max_tokens": 1024 }))
            .send().await.map_err(|error| {
                if error.is_timeout() { "The local model timed out.".to_owned() }
                else { "Could not reach the local model. Start `just chat-model`.".to_owned() }
            })?;
        if !response.status().is_success() {
            Err(format!("The local model refused the request (HTTP {}). Check its log and the conversation's context length.", response.status().as_u16()))?;
        }
        if !response.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/event-stream")) {
            Err("The model did not return an SSE stream.".to_owned())?;
        }
        let mut decoder = sse::Decoder::default();
        let mut parts = Parts::new(id);
        let mut finish = None;
        while let Some(bytes) = response.chunk().await.map_err(|_| "The model stream was interrupted or timed out.".to_owned())? {
            for data in decoder.push(&bytes).map_err(str::to_owned)? {
                if data == "[DONE]" {
                    let reason = finish.ok_or("The model stream ended without a finish reason.")?;
                    yield ChatEvent::Finish { reason };
                    return;
                }
                let chunk: Chunk = serde_json::from_str(&data).map_err(|_| "The model sent an invalid stream event.".to_owned())?;
                if chunk.error.is_some() { Err("The model reported an error while generating.".to_owned())?; }
                if chunk.choices.len() > 1 { Err("Multiple model choices are not supported yet.".to_owned())?; }
                for choice in chunk.choices {
                    if choice.delta.tool_calls.is_some() || choice.delta.function_call.is_some() {
                        Err("The model requested a tool, but tool execution is not connected yet.".to_owned())?;
                    }
                    for (kind, value) in [(PartKind::Reasoning, choice.delta.reasoning_content), (PartKind::Text, choice.delta.content)] {
                        if let Some(text) = value.filter(|t| !t.is_empty()) {
                            if finish.is_some() { Err("The model sent content after finishing.".to_owned())?; }
                            yield parts.delta(kind, text);
                        }
                    }
                    if let Some(reason) = choice.finish_reason {
                        finish = Some(match reason.as_str() {
                            "stop" => FinishReason::Stop,
                            "length" => FinishReason::Length,
                            _ => Err(format!("Unsupported model finish reason: {reason}"))?,
                        });
                    }
                }
            }
        }
        Err("The model connection ended before [DONE]. The reply may be incomplete.".to_owned())?;
    }
}

#[derive(Debug, Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<Choice>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Value>,
    function_call: Option<Value>,
}

#[derive(Debug)]
struct Parts {
    message_id: String,
    kind: Option<PartKind>,
    index: usize,
}

impl Parts {
    fn new(message_id: String) -> Self {
        Self {
            message_id,
            kind: None,
            index: 0,
        }
    }

    fn delta(&mut self, kind: PartKind, text: String) -> ChatEvent {
        if self.kind.is_some_and(|previous| previous != kind) {
            self.index += 1;
        }
        self.kind = Some(kind);
        ChatEvent::Delta {
            part_id: format!("{}-{}", self.message_id, self.index),
            kind,
            text,
        }
    }
}
