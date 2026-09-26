//! Bounded reads from configured corpus endpoints. Never fetch a model URL.

use crate::App;
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;

pub(crate) async fn get<T: DeserializeOwned>(
    app: &App,
    url: reqwest::Url,
) -> Result<T, &'static str> {
    let mut response = app
        .client
        .get(url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| "Could not reach the corpus. Start `just serve` and try again.")?;
    let status = response.status();
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The corpus request was interrupted or timed out.")?
    {
        if body.len() + chunk.len() > 1024 * 1024 {
            return Err("The corpus response exceeded the size limit.");
        }
        body.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let failure: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
        return Err(match status.as_u16() {
            400 if failure
                .as_ref()
                .is_some_and(|v| v["code"] == "failed_precondition") =>
            {
                "The corpus cannot resolve that request to one paper or is not ready. A shared identifier needs a specific ol: ID; try search."
            }
            400 => "The corpus rejected the query or identifier. Check its syntax.",
            404 => {
                "No single paper answers for that identifier. It may be absent or retired; try search or another identifier."
            }
            _ => "The corpus request failed. Try again later.",
        });
    }
    serde_json::from_slice(&body).map_err(|_| "The corpus returned an invalid response.")
}

pub(crate) fn check_size(value: &impl Serialize) -> Result<(), &'static str> {
    if serde_json::to_vec(value)
        .map_err(|_| "Could not encode corpus metadata.")?
        .len()
        > 24 * 1024
    {
        return Err(
            "Corpus metadata exceeded the context budget. Try fewer results or another paper.",
        );
    }
    Ok(())
}
