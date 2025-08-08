mod auth;
mod config;
mod pdu;

use auth::basic_auth::{basic_auth, BasicAuth};
use axum::{routing::get, Router};
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};

#[derive(Clone)]
pub struct AppState {
    basic_auth: BasicAuth,
    scrape_timeout: u64,
}

#[tokio::main]
async fn main() {
    let config = config::parser::load_config().unwrap_or_else(|e| {
        eprintln!("Error loading config: {}", e);
        std::process::exit(1);
    });

    let app_state = Arc::new(AppState {
        basic_auth: BasicAuth { credentials: config.basic_auth_users.credentials },
        scrape_timeout: config.scrape_configs.scrape_timeout,
    });

    let app = Router::new()
        .route("/pdu", get(pdu::handler::pdu_metrics))
        .route("/api/v1/rack_names", get(pdu::handler::rack_names))
        .route_layer(axum::middleware::from_fn_with_state(app_state.clone(), basic_auth))
        .with_state(app_state.clone());

    let bind_address = "0.0.0.0:9117";
    println!("Server running on http://{}", bind_address);

    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to bind SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to bind SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {
            println!("SIGINT received, Gracefully shutting down.");
        }
        _ = sigterm.recv() => {
            println!("SIGTERM received, Gracefully shutting down.");
        }
    }
}
