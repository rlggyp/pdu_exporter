use axum::{http::StatusCode, response::IntoResponse};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::RAW_DATA_LENGTH;

pub async fn fetch_raw_data(params: HashMap<String, String>) -> Result<Vec<String>, axum::response::Response> {
    let target = match params.get("target") {
        Some(value) => value,
        None => return Err((StatusCode::BAD_REQUEST, "Missing `target` parameter").into_response()),
    };

    let endpoint = format!("{}:80", target);

    let mut stream = match tokio::net::TcpStream::connect(endpoint).await {
        Ok(s) => s,
        Err(_) => return Err((StatusCode::NOT_FOUND, "Failed to connect to target").into_response()),
    };

    let request = format!(
        "GET /status.cgi HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        target
    );

    if let Err(_) = stream.write_all(request.as_bytes()).await {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to write request").into_response());
    }

    let mut response: Vec<u8> = Vec::new();
    if let Err(_) = stream.read_to_end(&mut response).await {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to read response").into_response());
    }

    let response_text = String::from_utf8_lossy(&response);

    let pos = match response_text.find("\r\n\r\n") {
        Some(p) => p, 
        None => return Err((StatusCode::BAD_REQUEST, format!("No body found in response")).into_response()),
    };

    let body = &response_text[pos + 4..];
    let data: Vec<String> = body.split("?")
        .map(|s| s.to_string())
        .collect();
    
    if data.len() != RAW_DATA_LENGTH {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, format!("Not a valid PDU device!")).into_response());
    }
    
    Ok(data)
}
