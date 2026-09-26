//! The application protocol, independent of both assistant-ui and llama.cpp.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ChatRequest {
    /// The selected branch, in chronological order; never the whole branch tree.
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ChatMessage {
    pub id: String,
    pub role: Role,
    pub content: Vec<ContentPart>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ContentPart {
    Text { text: String },
    Reasoning { text: String },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum PartKind {
    Text,
    Reasoning,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ChatEvent {
    Start {
        message_id: String,
    },
    /// Consecutive deltas with the same `part_id` append to that part. A new
    /// kind starts a new part, preserving interleaved reasoning and text.
    Delta {
        part_id: String,
        kind: PartKind,
        text: String,
    },
    Finish {
        reason: FinishReason,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum FinishReason {
    Stop,
    Length,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    use ts_rs::{Config, TS};

    fn bindings() -> String {
        let cfg = Config::default();
        let declarations = [
            ChatRequest::decl(&cfg),
            ChatMessage::decl(&cfg),
            Role::decl(&cfg),
            ContentPart::decl(&cfg),
            PartKind::decl(&cfg),
            ChatEvent::decl(&cfg),
            FinishReason::decl(&cfg),
        ];
        let mut out = String::from(
            "// Generated from crates/ol-rht-chat/src/protocol.rs. Run `just chat-types`.\n",
        );
        for declaration in declarations {
            writeln!(out, "export {declaration}").unwrap();
        }
        out.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    const FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/src/generated/chat.ts"
    );

    #[test]
    fn committed_types_match() {
        assert_eq!(
            std::fs::read_to_string(FILE).unwrap(),
            bindings(),
            "run just chat-types"
        );
    }

    #[test]
    #[ignore = "regenerates the committed browser contract"]
    fn write_types() {
        std::fs::write(FILE, bindings()).unwrap();
    }
}
