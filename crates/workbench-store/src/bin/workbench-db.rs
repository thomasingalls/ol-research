//! Explicit database operations; never run as a side effect of serving chat.

use std::process::ExitCode;

use workbench_store::Store;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("workbench-db: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let command = match args.as_slice() {
        [command] if matches!(command.as_str(), "migrate" | "check") => command,
        _ => {
            return Err(
                "usage: workbench-db <migrate|check> (OL_RHT_DATABASE_URL required)".into(),
            );
        }
    };
    let url = std::env::var("OL_RHT_DATABASE_URL")
        .map_err(|_| "OL_RHT_DATABASE_URL must name the application database")?;
    let store = Store::connect(&url).await?;
    if command == "migrate" {
        store.migrate().await?;
        println!("Application migrations are current.");
    } else {
        store.ready().await?;
        println!("Application database and schema are ready.");
    }
    store.close().await;
    Ok(())
}
