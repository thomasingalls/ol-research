//! The browser client's TypeScript, generated from the Rust on both
//! sides of a call (serving plan §7): the parameters each route
//! declares, and the body it answers with.
//!
//! `web/src/api.ts` held these shapes by hand until 2026-09-19, and a
//! hand-written contract drifts silently: a field added to the protocol
//! is a field the client keeps not reading, and one renamed is a
//! `undefined` the UI renders as a blank. The requests drifted the same
//! way and sooner — the client sent `page` where the route reads
//! `page_size`, a refusal it could not have been compiled against — so
//! both halves are generated here: the answers from the same structs
//! [`http`](crate::http) hands to `axum`'s `Json`, the requests from the
//! same structs it reads a query string into.
//!
//! Generated from the Rust and not from the protocol, because the bytes
//! are not proto3 JSON. `build.rs` derives `serde::Serialize` on the
//! prost types, so the encoding is serde's: `public_id`, not `publicId`,
//! and `"total": 74`, not `"total": "74"`. A generator reading the
//! `.proto` would be wrong on every field name and every `uint64`.
//!
//! `u64` becomes `number` and never `bigint`: `JSON.parse` turns a JSON
//! number into a JS `number`, so `bigint` would describe a runtime that
//! does not exist. Nothing here approaches the limit either — the
//! largest counts on the epoch of 2026-09-19 are 5,238,452 nodes and
//! 6,544,183 arcs, and a whole-corpus few billion is still eight orders
//! of magnitude short of `Number.MAX_SAFE_INTEGER`.
//!
//! The output is committed, and [`the_committed_bindings_are_current`]
//! fails when it is stale. `just ts-types` rewrites it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ts_rs::{Config, TS};

use crate::http;

/// The committed bindings, from this crate's directory.
const COMMITTED: &str = "../../web/src/generated/serving.ts";

/// What ts-rs is pointed at, and what the file is called there.
const FILE: &str = "serving.ts";

/// Above ts-rs's own note: why a generated file is checked in here, when
/// the router next door declines to check one in.
const HEADER: &str = "\
// The JSON API's contract, generated from the Rust that serves it: the
// parameters each route takes (crates/ol-rht-serve/src/http.rs) and the
// body it answers with (crates/ol-rht-serve/proto/olrht/v1/serving.proto,
// as prost renders it and serde writes it). Regenerate with
// `just ts-types`; `just check` fails when this file is not what the
// server generates.
//
// Checked in, unlike the route tree in src/router.ts, which is written by
// hand for the stated reason that three screens do not need a codegen step
// and a generated file in the repo. The trade-off differs here: this is an
// interface contract with another process, not a tree this app owns, so
// the frontend has to build from a fresh checkout with no Rust toolchain
// anywhere near it, and a protocol that moved under us should show up as a
// reviewable diff rather than as a type error in whoever next runs cargo.
//
// The refusal body `{ code, message }` is the one shape still written by
// hand: axum builds it and the protocol never mentions it, so there is no
// Rust type here to generate it from.

";

/// The generation, as it is committed: every count a `number`, into
/// `dir`.
fn config(dir: &Path) -> Config {
    Config::new().with_large_int("number").with_out_dir(dir)
}

/// The bindings as they should be on disk: both sides of every route,
/// and everything those reach.
///
/// The requests are here since 2026-09-19. They are query strings and
/// not protocol messages — two of their names differ from the
/// protocol's on purpose, `page_size` for `page` and `facet_values` for
/// `facets` — so they are generated from the structs the routes read a
/// query string into and not from the `.proto`.
///
/// Which is also why the protocol's own two enums stay out. `Walk` and
/// `By` are request-side, prost stores an enum as an `i32`, and serde
/// would write a bare integer that no TypeScript name could explain; the
/// request structs name their own enums instead, which are sets of words
/// and reach TypeScript as the unions of string literals they are.
/// [`no_enum_reaches_a_response_body`] is what holds the protocol's out.
fn bindings() -> String {
    let dir = scratch();
    let cfg = config(&dir);
    let table = table(&cfg);
    let generated = std::fs::read_to_string(dir.join(FILE)).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    format!("{HEADER}{}{}", laid_out(&generated), routes(&table))
}

