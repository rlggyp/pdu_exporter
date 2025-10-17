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
        log::debug!("Verifying Basic Auth header: {:?}", auth_header);

        if auth_header.starts_with("Basic") && auth_header.len() > 6 {
            let auth = match general_purpose::STANDARD.decode(&auth_header[6..]) {
                Ok(bytes) =>  match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(_) => {
                        log::debug!("Failed to decode base64 to UTF-8");
                        return false;
                    },
                },
                Err(_) => {
                    log::debug!("Failed to decode base64 from auth header");
                    return false;
                },
            };

            let user_pass: Vec<&str> = auth.split(":").collect();
            if user_pass.len() != 2 {
                log::debug!("Auth header does not contain user:pass");
                return false;
            }
            let (user, pass) = (user_pass[0], user_pass[1]);
            log::debug!("Parsed user: {}, pass: [REDACTED]", user);

            if let Some(hash) = self.credentials.get(user) {
                let verified = bcrypt::verify(pass, &hash).unwrap_or(false);
                log::debug!("Password verification for user {}: {}", user, verified);
                return verified;
            } else {
                log::debug!("User {} not found in credentials", user);
            }
        } else {
            log::debug!("Auth header does not start with 'Basic' or is too short");
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
        log::debug!("No credentials configured, skipping auth");
        return Ok(next.run(request).await);
    }

    let auth_header = request.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    match auth_header {
        Some(auth_header) => {
            log::debug!("Authorization header found");
            if state.basic_auth.verify(auth_header) {
                log::debug!("Authorization successful");
                Ok(next.run(request).await)
            } else {
                log::debug!("Authorization failed");
                Err(StatusCode::UNAUTHORIZED)
            }
        },
        None => {
            log::debug!("No Authorization header found");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
