//! `ol-rht-serve --store <root> [--listen <addr>] [--http <addr>|off]
//! [--allow-origin <origin>]… [--poll <seconds>]`: the query core over
//! gRPC and, on a second listener, over JSON (serving plan §7). Opens
//! the epoch `current` names, serves it, folds every batch the journal
//! gains (§5) on each poll, and switches to a new epoch when `current`
//! changes, requests in flight finishing on the old.
//!
//! A server built for another model or merge version than the store's
//! does not exit (2026-09-24). While `current` names an epoch built from
//! another merged layer it answers from the newest epoch built from its
//! own, and while there is none it waits: listening, refusing every call
//! `UNAVAILABLE`, its `olrht.v1.Serving` health `NOT_SERVING`, until an
//! epoch it can read is published. So a new build can be deployed before
//! the store is rebuilt for it: the one it replaces keeps answering, and
//! the new one is ready the moment the rebuild publishes.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use ingest::Store;
use ol_rht_serve::proto::v1::serving_server::ServingServer;
use ol_rht_serve::{Origins, Service, Terms};
use serve::Serving;
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;

/// The health service's name for the query service: what a readiness
/// probe asks about. The server as a whole, `""`, is serving while the
/// process is, which is what a liveness probe asks.
type Named = ServingServer<Service<Serving>>;

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
    let serving = match opening(&store) {
        Ok(serving) => Some(serving),
        Err(why) => {
            eprintln!("ol-rht-serve: waiting: {why}");
            None
        }
    };
    // The epoch `current` named when the server last looked, which it has
    // opened or tried to: a poll opens an epoch once, when `current`
    // moves to it, rather than every second while it cannot.
    let mut seen = serve::epoch::current(&store).ok().flatten();
    let addr: std::net::SocketAddr = match args.listen.parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("ol-rht-serve: {}: {e}", args.listen);
            return ExitCode::from(2);
        }
    };
    // Both ports bound before anything else starts, so a port already
    // taken stops the server rather than leaving half of it up — and
    // before the line below is printed, which until 2026-09-19 said the
    // server was listening on a port it had not tried yet, and then
    // failed underneath itself.
    let grpc = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("ol-rht-serve: {}: {e}", args.listen);
            return ExitCode::from(2);
        }
    };
    let json = if args.http == "off" {
        None
    } else {
        match tokio::net::TcpListener::bind(&args.http).await {
            Ok(listener) => Some(listener),
            Err(e) => {
                eprintln!("ol-rht-serve: {}: {e}", args.http);
                return ExitCode::from(2);
            }
        }
    };
    match &serving {
        Some(serving) => eprintln!(
            "ol-rht-serve: epoch {}, batch {}, folded to {}, listening on {}",
            serving.epoch().epoch,
            serving.epoch().batch,
            serving.watermark().batch,
            args.listen
        ),
        None => eprintln!(
            "ol-rht-serve: no epoch to answer from yet, listening on {} and not ready",
            args.listen
        ),
    }
    if json.is_some() {
        eprintln!(
            "ol-rht-serve: JSON on {}, readable from {}",
            args.http,
            origins.listed().join(", ")
        );
    }
    let service = Arc::new(match serving {
        Some(serving) => Service::new(serving, Terms::stub()),
        None => Service::waiting(Terms::stub()),
    });
    let (reporter, health) = tonic_health::server::health_reporter();
    if service.current().is_ok() {
        reporter.set_serving::<Named>().await;
    } else {
        reporter.set_not_serving::<Named>().await;
    }
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
        let reporter = reporter.clone();
        let root = args.store.clone();
        let every = Duration::from_secs(args.poll.max(1));
        tokio::spawn(async move {
            let mut stalled: Option<String> = None;
            loop {
                tokio::time::sleep(every).await;
                let store = Store::new(root.clone());
                // The deltas first: what the journal gained since the
                // last poll, folded on a blocking thread — when there is
                // an epoch to fold them onto.
                if let Ok(current) = service.current() {
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
                }
                match serve::epoch::current(&store) {
                    Ok(Some(n)) if Some(n) != seen => {
                        seen = Some(n);
                        let opened =
                            tokio::task::spawn_blocking(move || Serving::open_epoch(&store, n))
                                .await;
                        match opened {
                            Ok(Ok(new)) => {
                                service.swap(new);
                                reporter.set_serving::<Named>().await;
                                eprintln!("ol-rht-serve: switched to epoch {n}");
                            }
                            // What was answering goes on answering: an
                            // epoch built by another version is the next
                            // build's to serve, not this one's.
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
    let served = Server::builder()
        .add_service(health)
        .add_service(reflection_v1)
        .add_service(reflection_v1alpha)
        .add_service(ServingServer::from_arc(service))
        .serve_with_incoming_shutdown(TcpIncoming::from(grpc), async {
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

/// What the server answers from at start: the epoch `current` names, or,
/// when that one was built from another merged layer than this build's —
/// another model or merge version, a store not yet rebuilt for this
/// build, or rebuilt for a later one this build was rolled back from —
/// the newest epoch built from this build's own.
///
/// Only the version decides it. An epoch of this build's own version that
/// does not open is broken rather than foreign, and answering from an
/// older one in its place would hide that; so is the newest epoch of this
/// build's version when it does not open, and nothing older is tried.
///
/// # Errors
///
/// Why there is nothing to answer from, for the log; the server waits.
fn opening(store: &Store) -> Result<Serving, String> {
    let Some(n) = serve::epoch::current(store).map_err(|e| e.to_string())? else {
        return Err("no epoch is published".to_owned());
    };
    let refused = match Serving::open_epoch(store, n) {
        Ok(serving) => return Ok(serving),
        Err(e) => e,
    };
    let ours = merge::layout::version();
    let theirs = serve::epoch::read(store, n)
        .map_err(|e| format!("epoch {n}: {e}"))?
        .merged;
    if theirs == ours {
        return Err(format!("epoch {n}: {refused}"));
    }
    let Some(k) = (1..n)
        .rev()
        .find(|&k| serve::epoch::read(store, k).is_ok_and(|m| m.merged == ours))
    else {
        return Err(format!(
            "epoch {n} was built from merged {theirs}, and no epoch built from {ours}, \
             which this build reads, is published yet"
        ));
    };
    let serving =
        Serving::open_epoch(store, k).map_err(|e| format!("epoch {k}, built from {ours}: {e}"))?;
    eprintln!(
        "ol-rht-serve: answering from epoch {k}: epoch {n} was built from merged {theirs}, \
         and this build reads {ours}"
    );
    Ok(serving)
}
