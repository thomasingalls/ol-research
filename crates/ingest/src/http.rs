//! The HTTP client behind `list` and `fetch`: one agent per run,
//! identified by a User-Agent the caller chooses, rustls, and no
//! transparent decompression — raw bytes land exactly as served (plan
//! §3.1). Every response is checked for the status a transfer needs;
//! anything else is an error with the URL in it.

use std::error::Error as StdError;
use std::fmt;
use std::io::Read;
use std::time::Duration;

use ureq::http::{Response, StatusCode};
use ureq::{Agent, Body};

/// The most an index document may be.
const DOCUMENT_LIMIT: u64 = 256 * 1024 * 1024;

/// A client for one run.
#[derive(Debug, Clone)]
pub struct Client {
    agent: Agent,
}

/// A request that did not get the response a transfer needs.
#[derive(Debug)]
pub struct Error {
    #[allow(missing_docs)]
    pub url: String,
    #[allow(missing_docs)]
    pub kind: ErrorKind,
}

#[allow(missing_docs)]
#[derive(Debug)]
pub enum ErrorKind {
    /// A status other than 200 or 206.
    Status(u16),
    /// Could not connect, send, or read.
    Transport(ureq::Error),
    /// The body could not be read.
    Body(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.url)?;
        match &self.kind {
            ErrorKind::Status(code) => write!(f, "HTTP {code}"),
            ErrorKind::Transport(e) => write!(f, "{e}"),
            ErrorKind::Body(e) => write!(f, "reading the body: {e}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.kind {
            ErrorKind::Status(_) => None,
            ErrorKind::Transport(e) => Some(e),
            ErrorKind::Body(e) => Some(e),
        }
    }
}

impl Client {
    /// A client identifying itself as `user_agent`.
    #[must_use]
    pub fn new(user_agent: &str) -> Self {
        let config = Agent::config_builder()
            .user_agent(user_agent.to_owned())
            .http_status_as_error(false)
            .input_buffer_size(256 * 1024)
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .build();
        Self {
            agent: config.new_agent(),
        }
    }

    /// GET `url`, from byte `from` on when resuming. Ok for 200 and 206
    /// only; the body is unread.
    ///
    /// # Errors
    ///
    /// Any other status, or a transport failure.
    pub fn get(&self, url: &str, from: Option<u64>) -> Result<Response<Body>, Error> {
        let error = |kind| Error {
            url: url.to_owned(),
            kind,
        };
        let mut request = self.agent.get(url);
        if let Some(from) = from {
            request = request.header("Range", format!("bytes={from}-"));
        }
        let response = request.call().map_err(|e| error(ErrorKind::Transport(e)))?;
        match response.status() {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => Ok(response),
            status => Err(error(ErrorKind::Status(status.as_u16()))),
        }
    }

    /// GET `url` as text: an index document, a sidecar.
    ///
    /// # Errors
    ///
    /// As [`Client::get`], or a body over the document limit or not UTF-8.
    pub fn text(&self, url: &str) -> Result<String, Error> {
        let mut response = self.get(url, None)?;
        let mut text = String::new();
        response
            .body_mut()
            .with_config()
            .limit(DOCUMENT_LIMIT)
            .reader()
            .read_to_string(&mut text)
            .map_err(|e| Error {
                url: url.to_owned(),
                kind: ErrorKind::Body(e),
            })?;
        Ok(text)
    }
}

/// A header's value as text, when present and ASCII.
#[must_use]
pub fn header(response: &Response<Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}
