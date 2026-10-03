//! Retrieve one work through the corpus resolver. Citation prose belongs to
//! the model; link targets derive only from promoted identifiers in the work.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::protocol::{
    CorpusProvenance, Paper, PaperArgs, PaperDate, PaperIdentifier, PaperResult,
};

pub(crate) fn definition() -> Value {
    json!({"type":"function", "function":{
        "name":"get_paper",
        "description":"Get one paper's citation metadata, available abstract and reading links from the local corpus. Prefer the ol: public_id returned by search; otherwise pass any supported identifier: doi:, pmid:, pmc:, openalex:, or reg<digits>:. Use namespace-qualified identifiers in their canonical spelling; convert a bare DOI or DOI URL to a lowercase doi: identifier before calling. Use this after search to prepare a citation. Format the citation yourself using only returned fields. Link the exact work title to full_text_url if present, otherwise publisher_url (access not verified). Never invent pages, volume, issue or a full-text URL. Missing metadata may indicate a citation-only stub.",
        "parameters":{"type":"object", "properties":{"id":{"type":"string","description":"Prefer ol:… when known, otherwise the supported identifier as supplied; at most 2048 UTF-8 bytes."}},"required":["id"],"additionalProperties":false}
    }})
}

pub(crate) fn validate(args: &PaperArgs) -> Result<(), &'static str> {
    if args.id.trim().is_empty() || args.id.len() > 2048 {
        return Err("Get paper needs a nonempty identifier of at most 2048 bytes.");
    }
    Ok(())
}

pub(crate) async fn execute(app: &App, args: &PaperArgs) -> PaperResult {
    match fetch(app, args).await {
        Ok(result) => result,
        Err(message) => PaperResult::Error {
            message: message.to_owned(),
        },
    }
}

async fn fetch(app: &App, args: &PaperArgs) -> Result<PaperResult, &'static str> {
    validate(args)?;
    let mut url = app.config.work.clone();
    url.query_pairs_mut().append_pair("work", args.id.trim());
    let response: CorpusWork = crate::corpus::get(app, url).await?;
    let work = response
        .work
        .ok_or("No paper was returned for that identifier. Try search or another identifier.")?;
    let full_text_url = work
        .names
        .iter()
        .find_map(|name| identifier_url(name, "pmc"));
    let publisher_url = work
        .names
        .iter()
        .find_map(|name| identifier_url(name, "doi"));
    let abstract_text: String = work.abstract_text.chars().take(8000).collect();
    let result = PaperResult::Ok {
        paper: Box::new(Paper {
            public_id: work.public_id,
            stub: work.stub,
            title: work.title,
            authors_truncated: work.authors.len() > 50,
            authors: work.authors.into_iter().take(50).collect(),
            venue: work.venue,
            published: work.published,
            identifiers: work.names,
            abstract_truncated: abstract_text.len() < work.abstract_text.len(),
            abstract_text,
            retracted_by: work.retracted_by,
            corrected_by: work.corrected_by,
            concern_by: work.concern_by,
            open_access: work.open_access,
            full_text_url,
            publisher_url,
        }),
        answered: response.answered,
    };
    crate::corpus::check_size(&result)?;
    Ok(result)
}

fn identifier_url(name: &PaperIdentifier, namespace: &str) -> Option<String> {
    if name.namespace != namespace || name.standing != "promoted" || name.value.is_empty() {
        return None;
    }
    let mut url = reqwest::Url::parse(match namespace {
        "pmc" => "https://pmc.ncbi.nlm.nih.gov/articles/",
        "doi" => "https://doi.org/",
        _ => return None,
    })
    .ok()?;
    {
        let mut path = url.path_segments_mut().ok()?;
        path.pop_if_empty();
        // Keep DOI slashes readable, but encode ?/#/% rather than giving an
        // identifier control over the URL's authority, query or fragment.
        for segment in name.value.split('/') {
            path.push(segment);
        }
        if namespace == "pmc" {
            path.push("");
        }
    }
    Some(url.into())
}

#[derive(Deserialize)]
struct CorpusWork {
    answered: Option<CorpusProvenance>,
    work: Option<Work>,
}

#[derive(Deserialize)]
struct Work {
    public_id: String,
    stub: bool,
    names: Vec<PaperIdentifier>,
    title: String,
    authors: Vec<String>,
    venue: String,
    published: Option<PaperDate>,
    #[serde(rename = "abstract")]
    abstract_text: String,
    retracted_by: Vec<String>,
    corrected_by: Vec<String>,
    concern_by: Vec<String>,
    open_access: Option<bool>,
}
