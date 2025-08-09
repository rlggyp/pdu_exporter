mod auth;
mod config;
mod pdu;

use auth::basic_auth::{basic_auth, BasicAuth};
use axum::{http::StatusCode, extract::State, response::IntoResponse, routing::{get, post}, Router};
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{RwLock, watch};

#[derive(Clone)]
pub struct AppConfig {
    basic_auth: BasicAuth,
    scrape_timeout: u64,
}

impl AppConfig {
    fn build() -> Result<Self, String> {
        match config::parser::load_config() {
            Ok(config) => {
                Ok(AppConfig {
                    basic_auth: BasicAuth { credentials: config.basic_auth_users.credentials },
                    scrape_timeout: config.scrape_configs.scrape_timeout,
                })
            },
            Err(e)  => Err(format!("Error loading config: {}.", e)),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<AppConfig>>,
    reload_tx: watch::Sender<bool>,
}

#[tokio::main]
async fn main() {
    let app_config = AppConfig::build().unwrap_or_else(|_| {
        std::process::exit(1);
    });

    let (reload_tx, reload_rx) = watch::channel(false);

    let app_state = Arc::new(
        AppState {
            config: Arc::new(RwLock::new(app_config)),
            reload_tx,
        }
    );

    spawn_reload_subscriber(app_state.clone(), reload_rx);
    spawn_sighup_handler(app_state.clone());

    let app = Router::new()
        .route("/-/reload", post(reload_config).put(reload_config))
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

fn spawn_reload_subscriber(app_state: Arc<AppState>, mut reload_rx: watch::Receiver<bool>) {
    tokio::spawn(async move {
        while reload_rx.changed().await.is_ok() {
            if *reload_rx.borrow() {
                println!("Reload triggered by subscriber...");
                match AppConfig::build() {
                    Ok(new_config) => {
                        *app_state.config.write().await = new_config;
                        println!("Reload complete.");
                    },
                    Err(error) => eprintln!("{error}"),
                }

                let _ = app_state.reload_tx.send(false);
            }
        }
    });
}

fn spawn_sighup_handler(app_state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut hup = signal(SignalKind::hangup()).expect("failed to bind SIGHUP");
        while hup.recv().await.is_some() {
            println!("Received SIGHUP, sending reload signal...");
            let _ = app_state.reload_tx.send(true);
        }
    });
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

async fn reload_config(State(app_state): State<Arc<AppState>>) -> axum::response::Response {
    if app_state.reload_tx.send(true).is_ok() {
        (StatusCode::OK).into_response()
    } else {
        eprintln!("Failed to send reload signal");
        (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    }
}
