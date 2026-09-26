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
    Text {
        text: String,
    },
    Reasoning {
        text: String,
    },
    ToolCall {
        tool_call_id: String,
        call: ToolCall,
        result: Option<ToolResult>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "name",
    content = "args",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ToolCall {
    Search(SearchArgs),
    GetPaper(PaperArgs),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(
    tag = "name",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ToolResult {
    Search(SearchResult),
    GetPaper(PaperResult),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PaperArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum PaperResult {
    Ok {
        paper: Box<Paper>,
        answered: Option<CorpusProvenance>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Paper {
    pub public_id: String,
    pub stub: bool,
    pub title: String,
    pub authors: Vec<String>,
    pub authors_truncated: bool,
    pub venue: String,
    pub published: Option<PaperDate>,
    pub identifiers: Vec<PaperIdentifier>,
    pub abstract_text: String,
    pub abstract_truncated: bool,
    pub retracted_by: Vec<String>,
    pub corrected_by: Vec<String>,
    pub concern_by: Vec<String>,
    pub open_access: Option<bool>,
    pub full_text_url: Option<String>,
    pub publisher_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PaperDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PaperIdentifier {
    pub namespace: String,
    pub value: String,
    pub standing: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SearchArgs {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u8,
}

fn default_limit() -> u8 {
    5
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SearchResult {
    Ok {
        query: String,
        total: u64,
        hits: Vec<SearchPaper>,
        answered: Option<CorpusProvenance>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SearchPaper {
    pub public_id: String,
    pub title: String,
    /// At most six authors, in source order, to keep model context bounded.
    pub authors: Vec<String>,
    pub venue: String,
    pub year: Option<i32>,
    pub retracted_by: Vec<String>,
    pub corrected_by: Vec<String>,
    pub concern_by: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CorpusProvenance {
    pub epoch: u64,
    pub batch: u64,
    pub at: String,
    pub sources: Vec<String>,
    pub attribution: Vec<String>,
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
    /// Complete, validated arguments. Execution belongs to the server.
    ToolCall {
        part_id: String,
        call: ToolCall,
    },
    ToolResult {
        part_id: String,
        result: ToolResult,
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
        let cfg = Config::default().with_large_int("number");
        let declarations = [
            ChatRequest::decl(&cfg),
            ChatMessage::decl(&cfg),
            Role::decl(&cfg),
            ContentPart::decl(&cfg),
            ToolCall::decl(&cfg),
            ToolResult::decl(&cfg),
            PaperArgs::decl(&cfg),
            PaperResult::decl(&cfg),
            Paper::decl(&cfg),
            PaperDate::decl(&cfg),
            PaperIdentifier::decl(&cfg),
            SearchArgs::decl(&cfg),
            SearchResult::decl(&cfg),
            SearchPaper::decl(&cfg),
            CorpusProvenance::decl(&cfg),
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
