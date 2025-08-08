use super::{METRIC_STEP, RAW_DATA_LENGTH};

struct GaugeVec {
    name: String,
    help: String,
    labels: Vec<String>,
    samples: Vec<String>,
}

impl GaugeVec {
    fn new(name: &str, help: &str, labels: &[&str]) -> Self {
        let help_str = format!("# HELP {name} {help}\n# TYPE {name} gauge\n");
        Self {
            name: name.to_string(),
            help: help_str,
            labels: labels.into_iter().map(|s| s.to_string()).collect(),
            samples: Vec::new(),
        }
    }

    fn with_label_values(&mut self, labels: &[&str]) -> GaugeSample<'_> {
        let label_str = self.labels.iter()
            .zip(labels.iter())
            .map(|(k, v)| format!(r#"{}="{}""#, k, v))
            .collect::<Vec<String>>()
            .join(",");

        GaugeSample {
            name: &self.name,
            labels: label_str,
            samples: &mut self.samples,
        }
    }

    fn render(&mut self) -> String {
        self.samples.sort();
        format!("{}{}", self.help, self.samples.join("\n"))
    }
}

struct GaugeSample<'a> {
    name: &'a str,
    labels: String,
    samples: &'a mut Vec<String>,
}

impl<'a> GaugeSample<'a> {
    fn set(&mut self, value: &str) {
        let formatted = format_number(value);
        let sample = format!("{}{{{}}} {}", self.name, self.labels, formatted);
        self.samples.push(sample);
    }
}

fn format_number(number: &str) -> String {
    if let Ok(num) = number.parse::<f64>() {
        if !num.is_finite() {
            return "0".to_string()
        }

        if num.fract() == 0.0 {
            (num as i64).to_string()
        } else {
            num.to_string()
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

impl PduMetrics {
    fn export_metrics(&mut self) -> String {
       let mut metrics = vec![
           &mut self.current,
           &mut self.voltage,
           &mut self.power,
           &mut self.power_factor,
           &mut self.energy,
           &mut self.temperature,
           &mut self.humidity,
           &mut self.sensor_exists,
        ];

       metrics.sort_by(|a, b| a.name.cmp(&b.name));
       metrics.iter_mut()
           .filter_map(|m| {
               if !m.samples.is_empty() {
                   Some(m.render())
               } else {
                   None
               }
           })
           .collect::<Vec<String>>()
           .join("\n")
    }
}

pub fn process_metrics(data: &Box<[Box<str>]>) -> String {
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

        const TEMP_INDEX_OFFSET: usize = 15;

        for j in 0..16 {
            let index = i + TEMP_INDEX_OFFSET + (j * 3);
            let channel = (j + 1).to_string();
            if !data[index].is_empty() {
                metrics.temperature.with_label_values(&[&address, &channel]).set(&data[index+1]);
                metrics.humidity.with_label_values(&[&address, &channel]).set(&data[index+2]);
            }
        }
    }

    let has_temperature = (!metrics.temperature.samples.is_empty() as u8).to_string();
    let has_humidity = (!metrics.humidity.samples.is_empty() as u8).to_string();

    metrics.sensor_exists.with_label_values(&["temperature"]).set(&has_temperature);
    metrics.sensor_exists.with_label_values(&["humidity"]).set(&has_humidity);
    metrics.export_metrics()
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
