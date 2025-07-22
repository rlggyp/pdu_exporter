use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use prometheus::{Encoder, GaugeVec, Opts, TextEncoder, core::Collector};
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

        let current_opts = Opts::new("current", "Current in ampere");
        let current_gauge_vec = GaugeVec::new(current_opts, &["address"]).unwrap();

        let voltage_opts = Opts::new("voltage", "Voltage in volt");
        let voltage_gauge_vec = GaugeVec::new(voltage_opts, &["address"]).unwrap();

        let power_opts = Opts::new("power", "Power in watt");
        let power_gauge_vec = GaugeVec::new(power_opts, &["address"]).unwrap();

        let power_factor_opts = Opts::new("power_factor", "Power factor in ratio (0.0 - 1.0)");
        let power_factor_gauge_vec = GaugeVec::new(power_factor_opts, &["address"]).unwrap();

        let energy_opts = Opts::new("energy", "Energy in kWh");
        let energy_gauge_vec = GaugeVec::new(energy_opts, &["address"]).unwrap();

        let temp_opts = Opts::new("temperature", "Temperature in celsius");
        let temp_gauge_vec = GaugeVec::new(temp_opts, &["address", "channel"]).unwrap();

        let hum_opts = Opts::new("humidity", "Humidity in percent");
        let hum_gauge_vec = GaugeVec::new(hum_opts, &["address", "channel"]).unwrap();

        let mut addr: u8 = 1;

        for i in (0..2016).step_by(63) {
            let address = format!("{addr}");
            addr += 1;

            current_gauge_vec.with_label_values(&[&address]).set(s[i+10].parse::<f64>().unwrap_or(0.0));
            voltage_gauge_vec.with_label_values(&[&address]).set(s[i+11].parse::<f64>().unwrap_or(0.0));
            power_gauge_vec.with_label_values(&[&address]).set(s[i+12].parse::<f64>().unwrap_or(0.0));
            power_factor_gauge_vec.with_label_values(&[&address]).set(s[i+13].parse::<f64>().unwrap_or(0.0));
            energy_gauge_vec.with_label_values(&[&address]).set(s[i+14].parse::<f64>().unwrap_or(0.0));

            for j in 0..16 {
                let index = i + 15 + (j * 3);
                let channel = format!("{j}");
                if !s[index].is_empty() {
                    temp_gauge_vec.with_label_values(&[&address, &channel]).set(s[index+1].parse::<f64>().unwrap_or(0.0));
                    hum_gauge_vec.with_label_values(&[&address, &channel]).set(s[index+2].parse::<f64>().unwrap_or(0.0));
                }
            }
        }

        let mut metric_families = Vec::new();
        metric_families.extend(current_gauge_vec.collect());
        metric_families.extend(voltage_gauge_vec.collect());
        metric_families.extend(power_gauge_vec.collect());
        metric_families.extend(power_factor_gauge_vec.collect());
        metric_families.extend(energy_gauge_vec.collect());
        metric_families.extend(temp_gauge_vec.collect());
        metric_families.extend(hum_gauge_vec.collect());

        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        (StatusCode::OK, String::from_utf8(buffer).unwrap()).into_response()
    } else {
        (StatusCode::BAD_REQUEST, format!("No body found in response")).into_response()
    }
}
