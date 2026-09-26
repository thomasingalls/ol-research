//! Search tool: read metadata from the configured corpus service.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::protocol::{CorpusProvenance, SearchArgs, SearchPaper, SearchResult};

pub(crate) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "search",
            "description": "Search the local literature corpus by keywords. Returns ranked paper metadata, not abstracts or full text. Use short topical keywords. Empty results mean no matches in this corpus, not that no literature exists.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Nonempty search keywords, at most 512 UTF-8 bytes."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10, "description": "Number of papers, default 5."}
                },
                "required": ["query"],
                "additionalProperties": false
            }
        }
    })
}

pub(crate) fn validate(args: &SearchArgs) -> Result<(), &'static str> {
    if args.query.trim().is_empty() || args.query.len() > 512 || !(1..=10).contains(&args.limit) {
        return Err("Search needs a nonempty query of at most 512 bytes and a limit from 1 to 10.");
    }
    Ok(())
}

pub(crate) async fn execute(app: &App, args: &SearchArgs) -> SearchResult {
    match fetch(app, args).await {
        Ok(result) => result,
        Err(message) => SearchResult::Error {
            message: message.to_owned(),
        },
    }
}

async fn fetch(app: &App, args: &SearchArgs) -> Result<SearchResult, &'static str> {
    validate(args)?;
    let mut url = app.config.corpus.clone();
    url.query_pairs_mut().extend_pairs([
        ("query", args.query.as_str()),
        ("limit", &args.limit.to_string()),
        ("hydrate", "true"),
        ("facet_values", "1"),
    ]);
    let response: CorpusResponse = crate::corpus::get(app, url).await?;
    let hits = response
        .hits
        .into_iter()
        .take(usize::from(args.limit))
        .map(|hit| {
            let summary = hit.summary.unwrap_or_default();
            SearchPaper {
                public_id: hit.public_id,
                title: hit.title,
                authors: summary.authors.into_iter().take(6).collect(),
                venue: summary.venue,
                year: summary.published.map(|date| date.year),
                retracted_by: summary.retracted_by,
                corrected_by: summary.corrected_by,
                concern_by: summary.concern_by,
            }
        })
        .collect();
    let result = SearchResult::Ok {
        query: args.query.clone(),
        total: response.total,
        hits,
        answered: response.answered,
    };
    crate::corpus::check_size(&result)?;
    Ok(result)
}

#[derive(Deserialize)]
struct CorpusResponse {
    answered: Option<CorpusProvenance>,
    total: u64,
    hits: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    public_id: String,
    title: String,
    summary: Option<Summary>,
}

#[derive(Default, Deserialize)]
struct Summary {
    authors: Vec<String>,
    venue: String,
    published: Option<PubDate>,
    retracted_by: Vec<String>,
    corrected_by: Vec<String>,
    concern_by: Vec<String>,
}

#[derive(Deserialize)]
struct PubDate {
    year: i32,
}
