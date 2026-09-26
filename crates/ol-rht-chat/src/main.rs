//! Loopback-only development entry point. No provider credentials or downloads.

use ol_rht_chat::Config;
use std::net::SocketAddr;
use std::process::ExitCode;

const USAGE: &str = "usage: ol-rht-chat [--listen 127.0.0.1:50053] [--upstream http://127.0.0.1:8081/v1] [--model local-gemma] [--corpus http://127.0.0.1:50052]";

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
    let mut upstream = "http://127.0.0.1:8081/v1".to_owned();
    let mut model = "local-gemma".to_owned();
    let mut corpus = "http://127.0.0.1:50052".to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing option value")?;
        match flag.as_str() {
            "--listen" => listen = value,
            "--upstream" => upstream = value,
            "--model" => model = value,
            "--corpus" => corpus = value,
            _ => return Err(format!("unknown option {flag}").into()),
        }
    }
    let address: SocketAddr = listen.parse()?;
    if !address.ip().is_loopback() {
        return Err("the development listener must be on loopback".into());
    }
    let database_url = std::env::var("OL_RHT_DATABASE_URL")
        .map_err(|_| "OL_RHT_DATABASE_URL is required; use just chat-api for the local database")?;
    let store = workbench_store::Store::connect(&database_url).await?;
    // One development project, shared across restarts. Migrations remain explicit.
    let project_id = uuid::Uuid::from_u128(0x5d932c16_f172_4163_89b6_3cedc36d7a30);
    store.create_project(project_id, "Local chat").await?;
    let router = ol_rht_chat::router_with_storage(
        Config::local(&upstream, &model)?.with_corpus(&corpus)?,
        store,
        project_id,
    )?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("ol-rht-chat: http://{address}/v1/chat → {upstream}, model {model} (local dev)");
    axum::serve(listener, router).await?;
    Ok(())
}
