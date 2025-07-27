use prometheus::{GaugeVec, Opts, core::Collector};

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

use super::{METRIC_STEP, RAW_DATA_LENGTH, TEMP_INDEX_OFFSET};

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

pub fn process_metrics(data: Vec<String>) -> Vec<prometheus::proto::MetricFamily> {
    let metrics = build_metrics();

    let mut addr = 1;
    for i in (0..RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        let address = addr.to_string();
        addr += 1;

        metrics.current.with_label_values(&[&address]).set(parse_or_zero(&data[i+10]));
        metrics.voltage.with_label_values(&[&address]).set(parse_or_zero(&data[i+11]));
        metrics.power.with_label_values(&[&address]).set(parse_or_zero(&data[i+12]));
        metrics.power_factor.with_label_values(&[&address]).set(parse_or_zero(&data[i+13]));
        metrics.energy.with_label_values(&[&address]).set(parse_or_zero(&data[i+14]));

        for j in 0..16 {
            let index = i + TEMP_INDEX_OFFSET + (j * 3);
            let channel = (j + 1).to_string();
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

fn build_metrics() -> PduMetrics {
    PduMetrics {
        current: gauge_vec("current", "Current in ampere", &["address"]),
        voltage: gauge_vec("voltage", "Voltage in volt", &["address"]),
        power: gauge_vec("power", "Power in watt", &["address"]),
        power_factor: gauge_vec("power_factor", "Power factor in ratio (0.0 - 1.0)", &["address"]),
        energy: gauge_vec("energy", "Energy in kWh", &["address"]),
        temperature: gauge_vec("temperature", "Temperature in celsius", &["address", "channel"]),
        humidity: gauge_vec("humidity", "Humidity in percent", &["address", "channel"]),
        sensor_exists: gauge_vec("sensor_exists", "Sensor exists (bool)", &["type"]),
    }
}

fn gauge_vec(name: &str, help: &str, labels: &[&str]) -> GaugeVec {
    GaugeVec::new(Opts::new(name, help), labels).expect(&format!("failed to build gauge_vec {}", name))
}

fn parse_or_zero(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}
