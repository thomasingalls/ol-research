//! Per-process admission limits. A generation owns its permit until its body is dropped.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Shared demo limits, per process (not per visitor). Scale replicas deliberately.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub concurrent_replies: usize,
    pub replies_per_minute: usize,
    pub history_writes_per_minute: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            concurrent_replies: 2,
            replies_per_minute: 6,
            history_writes_per_minute: 60,
        }
    }
}

impl Limits {
    pub(crate) fn validate(self) -> Result<Self, String> {
        if !(1..=1024).contains(&self.concurrent_replies)
            || !(1..=10_000).contains(&self.replies_per_minute)
            || !(1..=10_000).contains(&self.history_writes_per_minute)
        {
            return Err("Chat limits must be positive (at most 1024 concurrent replies and 10000 requests/minute).".into());
        }
        Ok(self)
    }
}

#[derive(Debug)]
pub(crate) struct Admission {
    replies: Arc<Semaphore>,
    starts: Window,
    writes: Window,
}

impl Admission {
    pub(crate) fn new(limits: Limits) -> Self {
        Self {
            replies: Arc::new(Semaphore::new(limits.concurrent_replies)),
            starts: Window::new(limits.replies_per_minute),
            writes: Window::new(limits.history_writes_per_minute),
        }
    }

    pub(crate) fn reply(&self) -> Result<OwnedSemaphorePermit, Rejected> {
        let permit = self
            .replies
            .clone()
            .try_acquire_owned()
            .map_err(|_| Rejected {
                retry_after: 5,
                message: "All shared demo reply slots are busy. Please try again shortly.",
            })?;
        self.starts
            .take(Instant::now())
            .map_err(|retry_after| Rejected {
                retry_after,
                message: "The shared demo reply limit was reached. Please try again later.",
            })?;
        Ok(permit)
    }

    pub(crate) fn write(&self) -> Result<(), Rejected> {
        self.writes.take(Instant::now()).map_err(|retry_after| Rejected {
            retry_after,
            message: "The shared demo save limit was reached. Keep this page open and retry saving shortly.",
        })
    }
}

#[derive(Debug)]
struct Window {
    maximum: usize,
    accepted: Mutex<VecDeque<Instant>>,
}

impl Window {
    fn new(maximum: usize) -> Self {
        Self {
            maximum,
            accepted: Mutex::new(VecDeque::new()),
        }
    }

    fn take(&self, now: Instant) -> Result<(), u64> {
        let mut accepted = self.accepted.lock().expect("admission lock poisoned");
        while accepted
            .front()
            .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(60))
        {
            accepted.pop_front();
        }
        if accepted.len() >= self.maximum {
            let remaining = Duration::from_secs(60).saturating_sub(now.duration_since(accepted[0]));
            return Err(remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0));
        }
        accepted.push_back(now);
        Ok(())
    }
}

pub(crate) struct Rejected {
    retry_after: u64,
    message: &'static str,
}

impl IntoResponse for Rejected {
    fn into_response(self) -> Response {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [
                (header::RETRY_AFTER, self.retry_after.to_string()),
                (header::CACHE_CONTROL, "no-store".into()),
            ],
            Json(json!({"message": self.message})),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sliding_window_expires_and_rejections_do_not_extend_it() {
        let window = Window::new(2);
        let now = Instant::now();
        assert!(window.take(now).is_ok());
        assert!(window.take(now + Duration::from_secs(10)).is_ok());
        assert_eq!(window.take(now + Duration::from_millis(20_001)), Err(40));
        assert!(window.take(now + Duration::from_secs(60)).is_ok());
        assert_eq!(window.take(now + Duration::from_secs(60)), Err(10));
        assert!(window.take(now + Duration::from_secs(70)).is_ok());
    }
}
