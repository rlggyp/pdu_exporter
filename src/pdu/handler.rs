use axum::{extract::{Query, State}, http::{header, StatusCode}, response::IntoResponse, Json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use crate::AppState;

use super::{METRIC_STEP, RAW_DATA_LENGTH, metrics::process_metrics};

#[derive(Debug, serde::Serialize, Clone)]
struct Pdu {
    address: String,
    name: String,
    voltage: f32,
    current: f32,
    power: f32,
}

fn strip_suffix_ab(text: &str) -> &str {
    text.strip_suffix('A')
        .or_else(|| text.strip_suffix('B'))
        .unwrap_or(text)
}

async fn fetch_pdu_data(
    state: &Arc<AppState>,
    params: &HashMap<String, String>
) -> axum::response::Result<Box<[Box<str>]>> {
    let target = params.get("target").ok_or_else(|| {
        log::debug!("Missing `target` parameter");
        (StatusCode::BAD_REQUEST, "Missing `target` parameter")
    })?;

    let state = state.config.read().await;
    Ok(state.client.fetch_data(target).await?)
}

pub async fn pdu_metrics(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>
) -> axum::response::Result<impl IntoResponse> {
    log::debug!("pdu_metrics called with params: {:?}", params);
    let data = fetch_pdu_data(&state, &params).await?;
    Ok((StatusCode::OK, [(header::CONTENT_TYPE, "text/plain")], process_metrics(&data)).into_response())
}

pub async fn pdu_names(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>
) -> axum::response::Result<impl IntoResponse> {
    log::debug!("pdu_names called with params: {:?}", params);
    let data = fetch_pdu_data(&state, &params).await?;

    let mut pdu_names: HashMap<String, String> = HashMap::new();

    let mut address = 1;
    for i in (0..RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        log::debug!("Inserting pdu_{}: # {} {}", address, address, data[i+1]);
        pdu_names.insert(format!("pdu_{}", address), format!("# {} {}", address, data[i+1]));
        address += 1;
    }

    log::debug!("rack_names response: {:?}", pdu_names);

    Ok((StatusCode::OK, Json(HashMap::from([("pdu_names", pdu_names)]))).into_response())
}

pub async fn rack_names(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>
) -> axum::response::Result<impl IntoResponse> {
    log::debug!("rack_names called with params: {:?}", params);
    let data = fetch_pdu_data(&state, &params).await?;

    let mut rack_names: HashSet<String> = HashSet::new();

    for i in (0..RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        if data[i+1].is_empty() {
            continue;
        }

        let rack_name = strip_suffix_ab(&data[i+1])
            .trim()
            .to_string();

        rack_names.insert(rack_name);
    }

    let mut rack_names: Vec<String> = rack_names
        .into_iter()
        .collect::<Vec<String>>();

    rack_names.sort();
    rack_names.resize(32, String::from("-"));

    let mut rack_names_map: BTreeMap<String, String> = BTreeMap::new();

    for (id, name) in rack_names.into_iter().enumerate() {
        rack_names_map.insert(format!("rack_{}", id+1), name);
    }

    log::debug!("rack_names response: {:?}", rack_names_map);

    Ok((StatusCode::OK, Json(HashMap::from([("rack_names", rack_names_map)]))).into_response())
}

pub async fn rack_metrics(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>
) -> axum::response::Result<impl IntoResponse> {
    log::debug!("rack_metrics called with params: {:?}", params);
    let data = fetch_pdu_data(&state, &params).await?;

    let mut pdus: Vec<Pdu> = Vec::new();
    let mut address: i8 = 1;

    for i in (0..RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        let name = data[i+1].to_string();

        if name.is_empty() {
            continue;
        }

        let current = data[i+10].parse::<f32>().unwrap_or(0.0);
        let voltage = data[i+11].parse::<f32>().unwrap_or(0.0);
        let power = data[i+12].parse::<f32>().unwrap_or(0.0);

        let pdu = Pdu {
            address: address.to_string(),
            name,
            current,
            voltage,
            power,
        };

        pdus.push(pdu);
        address += 1;
    }

    pdus.sort_by(|a, b| {
        a.name.is_empty()
            .cmp(&b.name.is_empty())
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut rack_metrics: BTreeMap<String, Vec<Pdu>> = BTreeMap::new();

    for pdu in pdus {
        let rack_name = strip_suffix_ab(&pdu.name)
            .trim()
            .to_string();

        if let Some(x) = rack_metrics.get_mut(&rack_name) {
            x.push(pdu);
        } else {
            rack_metrics.insert(rack_name, vec![pdu]);
        }
    }

    for (_, v) in rack_metrics.iter_mut() {
        let mut total_current: f32 = 0.0;
        let mut total_voltage: f32 = 0.0;
        let mut total_power: f32 = 0.0;

        for i in v.iter() {
            total_current += i.current;
            total_voltage += i.voltage;
            total_power += i.power;
        }

        total_voltage /= v.len().to_string().parse::<f32>().unwrap_or(1.0);

        let pdu = Pdu {
            address: String::from(""),
            name: String::from("Total"),
            current: total_current,
            voltage: total_voltage,
            power: total_power,
        };

        v.push(pdu);
    }

    let empty_metrics = Pdu {
        address: String::from(""),
        name: String::from(""),
        current: 0.0,
        voltage: 0.0,
        power: 0.0,
    };

    rack_metrics.insert(String::from("-"), vec![empty_metrics]);

    Ok((StatusCode::OK, Json(HashMap::from([("rack_metrics", rack_metrics)]))).into_response())
}
