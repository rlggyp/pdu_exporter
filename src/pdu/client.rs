use crate::Error;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Clone)]
pub struct Client {
    http_client: reqwest::Client,
}

impl Client {
    pub fn new() -> Result<Self, Error> {
        let http_client = reqwest::ClientBuilder::new()
            .http1_only()
            .http1_ignore_invalid_headers_in_responses(true)
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(2)
            .connect_timeout(std::time::Duration::from_secs(5))
            .tcp_keepalive(Some(std::time::Duration::from_secs(10)))
            .build()?;

        let client = Self { http_client };

        Ok(client)
    }

    pub async fn fetch_data(
        &self,
        target: &str,
        timeout: u64,
    ) -> Result<Box<[Box<str>]>, Response> {
        let url = format!("http://{}/status.cgi", target);

        let response = self.http_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(timeout))
            .send()
            .await
            .map_err(|e| {
                log::error!("Request failed: {}", e);
                (StatusCode::SERVICE_UNAVAILABLE, "PDU Unreachable").into_response()
            })?;

        
        if response.status() != StatusCode::OK {
            return Err((StatusCode::BAD_GATEWAY, "Device returned error").into_response());
        }

        let body: String = response.text().await.map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read body").into_response()
        })?;

        let data: Box<[Box<str>]> = body.split("?")
            .map(|s| s.trim().into())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        
        Ok(data)
    }
}