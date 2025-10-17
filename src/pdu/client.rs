use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::RAW_DATA_LENGTH;

pub async fn fetch_raw_data(
    params: HashMap<String, String>,
    timeout: u64,
) -> Result<Box<[Box<str>]>, Response> {
    log::debug!("fetch_raw_data called with params: {:?}, timeout: {}", params, timeout);

    let target = match params.get("target") {
        Some(value) => value,
        None => {
            log::debug!("Missing `target` parameter");
            return Err((StatusCode::BAD_REQUEST, "Missing `target` parameter").into_response());
        },
    };

    let endpoint = format!("{}:80", target);
    log::debug!("Connecting to endpoint: {}", endpoint);

    let response = tokio::time::timeout(std::time::Duration::from_secs(timeout), async {
        let mut stream = match tokio::net::TcpStream::connect(endpoint).await {
            Ok(s) => {
                log::debug!("Successfully connected to target: {}", target);
                s
            },
            Err(e) => {
                log::debug!("Failed to connect to target: {}. Error: {}", target, e);
                return Err((StatusCode::NOT_FOUND, format!("Failed to connect to target: {}", e)).into_response());
            },
        };

        let request = format!(
            "GET /status.cgi HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            target
        );
        log::debug!("Sending request: {}", request);

        if let Err(e) = stream.write_all(request.as_bytes()).await {
            log::debug!("Failed to write request: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write request: {}", e)).into_response());
        }

        let mut response: Vec<u8> = Vec::new();
        if let Err(e) = stream.read_to_end(&mut response).await {
            log::debug!("Failed to read response: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read response: {}", e)).into_response());
        }

        log::debug!("Received response: {} bytes", response.len());
        Ok(response)
    })
    .await
    .map_err(|_| {
        log::debug!("Operation timed out");
        (StatusCode::REQUEST_TIMEOUT, "Operation timed out").into_response()
    })?;

    let response = response.map_err(|e| e)?;
    let response_text = String::from_utf8_lossy(&response);
    log::debug!("Response text: {}", response_text);

    let pos = match response_text.find("\r\n\r\n") {
        Some(p) => p, 
        None => {
            log::debug!("No body found in response");
            return Err((StatusCode::BAD_REQUEST, format!("No body found in response")).into_response());
        },
    };

    let body = &response_text[pos + 4..];
    log::debug!("Response body: {}", body);

    let data: Box<[Box<str>]> = body.split("?")
        .map(|s| s.trim().into())
        .collect::<Vec<_>>()
        .into_boxed_slice();

    log::debug!("Parsed data length: {}", data.len());

    if data.len() != RAW_DATA_LENGTH {
        log::debug!("Not a valid PDU device! Expected length: {}, got: {}", RAW_DATA_LENGTH, data.len());
        return Err((StatusCode::UNPROCESSABLE_ENTITY, format!("Not a valid PDU device!")).into_response());
    }

    Ok(data)
}
