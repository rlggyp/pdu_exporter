use crate::{Error, config};

use std::time::Duration;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Clone)]
pub struct Client {
    http_client: reqwest::Client,
}

impl Client {
    pub fn new(config: config::ScrapeConfigs) -> Result<Self, Error> {
        let http_client = reqwest::ClientBuilder::new()
            .http1_only()
            .http1_ignore_invalid_headers_in_responses(true)
            .tcp_keepalive(Some(Duration::from_secs(config.connect_timeout_seconds)))
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .pool_idle_timeout(std::time::Duration::from_secs(config.pool_idle_timeout_seconds))
            .connect_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .timeout(Duration::from_secs(config.scrape_timeout_seconds))
            .build()?;

        let client = Self { http_client };

        Ok(client)
    }

    pub async fn fetch_data(&self, target: &str) -> Result<Box<[Box<str>]>, Response> {
        let url = format!("http://{}/status.cgi", target);

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                log::debug!("Request to {} failed: {}", target, e);
                (StatusCode::SERVICE_UNAVAILABLE, "PDU Unreachable").into_response()
            })?;

        
        if response.status() != StatusCode::OK {
            log::debug!("Device at {} returned error: {}", target, response.status().as_str());
            return Err((StatusCode::BAD_GATEWAY, "Device returned error").into_response());
        }

        let body: String = response.text().await.map_err(|e| {
            log::debug!("Failed to read body from {}: {}", target, e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read body").into_response()
        })?;

        let data: Box<[Box<str>]> = body.split("?")
            .map(|s| s.trim().into())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        
        Ok(data)
    }
}
