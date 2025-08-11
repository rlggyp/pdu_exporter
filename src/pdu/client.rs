use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::RAW_DATA_LENGTH;

pub async fn fetch_raw_data(
    params: HashMap<String, String>,
    timeout: u64,
) -> Result<Box<[Box<str>]>, Response> {
    let target = match params.get("target") {
        Some(value) => value,
        None => return Err((StatusCode::BAD_REQUEST, "Missing `target` parameter").into_response()),
    };

    let endpoint = format!("{}:80", target);

    let response = tokio::time::timeout(std::time::Duration::from_secs(timeout), async {
        let mut stream = match tokio::net::TcpStream::connect(endpoint).await {
            Ok(s) => s,
            Err(e) => return Err((StatusCode::NOT_FOUND, format!("Failed to connect to target: {}", e)).into_response()),
        };

        let request = format!(
            "GET /status.cgi HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            target
        );

        if let Err(e) = stream.write_all(request.as_bytes()).await {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write request: {}", e)).into_response());
        }

        let mut response: Vec<u8> = Vec::new();
        if let Err(e) = stream.read_to_end(&mut response).await {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read response: {}", e)).into_response());
        }

        Ok(response)
    })
    .await
    .map_err(|_| (StatusCode::REQUEST_TIMEOUT, "Operation timed out").into_response())?;

    let response = response.map_err(|e| e)?;
    let response_text = String::from_utf8_lossy(&response);

    let pos = match response_text.find("\r\n\r\n") {
        Some(p) => p, 
        None => return Err((StatusCode::BAD_REQUEST, format!("No body found in response")).into_response()),
    };

    let body = &response_text[pos + 4..];
    let data: Box<[Box<str>]> = body.split("?")
        .map(|s| s.trim().into())
        .collect::<Vec<_>>()
        .into_boxed_slice();

    if data.len() != RAW_DATA_LENGTH {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, format!("Not a valid PDU device!")).into_response());
    }

    Ok(data)
}