/// What walks the router's own list of routes (`http::each_route`): one
/// call a route, with the two types its path decides — what may be sent
/// to it, and what comes back. A trait and not a closure, because the
/// types differ from one route to the next and a closure cannot be
/// generic.
pub(crate) trait EachRoute {
    fn route<P: TS + 'static, R: TS + 'static>(&mut self, path: &'static str);
}

/// Every route, as its path and the TypeScript names of its two types —
/// exporting the declarations on the way. There is no list of routes
/// here to fall behind the router: this is the router's.
fn table(cfg: &Config) -> Vec<(&'static str, String, String)> {
    struct Exported<'a>(&'a Config, Vec<(&'static str, String, String)>);

    impl EachRoute for Exported<'_> {
        /// One route's two types, written out and then named.
        /// `export_all` follows each root's dependencies — a request
        /// reaches its enums, a response reaches every message under it
        /// — and every type carries the same `export_to`, so all of them
        /// merge into the one file.
        fn route<P: TS + 'static, R: TS + 'static>(&mut self, path: &'static str) {
            let cfg = self.0;
            P::export_all(cfg).unwrap();
            R::export_all(cfg).unwrap();
            self.1.push((path, P::name(cfg), R::name(cfg)));
        }
    }

    let mut exported = Exported(cfg, Vec::new());
    http::each_route(&mut exported);
    exported.1
}

/// The map `api.ts` indexes its one `get` by, so that a path decides
/// both halves of a call and neither can be got wrong on its own: a
/// parameter the route does not take, or a missing one, is a type error
/// where the path is written, which is where the mistake is.
fn routes(table: &[(&'static str, String, String)]) -> String {
    let rows = table
        .iter()
        .map(|(path, params, response)| {
            format!("  \"{path}\": {{ params: {params}, response: {response} }},")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "
// Every route, as the path it is asked at, the parameters it takes and
// the body it answers with. `api.ts` indexes its one `get` by this: the
// path decides both halves, so a parameter this surface does not read
// does not compile. Written from the router itself — see
// crates/ol-rht-serve/src/http.rs.
export type Routes = {{
{rows}
}};
"
    )
}

/// ts-rs writes a whole type on one line and starts every doc comment in
/// the first column. That is valid TypeScript and an unreadable diff: a
/// field added to `Work` would rewrite a six-hundred-character line, and
/// the point of committing this file is that a protocol which moved
/// under us arrives as a diff someone can read. So the same text is laid
/// out here, one member to a line.
///
/// It leans on what this protocol generates — no object inside an
/// object, no comma inside a member's type, no doc comment inside a
/// union — and asserts each of those rather than quietly mangling the
/// day one of them stops holding.
fn laid_out(generated: &str) -> String {
    let mut out = String::new();
    for (n, chunk) in generated.trim_end().split("\n\n").enumerate() {
        if n > 0 {
            out.push('\n');
        }
        out.push_str(&declaration(chunk));
    }
    out
}

/// One declaration — ts-rs's own note, or `export type X = …` under
/// whatever doc comment the `.proto` gave the message.
fn declaration(chunk: &str) -> String {
    let Some(start) = chunk.find("export type ") else {
        // ts-rs's note about not editing the file by hand.
        return format!("{}\n", chunk.trim_end());
    };
    let (docs, decl) = chunk.split_at(start);
    let decl = decl.trim_end();
    let split = decl.find(" = ").expect("`export type X = …`");
    let head = &decl[..split];
    let body = decl[split + 3..].trim_end().trim_end_matches(';');

    // Not an object at all: a set of names, which a request-side enum
    // is, or the `Record<symbol, never>` ts-rs writes for the route that
    // takes no parameters — an index signature no string key matches,
    // so `{ verbose: true }` does not compile against it, which is the
    // whole point of declaring parameters a route does not take. Both
    // are one short line and stay on it.
    if !body.starts_with('{') {
        return format!("{docs}{head} = {body};\n");
    }

    // A oneof: the arms are short and carry no comments of their own, so
    // each goes on a line of its own, led by the `|` TypeScript reads
    // them by.
    if body.contains("} | {") {
        let arms = pieces(body);
        assert_eq!(arms.len(), 1, "{head}: a union arm with a doc comment");
        let arms = arms[0]
            .1
            .split(" | ")
            .map(|arm| format!("  | {}", arm.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        return format!("{docs}{head} =\n{arms};\n");
    }

    let mut lines = Vec::new();
    for (doc, code) in pieces(body) {
        for line in doc.lines() {
            lines.push(format!("  {}", line.trim_end()));
        }
        let code = code.trim().trim_start_matches('{').trim_end_matches('}');
        for member in code.split(',') {
            let member = member.trim();
            if member.is_empty() {
                continue;
            }
            assert!(
                !member.contains(['{', '}']),
                "{head}: {member} is an object inside an object"
            );
            lines.push(format!("  {member},"));
        }
    }
    format!("{docs}{head} = {{\n{}\n}};\n", lines.join("\n"))
}

/// A body as the doc comment before each run of members and the run
/// itself: `[("", "{ epoch: number,"), ("/** … */", "batch: number,"), …]`.
fn pieces(body: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = body;
    let mut doc = "";
    while let Some(at) = rest.find("/**") {
        let (code, tail) = rest.split_at(at);
        let end = tail.find("*/").expect("an unclosed doc comment") + 2;
        out.push((doc, code));
        doc = &tail[..end];
        rest = &tail[end..];
    }
    out.push((doc, rest));
    out
}

/// A directory of its own per call: ts-rs remembers, per process, which
/// files it has written to, and two calls sharing one directory would
/// merge into each other rather than start clean.
fn scratch() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ol-rht-ts-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Where the committed file is.
fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(COMMITTED)
}

/// Rewrites the committed bindings. Ignored, so that `cargo test` never
/// writes into the working tree by itself; `just ts-types` runs it.
#[test]
#[ignore = "writes web/src/generated/serving.ts; run it with `just ts-types`"]
fn write_the_bindings() {
    let path = committed();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bindings()).unwrap();
    println!("wrote {}", path.display());
}

/// The committed bindings are what this protocol generates today. This
/// is the whole of the guard: the file is read by a frontend that never
/// compiles any of this, so nothing else would notice it going stale.
#[test]
fn the_committed_bindings_are_current() {
    let path = committed();
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        on_disk,
        bindings(),
        "{} is not what the protocol generates; run `just ts-types`",
        path.display()
    );
}

/// No enum reaches a response body.
///
/// It matters because prost stores a proto enum as an `i32` and serde
/// writes that as a bare integer — `"walk": 2` — which is a number a
/// client can neither read nor write back, and which ts-rs would type
/// `number` and so describe correctly and uselessly. The protocol's two
/// enums are request-side today; this walks the descriptor set the
/// server registers for reflection, from what each call of the service
/// answers with through every message that reaches, and fails if one
/// ever is not. The calls are read from the descriptor set too, so a
/// call added to the protocol is walked the day it is added.
#[test]
fn no_enum_reaches_a_response_body() {
    use prost::Message as _;
    use prost_types::field_descriptor_proto::Type;

    let set = prost_types::FileDescriptorSet::decode(crate::proto::DESCRIPTORS).unwrap();
    let messages: Vec<_> = set
        .file
        .iter()
        .flat_map(|f| f.message_type.iter().map(move |m| (f.package(), m)))
        .collect();
    let find = |name: &str| {
        messages
            .iter()
            .find(|(package, m)| format!(".{package}.{}", m.name()) == name)
            .map(|(_, m)| *m)
    };

    let mut queue: Vec<String> = set
        .file
        .iter()
        .flat_map(|f| &f.service)
        .flat_map(|s| &s.method)
        .map(|m| m.output_type().to_owned())
        .collect();
    // The two lists answer with the same page.
    queue.sort();
    queue.dedup();
    assert!(!queue.is_empty(), "the descriptor set declares no call");
    let roots = queue.len();
    let mut seen: Vec<String> = Vec::new();

    while let Some(name) = queue.pop() {
        if seen.contains(&name) {
            continue;
        }
        let message = find(&name).unwrap_or_else(|| panic!("{name} is not in the descriptor set"));
        seen.push(name.clone());
        for field in &message.field {
            assert_ne!(
                field.r#type(),
                Type::Enum,
                "{name}.{} is the enum {}, which serde writes as a bare integer",
                field.name(),
                field.type_name()
            );
            if field.r#type() == Type::Message {
                queue.push(field.type_name().to_owned());
            }
        }
    }
    assert!(
        seen.len() > roots,
        "the walk reached only the roots it started from"
    );
}
