//! Loopback development entry point with local or `OpenRouter` model access.

use ol_rht_chat::Config;
use std::net::SocketAddr;
use std::process::ExitCode;

const USAGE: &str = "usage: ol-rht-chat [--provider local|openrouter] [--listen 127.0.0.1:50053] [--upstream http://127.0.0.1:8081/v1 (local only)] [--model MODEL] [--corpus http://127.0.0.1:50052]\nEnvironment: OL_RHT_CHAT_PROVIDER, OL_RHT_CHAT_MODEL, OPENROUTER_API_KEY, OL_RHT_DATABASE_URL. `just chat-api` loads the root .env file.";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ol-rht-chat: {error}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen = "127.0.0.1:50053".to_owned();
    let mut provider = std::env::var("OL_RHT_CHAT_PROVIDER").unwrap_or_else(|_| "local".to_owned());
    let mut upstream = None;
    let mut model = std::env::var("OL_RHT_CHAT_MODEL").ok();
    let mut corpus = "http://127.0.0.1:50052".to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing option value")?;
        match flag.as_str() {
            "--listen" => listen = value,
            "--provider" => provider = value,
            "--upstream" => upstream = Some(value),
            "--model" => model = Some(value),
            "--corpus" => corpus = value,
            _ => return Err(format!("unknown option {flag}").into()),
        }
    }
    let address: SocketAddr = listen.parse()?;
    if !address.ip().is_loopback() {
        return Err("the development listener must be on loopback".into());
    }
    let model = model.unwrap_or_else(|| match provider.as_str() {
        "openrouter" => ol_rht_chat::OPENROUTER_MODEL.to_owned(),
        _ => "local-gemma".to_owned(),
    });
    let config = match provider.as_str() {
        "local" => Config::local(
            upstream.as_deref().unwrap_or("http://127.0.0.1:8081/v1"),
            &model,
        )?,
        "openrouter" => {
            if upstream.is_some() {
                return Err("--upstream is only supported with --provider local".into());
            }
            let api_key = std::env::var("OPENROUTER_API_KEY").map_err(
                |_| "Set OPENROUTER_API_KEY in the root .env file before starting chat.",
            )?;
            Config::openrouter(&model, &api_key)?
        }
        _ => return Err("OL_RHT_CHAT_PROVIDER / --provider must be local or openrouter".into()),
    }
    .with_corpus(&corpus)?;
    let database_url = std::env::var("OL_RHT_DATABASE_URL")
        .map_err(|_| "OL_RHT_DATABASE_URL is required; use just chat-api for the local database")?;
    let store = workbench_store::Store::connect(&database_url).await?;
    // One development project, shared across restarts. Migrations remain explicit.
    let project_id = uuid::Uuid::from_u128(0x5d932c16_f172_4163_89b6_3cedc36d7a30);
    store.create_project(project_id, "Local chat").await?;
    let router = ol_rht_chat::router_with_storage(config, store, project_id)?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("ol-rht-chat: http://{address}/v1/chat → {provider}, model {model} (local dev)");
    axum::serve(listener, router).await?;
    Ok(())
}
