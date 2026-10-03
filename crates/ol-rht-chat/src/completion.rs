//! One model completion, including fragmented function arguments. A tool is
//! executable only after a matching finish reason and the final SSE sentinel.

use serde::Deserialize;
use serde_json::{Value, json};
use tokio_stream::Stream;

use crate::protocol::{PartKind, ToolCall};
use crate::{App, Provider, sse, tool};

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
        let mut response = request(app, messages, remaining)
            .send().await.map_err(|error| {
                if error.is_timeout() { "The model provider timed out.".to_owned() }
                else if app.config.provider == Provider::Local { "Could not reach the local model. Start `just chat-model`.".to_owned() }
                else { "Could not reach OpenRouter. Check your network connection.".to_owned() }
            })?;
        if !response.status().is_success() {
            let hint = match response.status().as_u16() {
                401 | 403 => "Check the configured model API key and provider access.",
                402 => "Check your provider account's credits and model access.",
                429 => "The provider rate limit was reached. Try again later.",
                _ => "Check model availability and the conversation's context length.",
            };
            Err(format!("The model provider refused the request (HTTP {}). {hint}", response.status().as_u16()))?;
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
                    if let Some(reason) = finish.as_deref() {
                        // OpenRouter's final usage chunk repeats the finish reason
                        // with an empty delta. It is accounting, not new output.
                        if chunk.usage.as_ref().is_some_and(Value::is_object)
                            && choice.finish_reason.as_deref() == Some(reason)
                            && choice.delta.is_empty()
                        {
                            continue;
                        }
                        Err("The model sent a choice after finishing.".to_owned())?;
                    }
                    if choice.delta.function_call.is_some() { Err("Legacy function calls are not supported. Use tool_calls.".to_owned())?; }
                    for (kind, value) in [(PartKind::Reasoning, choice.delta.reasoning_content.or(choice.delta.reasoning)), (PartKind::Text, choice.delta.content)] {
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

fn request(app: &App, messages: &[Value], remaining: usize) -> reqwest::RequestBuilder {
    let mut body = json!({
        "model": app.config.model, "messages": messages, "stream": true, "n": 1, "max_tokens": 65_536,
        "tools": tool::definitions(), "tool_choice": if remaining == 0 { "none" } else { "auto" },
        "parallel_tool_calls": false
    });
    if app.config.provider == Provider::OpenRouter {
        // Match the local recipe's reasoning-off behavior.
        body["reasoning"] = json!({"enabled": false});
        if let Some(provider) = &app.config.openrouter_provider {
            body["provider"] = json!({"only": [provider], "allow_fallbacks": false});
        }
    }
    app.client
        .post(app.config.endpoint.clone())
        .header(
            reqwest::header::AUTHORIZATION,
            app.config.authorization.clone(),
        )
        .json(&body)
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
    usage: Option<Value>,
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
    reasoning: Option<String>,
    tool_calls: Option<Vec<CallDelta>>,
    function_call: Option<Value>,
}

impl Delta {
    fn is_empty(&self) -> bool {
        [&self.content, &self.reasoning_content, &self.reasoning]
            .into_iter()
            .all(|text| text.as_ref().is_none_or(String::is_empty))
            && self.tool_calls.as_ref().is_none_or(Vec::is_empty)
            && self.function_call.is_none()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, DEV_KEY, OPENROUTER_MODEL};

    #[test]
    fn provider_requests_keep_credentials_out_of_body_and_corpus_requests() {
        let key = "test-openrouter-secret";
        for (config, expected_key, endpoint) in [
            (
                Config::openrouter(OPENROUTER_MODEL, key)
                    .unwrap()
                    .with_openrouter_provider("nvidia")
                    .unwrap(),
                key,
                "https://openrouter.ai/api/v1/chat/completions",
            ),
            (
                Config::openrouter(OPENROUTER_MODEL, key).unwrap(),
                key,
                "https://openrouter.ai/api/v1/chat/completions",
            ),
            (
                Config::local("http://127.0.0.1:8081/v1", "local-gemma").unwrap(),
                DEV_KEY,
                "http://127.0.0.1:8081/v1/chat/completions",
            ),
        ] {
            let app = App {
                admission: None,
                config,
                client: reqwest::Client::new(),
                storage: None,
            };
            assert!(!format!("{:?}", app.config).contains(expected_key));
            for remaining in [0, 6] {
                let request = request(&app, &[json!({"role":"user", "content":"Hi"})], remaining)
                    .build()
                    .unwrap();
                assert_eq!(request.url().as_str(), endpoint);
                assert_eq!(
                    request.headers()["authorization"],
                    format!("Bearer {expected_key}")
                );
                assert!(request.headers()["authorization"].is_sensitive());
                let body: Value =
                    serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
                assert!(!body.to_string().contains(expected_key));
                assert_eq!(body["model"], app.config.model);
                assert_eq!(body["stream"], true);
                assert_eq!(
                    body["tool_choice"],
                    if remaining == 0 { "none" } else { "auto" }
                );
                assert_eq!(body["tools"].as_array().unwrap().len(), 2);
                if app.config.provider == Provider::OpenRouter {
                    assert_eq!(body["reasoning"], json!({"enabled": false}));
                } else {
                    assert!(body.get("reasoning").is_none());
                }
                if let Some(provider) = &app.config.openrouter_provider {
                    assert_eq!(
                        body["provider"],
                        json!({"only": [provider], "allow_fallbacks": false})
                    );
                } else {
                    assert!(body.get("provider").is_none());
                }
            }
            for url in [app.config.corpus, app.config.work] {
                let corpus_request = app.client.get(url).build().unwrap();
                assert!(!corpus_request.headers().contains_key("authorization"));
            }
        }
    }
}
