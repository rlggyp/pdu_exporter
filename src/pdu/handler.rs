use axum::{extract::Query, http::StatusCode, response::IntoResponse, Json};
use std::collections::HashMap;
use prometheus::{Encoder, TextEncoder};

use super::{METRIC_STEP, RAW_DATA_LENGTH, metrics::process_metrics, client::fetch_raw_data};

pub async fn pdu_metrics(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let data = match fetch_raw_data(params).await {
        Ok(d) => d,
        Err(e) => return e,
    };

    let metric_families = process_metrics(data);

    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    (StatusCode::OK, String::from_utf8(buffer).unwrap()).into_response()
}

pub async fn rack_names(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let data = match fetch_raw_data(params).await {
        Ok(d) => d,
        Err(e) => return e,
    };

    let mut rack_names: HashMap<String, String> = HashMap::new();

    let mut address = 1;
    for i in (0..RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        rack_names.insert(format!("rack_{}", address), format!("# {} {}", address, data[i+1]));
        address += 1;
    }

    (StatusCode::OK, Json(HashMap::from([("rack_names", rack_names)]))).into_response()
}
