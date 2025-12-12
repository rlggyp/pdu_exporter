mod auth;
mod config;
mod pdu;

use auth::basic_auth::{basic_auth, BasicAuth};
use config::Config;

use axum::{http::StatusCode, extract::State, response::IntoResponse, routing::{get, post}, Router};
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{RwLock, watch};

#[derive(Clone)]
pub struct AppConfig {
    basic_auth: BasicAuth,
    scrape_timeout_seconds: u64,
}

impl AppConfig {
    fn new(config: Config) -> Self {
        let credentials = config.basic_auth_users.clone();
        let scrape_timeout_seconds = config.scrape_configs.scrape_timeout_seconds;
        let auth_header_cache: Arc<RwLock<Vec::<String>>> = Arc::new(RwLock::new(Vec::new()));

        Self {
            basic_auth: BasicAuth { credentials, auth_header_cache },
            scrape_timeout_seconds,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<AppConfig>>,
    reload_tx: watch::Sender<bool>,
}

impl AppState {
    pub fn new(config: Config) -> Arc<Self> {
        let config = AppConfig::new(config);

        let (reload_tx, reload_rx) = watch::channel(false);
        let config = Arc::new(RwLock::new(config));

        let app_state = Arc::new(AppState {
            config,
            reload_tx,
        });

        AppState::spawn_reload_config_subscriber(app_state.clone(), reload_rx);
        AppState::spawn_sighup_handler(app_state.clone());

        app_state
    }

    fn spawn_reload_config_subscriber(app_state: Arc<AppState>, mut reload_rx: watch::Receiver<bool>) {
        tokio::spawn(async move {
            while reload_rx.changed().await.is_ok() {
                if *reload_rx.borrow() {
                    log::info!("Reload triggered by subscriber...");
                    match Config::get_config() {
                        Ok(config) => {
                            *app_state.config.write().await = AppConfig::new(config);
                            log::info!("Reload complete.");
                        },
                        Err(error) => log::error!("{}", error),
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
                log::info!("Received SIGHUP, sending reload signal...");
                let _ = app_state.reload_tx.send(true);
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::get_config()?;

    log4rs::init_file(&config.log_config_file, Default::default())
        .expect("Failed to init log4rs");

    let app_state = AppState::new(config);

    let app = Router::new()
        .route("/-/reload", post(reload_config).put(reload_config))
        .route("/pdu", get(pdu::handler::pdu_metrics))
        .route("/api/v1/rack_names", get(pdu::handler::rack_names))
        .route_layer(axum::middleware::from_fn_with_state(app_state.clone(), basic_auth))
        .with_state(app_state.clone());

    let bind_address = "0.0.0.0:9117";
    log::info!("Server running on http://0.0.0.0:9117");

    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    Ok(())
}

async fn shutdown_signal() {
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to bind SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to bind SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {
            log::info!("SIGINT received, Gracefully shutting down.");
        }
        _ = sigterm.recv() => {
            log::info!("SIGTERM received, Gracefully shutting down.");
        }
    }
}

async fn reload_config(State(app_state): State<Arc<AppState>>) -> axum::response::Response {
    if app_state.reload_tx.send(true).is_ok() {
        (StatusCode::NO_CONTENT).into_response()
    } else {
        log::error!("Failed to send reload signal");
        (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    }
}
