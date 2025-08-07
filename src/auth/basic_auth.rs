use crate::AppState;

use std::sync::Arc;
use std::collections::HashMap;
use axum::{extract::{Request, State}, http::StatusCode, middleware::Next, response::Response};
use base64::{engine::general_purpose::STANDARD, Engine};

#[derive(Clone)]
pub struct BasicAuth {
    pub credentials: HashMap<String, String>,
}

impl BasicAuth {
    fn verify(&self, auth_header: &str) -> bool {
        if auth_header.starts_with("Basic") && auth_header.len() > 6 {
            let auth = match STANDARD.decode(&auth_header[6..]) {
                Ok(bytes) =>  match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(_) => return false,
                },
                Err(_) => return false,
            };

            let auth_split: Vec<&str> = auth.split(":").collect();
            let user = auth_split[0];
            let pass = auth_split[1];

            if let Some(hash) = self.credentials.get(user) {
                if let Ok(result) = bcrypt::verify(pass, &hash) {
                    return result
                }
            }

            false
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
