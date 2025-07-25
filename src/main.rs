use axum::{
    extract::Query, http::StatusCode, response::IntoResponse, routing::get, Json, Router
};
use base64::Engine;
use prometheus::{Encoder, GaugeVec, Opts, TextEncoder, core::Collector};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PDU_RAW_DATA_LENGTH: usize = 2016;

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

    let sensor_exists_opts = Opts::new("sensor_exists", "Sensor exists (bool)");
    let sensor_exists_gauge_vec = GaugeVec::new(sensor_exists_opts, &["type"]).unwrap();

    let mut addr: u8 = 1;

    for i in (0..PDU_RAW_DATA_LENGTH).step_by(63) {
        let address = format!("{}", addr);
        addr += 1;

        current_gauge_vec.with_label_values(&[&address]).set(data[i+10].parse::<f64>().unwrap_or(0.0));
        voltage_gauge_vec.with_label_values(&[&address]).set(data[i+11].parse::<f64>().unwrap_or(0.0));
        power_gauge_vec.with_label_values(&[&address]).set(data[i+12].parse::<f64>().unwrap_or(0.0));
        power_factor_gauge_vec.with_label_values(&[&address]).set(data[i+13].parse::<f64>().unwrap_or(0.0));
        energy_gauge_vec.with_label_values(&[&address]).set(data[i+14].parse::<f64>().unwrap_or(0.0));

        for j in 0..16 {
            let index = i + 15 + (j * 3);
            let channel = format!("{}", j+1);
            if !data[index].is_empty() {
                temp_gauge_vec.with_label_values(&[&address, &channel]).set(data[index+1].parse::<f64>().unwrap_or(0.0));
                hum_gauge_vec.with_label_values(&[&address, &channel]).set(data[index+2].parse::<f64>().unwrap_or(0.0));
            }
        }
    }

    let mut metric_families = Vec::new();
    metric_families.extend(current_gauge_vec.collect());
    metric_families.extend(voltage_gauge_vec.collect());
    metric_families.extend(power_gauge_vec.collect());
    metric_families.extend(power_factor_gauge_vec.collect());
    metric_families.extend(energy_gauge_vec.collect());

    if !temp_gauge_vec.collect()[0].metric.is_empty() {
        metric_families.extend(temp_gauge_vec.collect());
        sensor_exists_gauge_vec.with_label_values(&["temperature"]).set(1.0);
    } else {
        sensor_exists_gauge_vec.with_label_values(&["temperature"]).set(0.0);
    }

    if !hum_gauge_vec.collect()[0].metric.is_empty() {
        metric_families.extend(hum_gauge_vec.collect());
        sensor_exists_gauge_vec.with_label_values(&["humidity"]).set(1.0);
    } else {
        sensor_exists_gauge_vec.with_label_values(&["humidity"]).set(0.0);
    }

    metric_families.extend(sensor_exists_gauge_vec.collect());

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
    for i in (0..PDU_RAW_DATA_LENGTH).step_by(63) {
        rack_names.insert(format!("rack_{}", address), format!("# {} {}", address, data[i+1]));
        address += 1;
    }

    (StatusCode::OK, Json(HashMap::from([("rack_names", rack_names)]))).into_response()
}

async fn get_pdu_data(
    params: HashMap<String, String>
) -> Result<Vec<String>, axum::response::Response> {
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
