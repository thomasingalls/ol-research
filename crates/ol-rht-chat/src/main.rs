//! Loopback development and explicitly enabled public shared demo entry point.

use ol_rht_chat::{Config, Limits};
use std::net::SocketAddr;
use std::process::ExitCode;

const USAGE: &str = "usage: ol-rht-chat [--mode development|public-demo] [--provider local|openrouter] [--listen 127.0.0.1:50053] [--upstream http://127.0.0.1:8081/v1 (local only)] [--model MODEL] [--corpus http://127.0.0.1:50052]\nEnvironment: OL_RHT_CHAT_PROVIDER, OL_RHT_CHAT_MODEL, OPENROUTER_API_KEY, OL_RHT_DATABASE_URL. Public demo limits: OL_RHT_CHAT_MAX_CONCURRENT (2), OL_RHT_CHAT_REPLIES_PER_MINUTE (6), OL_RHT_CHAT_WRITES_PER_MINUTE (60). `just chat-api` loads the root .env file.";

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
    let mut mode = "development".to_owned();
    let mut provider = std::env::var("OL_RHT_CHAT_PROVIDER").unwrap_or_else(|_| "local".to_owned());
    let mut upstream = None;
    let mut model = std::env::var("OL_RHT_CHAT_MODEL").ok();
    let mut corpus = "http://127.0.0.1:50052".to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing option value")?;
        match flag.as_str() {
            "--mode" => mode = value,
            "--listen" => listen = value,
            "--provider" => provider = value,
            "--upstream" => upstream = Some(value),
            "--model" => model = Some(value),
            "--corpus" => corpus = value,
            _ => return Err(format!("unknown option {flag}").into()),
        }
    }
    let address: SocketAddr = listen.parse()?;
    match mode.as_str() {
        "development" if !address.ip().is_loopback() => {
            return Err("the development listener must be on loopback; use --mode public-demo to expose shared chat".into());
        }
        "development" | "public-demo" => {}
        _ => return Err("--mode must be development or public-demo".into()),
    }
    let model = model.unwrap_or_else(|| match provider.as_str() {
        "openrouter" => ol_rht_chat::OPENROUTER_MODEL.to_owned(),
        _ => "local-gemma".to_owned(),
    });
    let mut config = match provider.as_str() {
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
    };
    if let Ok(provider) = std::env::var("OL_RHT_OPENROUTER_PROVIDER") {
        config = config.with_openrouter_provider(&provider)?;
    }
    config = if mode == "public-demo" {
        config.with_service_corpus(&corpus)?.with_limits(Limits {
            concurrent_replies: limit("OL_RHT_CHAT_MAX_CONCURRENT", 2)?,
            replies_per_minute: limit("OL_RHT_CHAT_REPLIES_PER_MINUTE", 6)?,
            history_writes_per_minute: limit("OL_RHT_CHAT_WRITES_PER_MINUTE", 60)?,
        })?
    } else {
        config.with_corpus(&corpus)?
    };
    let database_url = std::env::var("OL_RHT_DATABASE_URL")
        .map_err(|_| "OL_RHT_DATABASE_URL is required; use just chat-api for the local database")?;
    let store = workbench_store::Store::connect(&database_url).await?;
    store.ready().await?;
    // Stable, separate shared projects for the public demo and local development.
    // Neither mode provides user isolation. Migrations remain explicit.
    let (project_id, title) = if mode == "public-demo" {
        (
            uuid::Uuid::from_u128(0x4a68c8c7_37de_4b19_ad18_595a9f948ed2),
            "Public demo",
        )
    } else {
        (
            uuid::Uuid::from_u128(0x5d932c16_f172_4163_89b6_3cedc36d7a30),
            "Local chat",
        )
    };
    store.create_project(project_id, title).await?;
    let router = ol_rht_chat::router_with_storage(config, store, project_id)?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("ol-rht-chat: http://{address}/v1/chat → {provider}, model {model} ({mode})");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

fn limit(name: &str, default: usize) -> Result<usize, Box<dyn std::error::Error>> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("{name} must be a positive integer").into()),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

async fn shutdown() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
    eprintln!("ol-rht-chat: draining active requests");
}
