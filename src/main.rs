use axum::{
    extract::{Query, State}, http::StatusCode, response::IntoResponse, routing::get, Router
};
use std::{collections::HashMap, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
struct AppState {
    authorization: String,
}

#[tokio::main]
async fn main() {
    let authorization = match std::env::var("AUTHORIZATION") {
        Ok(value) => value,
        Err(e) => panic!("{e}"),
    };

    let app = Router::new()
        .route("/pdu", get(pdu_handler))
        .with_state(Arc::new(AppState { authorization }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn pdu_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let target = match params.get("target") {
        Some(value) => value,
        None => return (StatusCode::BAD_REQUEST, "Missing `target` parameter").into_response(),
    };

    let endpoint = format!("{}:80", target);

    let mut stream = match tokio::net::TcpStream::connect(endpoint).await {
        Ok(stream) => stream,
        Err(_) => return (StatusCode::NOT_FOUND, "Failed to connect to target").into_response(),
    };

    let request = format!(
        "GET /status.cgi HTTP/1.1\r\n\
        Host: {}\r\n\
        Authorization: {}\r\n\
        Connection: close\r\n\
        \r\n",
        target, state.authorization
    );

    if let Err(e) = stream.write_all(request.as_bytes()).await {
        eprintln!("Write error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write request").into_response();
    }

    let mut response: Vec<u8> = Vec::new();

    if let Err(e) = stream.read_to_end(&mut response).await {
        eprintln!("Read error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read response").into_response();
    }

    let response_text = String::from_utf8_lossy(&response);

    if let Some(pos) = response_text.find("\r\n\r\n") {
        let body = &response_text[pos + 4..];

        let s: Vec<&str> = body.split("?").collect();

        if s.len() != 2016 {
            return (StatusCode::UNPROCESSABLE_ENTITY, format!("Not a valid PDU device!")).into_response();
        }

        let mut response = String::new();

        for i in (0..2016).step_by(63) {
            response.push_str(&format!("{} {}\n", s[i], s[i+1]));
            response.push_str(&format!("{} A\n", s[i+10]));
            response.push_str(&format!("{} V\n", s[i+11]));
            response.push_str(&format!("{} P(w)\n", s[i+12]));
            response.push_str(&format!("{} Pf\n", s[i+13]));
            response.push_str(&format!("{} Ep(kWh)\n", s[i+14]));
            for j in 0..16 {
                let index = i + 15 + (j * 3);
                if index + 2 < s.len() && !s[index].is_empty() {
                    response.push_str(&format!("temp: {}\n", s[index + 1]));
                    response.push_str(&format!("hum: {}\n", s[index + 2]));
                }
            }
            response.push_str("---\n\n");
        }
        (StatusCode::OK, format!("{}", response)).into_response()
    } else {
        (StatusCode::BAD_REQUEST, format!("No body found in response")).into_response()
    }
}
