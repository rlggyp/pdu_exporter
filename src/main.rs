mod auth;
mod pdu;

use auth::basic_auth::{basic_auth, BasicAuth};
use axum::{routing::get, Router};
use std::sync::Arc;
use base64::{engine::general_purpose, Engine};

#[derive(Clone)]
pub struct AppState {
    basic_auth: BasicAuth,
}

impl AppState {
    fn new() -> Arc::<Self> {
        Arc::new(AppState {
            basic_auth: auth::basic_auth::BasicAuth {
                base64_userpass: vec!["user:pass", "hello:world"]
                    .into_iter()
                    .map(|x| general_purpose::STANDARD.encode(x))
                    .collect::<Vec<String>>(),
            }
        })
    }
}

#[tokio::main]
async fn main() {
    let app_state = AppState::new();

    let app = Router::new()
        .route("/pdu", get(pdu::handler::pdu_metrics))
        .route("/api/v1/rack_names", get(pdu::handler::rack_names))
        .route_layer(axum::middleware::from_fn_with_state(app_state.clone(), basic_auth));

    let bind_address = "0.0.0.0:9117";
    println!("Server running on http://{}", bind_address);

    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
