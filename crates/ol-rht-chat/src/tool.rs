//! The server's read-only tool allowlist. Browser history never executes here.

use serde_json::{Value, json};

use crate::protocol::{ToolCall, ToolResult};
use crate::{App, paper, search};

pub(crate) const MAX_CALLS: usize = 6;

pub(crate) fn definitions() -> Value {
    json!([search::definition(), paper::definition()])
}

impl ToolCall {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Search(args) => search::validate(args),
            Self::GetPaper(args) => paper::validate(args),
        }
    }

    pub(crate) fn model_call(&self, id: &str) -> Value {
        let (name, args) = match self {
            Self::Search(args) => ("search", serde_json::to_string(args)),
            Self::GetPaper(args) => ("get_paper", serde_json::to_string(args)),
        };
        json!({"id":id, "type":"function", "function":{"name":name,"arguments":args.expect("tool arguments are JSON-safe")}})
    }

    pub(crate) fn matches(&self, result: &ToolResult) -> bool {
        matches!(
            (self, result),
            (Self::Search(_), ToolResult::Search(_)) | (Self::GetPaper(_), ToolResult::GetPaper(_))
        )
    }

    pub(crate) async fn execute(&self, app: &App) -> ToolResult {
        match self {
            Self::Search(args) => ToolResult::Search(search::execute(app, args).await),
            Self::GetPaper(args) => ToolResult::GetPaper(paper::execute(app, args).await),
        }
    }
}

impl ToolResult {
    pub(crate) fn model_content(&self) -> String {
        match self {
            Self::Search(result) => serde_json::to_string(result),
            Self::GetPaper(result) => serde_json::to_string(result),
        }
        .expect("tool results are JSON-safe")
    }
}
