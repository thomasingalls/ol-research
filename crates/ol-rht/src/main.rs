//! Command line for the pipeline.
//!
//! Only `id` exists so far: it runs one identifier parser over stdin, one
//! value per line, reports every failure with its line number, and exits
//! non-zero if anything failed. It is the seed of the "reproduce against one
//! record" command the failure report promises (plan §6), and the quickest
//! way to let a real file vote on a parser.

use std::io::{self, BufRead};
use std::process::ExitCode;

use model::{OpenAlexId, ParseError, Pmid};

const USAGE: &str = "usage: ol-rht id <pmid|pmid-url|openalex|openalex-url> < values.txt";

type Parser = fn(&str) -> Result<(), ParseError>;

fn parser(kind: &str) -> Option<Parser> {
    Some(match kind {
        "pmid" => |s| Pmid::from_digits(s).map(drop),
        "pmid-url" => |s| Pmid::from_url(s).map(drop),
        "openalex" => |s| OpenAlexId::from_bare(s).map(drop),
        "openalex-url" => |s| OpenAlexId::from_url(s).map(drop),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parse = match args.as_slice() {
        [cmd, kind] if cmd == "id" => parser(kind),
        _ => None,
    };
    let Some(parse) = parse else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let (mut ok, mut failed) = (0u64, 0u64);
    for (index, line) in io::stdin().lock().lines().enumerate() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("stdin: {err}");
                return ExitCode::from(2);
            }
        };
        match parse(&line) {
            Ok(()) => ok += 1,
            Err(err) => {
                failed += 1;
                eprintln!("line {}: {line:?}: {err}", index + 1);
            }
        }
    }
    eprintln!("ok={ok} failed={failed}");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
