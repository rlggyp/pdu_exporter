use crate::AppState;

use std::sync::Arc;
use axum::{extract::{Request, State}, http::StatusCode, middleware::Next, response::Response};

#[derive(Clone)]
pub struct BasicAuth {
    pub base64_userpass: Vec<String>,
}

impl BasicAuth {
    fn verify(&self, auth_header: &str) -> bool {
        if auth_header.starts_with("Basic") && auth_header.len() > 6 {
            let auth = &auth_header[6..];
            self.base64_userpass.contains(&auth.to_string())
        } else {
            false
        }
    }
}

pub async fn basic_auth(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next
) -> Result<Response, StatusCode> {
    let auth_header = request.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    match auth_header {
        Some(auth_header) => {
            if state.basic_auth.verify(auth_header) {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        },
        None => Err(StatusCode::UNAUTHORIZED)
    }
}
