const TEMP_INDEX_OFFSET: usize = 15;

pub struct GaugeVec {
    name: String,
    help: String,
    labels: Vec<String>,
    output: String,
}

impl GaugeVec {
    fn new(name: &str, help: &str, labels: &[&str]) -> Self {
        let help_str = format!("# HELP {name} {help}\nTYPE {name} gauge\n");
        Self {
            name: name.to_string(),
            help: help_str,
            labels: labels.into_iter().map(|s| s.to_string()).collect(),
            output: String::new(),
        }
    }

    fn with_label_values(&mut self, labels: &[&str]) -> GaugeSample<'_> {
        let label_str = self.labels.iter()
            .zip(labels.iter())
            .map(|(k, v)| format!(r#"{}="{}""#, k, v))
            .collect::<Vec<_>>()
            .join(",");

        GaugeSample {
            name: &self.name,
            labels: label_str,
            output: &mut self.output,
        }
    }

    fn render(&self) -> String {
        format!("{}{}", self.help, self.output)
    }
}

pub struct GaugeSample<'a> {
    name: &'a str,
    labels: String,
    output: &'a mut String,
}

impl<'a> GaugeSample<'a> {
    pub fn set(&mut self, value: &str) {
        let formatted = format_number(value);
        self.output
            .push_str(&format!("{}{{{}}} {}", self.name, self.labels, formatted));
    }
}

fn format_number(number: &str) -> String {
    if let Ok(num) = number.parse::<f64>() {
        if num.fract() == 0.0 {
            (num as i32).to_string()
        } else {
            number.to_string()
        }
    } else {
        "0".to_string()
    }
}

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

use super::{METRIC_STEP, RAW_DATA_LENGTH};

pub fn process_metrics(data: Vec<String>) -> String {
    let mut metrics = build_metrics();

    let mut addr = 1;
    for i in (0..RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        let address = addr.to_string();
        addr += 1;

        metrics.current.with_label_values(&[&address]).set(&data[i+10]);
        metrics.voltage.with_label_values(&[&address]).set(&data[i+11]);
        metrics.power.with_label_values(&[&address]).set(&data[i+12]);
        metrics.power_factor.with_label_values(&[&address]).set(&data[i+13]);
        metrics.energy.with_label_values(&[&address]).set(&data[i+14]);

        for j in 0..16 {
            let index = i + TEMP_INDEX_OFFSET + (j * 3);
            let channel = (j + 1).to_string();
            if !data[index].is_empty() {
                metrics.temperature.with_label_values(&[&address, &channel]).set(&data[index+1]);
                metrics.humidity.with_label_values(&[&address, &channel]).set(&data[index+2]);
            }
        }
    }

    let has_temperature = (!metrics.temperature.output.is_empty() as u8).to_string();
    let has_humidity = (!metrics.humidity.output.is_empty()).to_string();

    metrics.sensor_exists.with_label_values(&["temperature"]).set(&has_temperature);
    metrics.sensor_exists.with_label_values(&["humidity"]).set(&has_humidity);

    vec![
        metrics.current.render(),
        metrics.voltage.render(),
        metrics.power.render(),
        metrics.power_factor.render(),
        metrics.energy.render(),
        metrics.temperature.render(),
        metrics.humidity.render(),
        metrics.sensor_exists.render(),
    ].join("")
}

fn build_metrics() -> PduMetrics {
    PduMetrics {
        current: GaugeVec::new("current", "Current in ampere", &["address"]),
        voltage: GaugeVec::new("voltage", "Voltage in volt", &["address"]),
        power: GaugeVec::new("power", "Power in watt", &["address"]),
        power_factor: GaugeVec::new("power_factor", "Power factor in ratio (0.0 - 1.0)", &["address"]),
        energy: GaugeVec::new("energy", "Energy in kWh", &["address"]),
        temperature: GaugeVec::new("temperature", "Temperature in celsius", &["address", "channel"]),
        humidity: GaugeVec::new("humidity", "Humidity in percent", &["address", "channel"]),
        sensor_exists: GaugeVec::new("sensor_exists", "Sensor exists (bool)", &["type"]),
    }
}
