use axum::{extract::Query, http::{header, StatusCode}, response::IntoResponse, Json};
use std::collections::HashMap;

use super::{METRIC_STEP, RAW_DATA_LENGTH, metrics::process_metrics, client::fetch_raw_data};

pub async fn pdu_metrics(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    match fetch_raw_data(params).await {
        Ok(d) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/plain")], process_metrics(&d)).into_response(),
        Err(e) => e,
    }
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
