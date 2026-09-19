//! `ol-rht-serve --store <root> [--listen <addr>] [--http <addr>|off]
//! [--allow-origin <origin>]… [--poll <seconds>]`: the query core over
//! gRPC and, on a second listener, over JSON (serving plan §7). Opens
//! the epoch `current` names, serves it, folds every batch the journal
//! gains (§5) on each poll, and switches to a new epoch when `current`
//! changes, requests in flight finishing on the old.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use ingest::Store;
use ol_rht_serve::proto::v1::serving_server::ServingServer;
use ol_rht_serve::{Origins, Service, Terms};
use serve::Serving;
use tonic::transport::Server;

const USAGE: &str = "usage: ol-rht-serve [--store <root>] [--listen <addr>] [--http <addr>|off] [--allow-origin <origin>]... [--poll <seconds>]";

struct Args {
    store: PathBuf,
    listen: String,
    http: String,
    origins: Vec<String>,
    poll: u64,
}

fn parse(args: &[String]) -> Option<Args> {
    let mut out = Args {
        store: PathBuf::from("data"),
        listen: "127.0.0.1:50051".to_owned(),
        // The JSON surface on the next port along. A browser speaks
        // HTTP/1.1 and gRPC wants HTTP/2 with prior knowledge, so a
        // listener each keeps both protocols at their own defaults and
        // lets either be bound alone; `off` leaves the port unbound.
        // Loopback, since nothing here authenticates anyone yet.
        http: "127.0.0.1:50052".to_owned(),
        origins: Vec::new(),
        poll: 1,
    };
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1)?;
        match args[i].as_str() {
            "--store" => out.store = PathBuf::from(value),
            "--listen" => out.listen.clone_from(value),
            "--http" => out.http.clone_from(value),
            "--allow-origin" => out.origins.push(value.clone()),
            "--poll" => out.poll = value.parse().ok()?,
            _ => return None,
        }
        i += 2;
    }
    // Naming one origin replaces the dev server's, rather than adding to
    // it: what a deployment allows is the deployment's to say in full.
    if out.origins.is_empty() {
        out.origins = Origins::DEV.iter().map(|o| (*o).to_owned()).collect();
    }
    Some(out)
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(args) = parse(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let origins = match Origins::new(&args.origins) {
        Ok(origins) => origins,
        Err(e) => {
            eprintln!("ol-rht-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let store = Store::new(args.store.clone());
    let serving = match Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("ol-rht-serve: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut epoch = serving.epoch().epoch;
    eprintln!(
        "ol-rht-serve: epoch {epoch}, batch {}, folded to {}, listening on {}",
        serving.epoch().batch,
        serving.watermark().batch,
        args.listen
    );
    let addr = match args.listen.parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("ol-rht-serve: {}: {e}", args.listen);
            return ExitCode::from(2);
        }
    };
    // Bound before anything else starts, so a port already taken stops
    // the server rather than leaving half of it up.
    let json = if args.http == "off" {
        None
    } else {
        match tokio::net::TcpListener::bind(&args.http).await {
            Ok(listener) => {
                eprintln!(
                    "ol-rht-serve: JSON on {}, readable from {}",
                    args.http,
                    origins.listed().join(", ")
                );
                Some(listener)
            }
            Err(e) => {
                eprintln!("ol-rht-serve: {}: {e}", args.http);
                return ExitCode::from(2);
            }
        }
    };
    let service = Arc::new(Service::new(serving, Terms::stub()));
    let json = json.map(|listener| {
        let service = service.clone();
        tokio::spawn(async move {
            if let Err(e) = ol_rht_serve::http::serve(listener, service, origins).await {
                eprintln!("ol-rht-serve: JSON: {e}");
            }
        })
    });
    let poller = {
        let service = service.clone();
        let root = args.store.clone();
        let every = Duration::from_secs(args.poll.max(1));
        tokio::spawn(async move {
            let mut stalled: Option<String> = None;
            loop {
                tokio::time::sleep(every).await;
                let store = Store::new(root.clone());
                // The deltas first: what the journal gained since the
                // last poll, folded on a blocking thread.
                let current = service.current();
                let folded = tokio::task::spawn_blocking(move || {
                    let before = current.watermark().batch;
                    current
                        .refresh()
                        .map(|a| (before, current.watermark().batch, a))
                })
                .await;
                match folded {
                    Ok(Ok((before, after, advanced))) => {
                        if after > before {
                            eprintln!(
                                "ol-rht-serve: folded to batch {after} at {}",
                                ingest::time::now()
                            );
                        }
                        let now = advanced.stopped.map(|s| s.to_string());
                        if now != stalled {
                            if let Some(why) = &now {
                                eprintln!("ol-rht-serve: {why}");
                            }
                            stalled = now;
                        }
                    }
                    Ok(Err(e)) => eprintln!("ol-rht-serve: fold: {e}"),
                    Err(e) => eprintln!("ol-rht-serve: {e}"),
                }
                match serve::epoch::current(&store) {
                    Ok(Some(n)) if n != epoch => {
                        let opened =
                            tokio::task::spawn_blocking(move || Serving::open(&store)).await;
                        match opened {
                            Ok(Ok(new)) => {
                                service.swap(new);
                                epoch = n;
                                eprintln!("ol-rht-serve: switched to epoch {n}");
                            }
                            Ok(Err(e)) => eprintln!("ol-rht-serve: epoch {n} not opened: {e}"),
                            Err(e) => eprintln!("ol-rht-serve: {e}"),
                        }
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("ol-rht-serve: {e}"),
                }
            }
        })
    };
    let reflection_v1 = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(ol_rht_serve::proto::DESCRIPTORS)
        .build_v1();
    let reflection_v1alpha = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(ol_rht_serve::proto::DESCRIPTORS)
        .build_v1alpha();
    let (reflection_v1, reflection_v1alpha) = match (reflection_v1, reflection_v1alpha) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("ol-rht-serve: reflection: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (reporter, health) = tonic_health::server::health_reporter();
    reporter
        .set_serving::<ServingServer<Service<Serving>>>()
        .await;
    let served = Server::builder()
        .add_service(health)
        .add_service(reflection_v1)
        .add_service(reflection_v1alpha)
        .add_service(ServingServer::from_arc(service))
        .serve_with_shutdown(addr, async {
            let _ = tokio::signal::ctrl_c().await;
            eprintln!("ol-rht-serve: stopping");
        })
        .await;
    poller.abort();
    if let Some(json) = json {
        json.abort();
    }
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ol-rht-serve: {e}");
            ExitCode::FAILURE
        }
    }
}
