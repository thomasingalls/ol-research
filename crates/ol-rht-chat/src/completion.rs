//! One model completion, including fragmented function arguments. A tool is
//! executable only after a matching finish reason and the final SSE sentinel.

use serde::Deserialize;
use serde_json::{Value, json};
use tokio_stream::Stream;

use crate::protocol::{PartKind, ToolCall};
use crate::{App, DEV_KEY, sse, tool};

pub(crate) enum Event {
    Delta {
        kind: PartKind,
        text: String,
    },
    Done {
        reason: String,
        calls: Vec<ToolCall>,
    },
}

pub(crate) fn run<'a>(
    app: &'a App,
    messages: &'a [Value],
    remaining: usize,
) -> impl Stream<Item = Result<Event, String>> + Send + 'a {
    async_stream::try_stream! {
        let mut response = app.client.post(app.config.endpoint.clone())
            .bearer_auth(DEV_KEY)
            .json(&json!({
                "model": app.config.model, "messages": messages, "stream": true, "n": 1, "max_tokens": 1024,
                "tools": tool::definitions(), "tool_choice": if remaining == 0 { "none" } else { "auto" },
                "parallel_tool_calls": false
            }))
            .send().await.map_err(|error| {
                if error.is_timeout() { "The local model timed out.".to_owned() }
                else { "Could not reach the local model. Start `just chat-model`.".to_owned() }
            })?;
        if !response.status().is_success() {
            Err(format!("The local model refused the request (HTTP {}). Check its log and the conversation's context length.", response.status().as_u16()))?;
        }
        if !response.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/event-stream")) {
            Err("The model did not return an SSE stream.".to_owned())?;
        }
        let mut decoder = sse::Decoder::default();
        let mut calls = Vec::<Call>::new();
        let mut finish: Option<String> = None;
        while let Some(bytes) = response.chunk().await.map_err(|_| "The model stream was interrupted or timed out.".to_owned())? {
            for data in decoder.push(&bytes).map_err(str::to_owned)? {
                if data == "[DONE]" {
                    let reason = finish.ok_or("The model stream ended without a finish reason.")?;
                    // Truncated arguments never execute, even if a JSON prefix
                    // happens to parse. A length finish still reaches the UI.
                    let calls = if reason == "length" { vec![] } else {
                        if (reason == "tool_calls") == calls.is_empty() {
                            Err("The model's tool calls did not match its finish reason.")?;
                        }
                        calls.into_iter().map(Call::finish).collect::<Result<Vec<_>, _>>()?
                    };
                    yield Event::Done { reason, calls };
                    return;
                }
                let chunk: Chunk = serde_json::from_str(&data).map_err(|_| "The model sent an invalid stream event.".to_owned())?;
                if chunk.error.is_some() { Err("The model reported an error while generating.".to_owned())?; }
                if chunk.choices.len() > 1 { Err("Multiple model choices are not supported yet.".to_owned())?; }
                for choice in chunk.choices {
                    if finish.is_some() { Err("The model sent a choice after finishing.".to_owned())?; }
                    if choice.delta.function_call.is_some() { Err("Legacy function calls are not supported. Use tool_calls.".to_owned())?; }
                    for (kind, value) in [(PartKind::Reasoning, choice.delta.reasoning_content), (PartKind::Text, choice.delta.content)] {
                        if let Some(text) = value.filter(|t| !t.is_empty()) { yield Event::Delta { kind, text }; }
                    }
                    for delta in choice.delta.tool_calls.unwrap_or_default() {
                        if delta.index >= remaining || delta.index > calls.len() {
                            Err("The model exceeded the tool budget or sent out-of-order tool calls.".to_owned())?;
                        }
                        if delta.index == calls.len() { calls.push(Call::default()); }
                        calls[delta.index].append(delta)?;
                    }
                    if let Some(reason) = choice.finish_reason {
                        if !matches!(reason.as_str(), "stop" | "length" | "tool_calls") {
                            Err(format!("Unsupported model finish reason: {reason}"))?;
                        }
                        finish = Some(reason);
                    }
                }
            }
        }
        Err("The model connection ended before [DONE]. The reply may be incomplete.".to_owned())?;
    }
}

#[derive(Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
}

impl Call {
    fn append(&mut self, delta: CallDelta) -> Result<(), &'static str> {
        if delta.kind.is_some_and(|kind| kind != "function") {
            return Err("Unsupported model tool type.");
        }
        self.id.push_str(delta.id.as_deref().unwrap_or_default());
        if let Some(function) = delta.function {
            self.name
                .push_str(function.name.as_deref().unwrap_or_default());
            self.arguments
                .push_str(function.arguments.as_deref().unwrap_or_default());
        }
        if self.id.len() > 128 || self.name.len() > 64 || self.arguments.len() > 4096 {
            return Err("The model's tool call exceeded the size limit.");
        }
        Ok(())
    }

    fn finish(self) -> Result<ToolCall, &'static str> {
        if self.id.is_empty() {
            return Err("The model requested an unidentified tool.");
        }
        let args: Value = serde_json::from_str(&self.arguments)
            .map_err(|_| "The model sent invalid tool arguments.")?;
        let call: ToolCall = serde_json::from_value(json!({"name":self.name,"args":args}))
            .map_err(|_| "The model requested an unknown tool or invalid arguments. Only search and get_paper are available.")?;
        call.validate()?;
        Ok(call)
    }
}

#[derive(Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<Choice>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<CallDelta>>,
    function_call: Option<Value>,
}

#[derive(Deserialize)]
struct CallDelta {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}
