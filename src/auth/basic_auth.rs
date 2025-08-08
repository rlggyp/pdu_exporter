use crate::AppState;

use std::sync::Arc;
use std::collections::HashMap;
use axum::{extract::{Request, State}, http::StatusCode, middleware::Next, response::Response};
use base64::{engine::general_purpose, Engine};

#[derive(Clone)]
pub struct BasicAuth {
    pub credentials: HashMap<String, String>,
}

impl BasicAuth {
    fn verify(&self, auth_header: &str) -> bool {
        if auth_header.starts_with("Basic") && auth_header.len() > 6 {
            let auth = match general_purpose::STANDARD.decode(&auth_header[6..]) {
                Ok(bytes) =>  match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(_) => return false,
                },
                Err(_) => return false,
            };

            let user_pass: Vec<&str> = auth.split(":").collect();
            let (user, pass) = (user_pass[0], user_pass[1]);

            if let Some(hash) = self.credentials.get(user) {
                return bcrypt::verify(pass, &hash).unwrap_or(false);
            }
        }

        false
    }
}

pub async fn basic_auth(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next
) -> Result<Response, StatusCode> {
    let state = state.config.read().await;

    if state.basic_auth.credentials.is_empty() {
        return Ok(next.run(request).await);
    }

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
