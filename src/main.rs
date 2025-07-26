use std::alloc::System;

#[global_allocator]
static A: System = System;

use axum::{
    extract::Query, http::StatusCode, response::IntoResponse, routing::get, Json, Router
};
use base64::Engine;
use prometheus::{Encoder, GaugeVec, Opts, TextEncoder, core::Collector};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PDU_RAW_DATA_LENGTH: usize = 2016;
const METRIC_STEP: usize = 63;
const TEMP_INDEX_OFFSET: usize = 15;

struct PduMetrics {
    current: GaugeVec,
    voltage: GaugeVec,
    power: GaugeVec,
    power_factor: GaugeVec,
    energy: GaugeVec,
    temperature: GaugeVec,
    humidity: GaugeVec,
    sensor_exists: GaugeVec,
}

impl<'a> IntoIterator for &'a PduMetrics {
    type Item = (&'a str, &'a GaugeVec);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        vec![
            ("current", &self.current),
            ("voltage", &self.voltage),
            ("power", &self.power),
            ("power_factor", &self.power_factor),
            ("energy", &self.energy),
            ("temperature", &self.temperature),
            ("humidity", &self.humidity),
            ("sensor_exists", &self.sensor_exists),
        ].into_iter()
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/pdu", get(get_pdu_metrics_handler))
        .route("/api/v1/rack_names", get(get_rack_names_handler));

    let bind_address = "0.0.0.0:9117";
    println!("Server running on http://{}", bind_address);

    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_pdu_metrics_handler(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let data = match get_pdu_data(params).await {
        Ok(response) => response,
        Err(e) => return e,
    };

    let metric_families = process_pdu_metrics(data);

    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    (StatusCode::OK, String::from_utf8(buffer).unwrap()).into_response()
}

async fn get_rack_names_handler(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let data = match get_pdu_data(params).await {
        Ok(response) => response,
        Err(e) => return e,
    };

    let mut rack_names: HashMap<String, String> = HashMap::new();

    let mut address = 1;
    for i in (0..PDU_RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        rack_names.insert(format!("rack_{}", address), format!("# {} {}", address, data[i+1]));
        address += 1;
    }

    (StatusCode::OK, Json(HashMap::from([("rack_names", rack_names)]))).into_response()
}

async fn get_pdu_data(params: HashMap<String, String>) -> Result<Vec<String>, axum::response::Response> {
    let target = match params.get("target") {
        Some(value) => value,
        None => return Err((StatusCode::BAD_REQUEST, "Missing `target` parameter").into_response()),
    };

    let authorization = match params.get("authorization") {
        Some(value) => base64::engine::general_purpose::STANDARD_NO_PAD.encode(value),
        None => return Err((StatusCode::BAD_REQUEST, "Missing `authorization` parameter").into_response()),
    };

    let endpoint = format!("{}:80", target);

    let mut stream = match tokio::net::TcpStream::connect(endpoint).await {
        Ok(stream) => stream,
        Err(_) => return Err((StatusCode::NOT_FOUND, "Failed to connect to target").into_response()),
    };

    let request = format!(
        "GET /status.cgi HTTP/1.1\r\n\
        Host: {}\r\n\
        Authorization: Basic {}\r\n\
        Connection: close\r\n\
        \r\n",
        target, authorization
    );

    if let Err(e) = stream.write_all(request.as_bytes()).await {
        eprintln!("Write error `{}`: {}", target, e);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to write request").into_response());
    }

    let mut response: Vec<u8> = Vec::new();

    if let Err(e) = stream.read_to_end(&mut response).await {
        eprintln!("Read error `{}`: {}", target, e);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to read response").into_response());
    }

    let response_text = String::from_utf8_lossy(&response);

    if let Some(pos) = response_text.find("\r\n\r\n") {
        let body = &response_text[pos + 4..];
        let data: Vec<String> = body.split("?")
            .map(|s| s.to_string())
            .collect();

        if data.len() != PDU_RAW_DATA_LENGTH {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, format!("Not a valid PDU device!")).into_response());
        }

        Ok(data)
    } else {
        Err((StatusCode::BAD_REQUEST, format!("No body found in response")).into_response())
    }
}

fn build_gauge_vec(name: &str, help: &str, labels: &[&str]) -> GaugeVec {
    GaugeVec::new(Opts::new(name, help), labels).expect(&format!("failed to build gauge_vec {}", name))
}

fn parse_or_zero(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

fn build_pdu_metrics() -> PduMetrics {
    PduMetrics {
        current: build_gauge_vec("current", "Current in ampere", &["address"]),
        voltage: build_gauge_vec("voltage", "Voltage in volt", &["address"]),
        power: build_gauge_vec("power", "Power in watt", &["address"]),
        power_factor: build_gauge_vec("power_factor", "Power factor in ratio (0.0 - 1.0)", &["address"]),
        energy: build_gauge_vec("energy", "Energy in kWh", &["address"]),
        temperature: build_gauge_vec("temperature", "Temperature in celsius", &["address", "channel"]),
        humidity: build_gauge_vec("humidity", "Humidity in percent", &["address", "channel"]),
        sensor_exists: build_gauge_vec("sensor_exists", "Sensor exists (bool)", &["type"]),
    }
}

fn process_pdu_metrics(data: Vec<String>) -> Vec<prometheus::proto::MetricFamily> {
    let metrics = build_pdu_metrics();

    let mut addr = 1;
    for i in (0..PDU_RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        let address = format!("{}", addr);
        addr += 1;

        metrics.current.with_label_values(&[&address]).set(parse_or_zero(&data[i+10]));
        metrics.voltage.with_label_values(&[&address]).set(parse_or_zero(&data[i+11]));
        metrics.power.with_label_values(&[&address]).set(parse_or_zero(&data[i+12]));
        metrics.power_factor.with_label_values(&[&address]).set(parse_or_zero(&data[i+13]));
        metrics.energy.with_label_values(&[&address]).set(parse_or_zero(&data[i+14]));

        for j in 0..16 {
            let index = i + TEMP_INDEX_OFFSET + (j * 3);
            let channel = format!("{}", j+1);
            if !data[index].is_empty() {
                metrics.temperature.with_label_values(&[&address, &channel]).set(parse_or_zero(&data[index+1]));
                metrics.humidity.with_label_values(&[&address, &channel]).set(parse_or_zero(&data[index+2]));
            }
        }
    }

    let mut metric_families = Vec::new();

    for (name, metric) in &metrics {
        if name == "temperature" || name == "humidity" {
            if !metric.collect()[0].metric.is_empty() {
                metric_families.extend(metric.collect());
                metrics.sensor_exists.with_label_values(&[name]).set(1.0);
            } else {
                metrics.sensor_exists.with_label_values(&[name]).set(0.0);
            }
        } else {
            metric_families.extend(metric.collect());
        }
    }

    metric_families
}
