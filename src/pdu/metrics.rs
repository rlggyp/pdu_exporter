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
        log::debug!("Creating GaugeVec: name={}, help={}, labels={:?}", name, help, labels);
        Self {
            name: name.to_string(),
            help: help_str,
            labels: labels.into_iter().map(|s| s.to_string()).collect(),
            samples: Vec::new(),
        }
    }

    fn with_label_values(&mut self, labels: &[&str]) -> GaugeSample<'_> {
        log::debug!("with_label_values for {} with labels {:?}", self.name, labels);
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
        log::debug!("Rendering GaugeVec: {}", self.name);
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
        log::debug!("Setting sample for {} with labels {{{}}}: value={}", self.name, self.labels, value);
        let formatted = format_number(value);

        let sample: String;
        if self.labels.is_empty() {
            sample = format!("{} {}", self.name, formatted);
        } else {
            sample = format!("{}{{{}}} {}", self.name, self.labels, formatted);
        }

        self.samples.push(sample);
    }
}

fn format_number(number: &str) -> String {
    log::debug!("Formatting number: {}", number);
    if let Ok(num) = number.parse::<f64>() {
        if !num.is_finite() {
            log::debug!("Number is not finite, returning 0");
            return "0".to_string()
        }

        if num.fract() == 0.0 {
            log::debug!("Number is integer: {}", num);
            (num as i64).to_string()
        } else {
            log::debug!("Number is float: {}", num);
            num.to_string()
        }
    } else {
        log::debug!("Failed to parse number, returning 0");
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
    total_load: GaugeVec,
    total_load_current: GaugeVec,
}

impl PduMetrics {
    fn export_metrics(&mut self) -> String {
       log::debug!("Exporting metrics");
       let mut metrics = vec![
           &mut self.current,
           &mut self.voltage,
           &mut self.power,
           &mut self.power_factor,
           &mut self.energy,
           &mut self.temperature,
           &mut self.humidity,
           &mut self.sensor_exists,
           &mut self.total_load,
           &mut self.total_load_current,
        ];

       metrics.sort_by(|a, b| a.name.cmp(&b.name));
       metrics.iter_mut()
           .filter_map(|m| {
               if !m.samples.is_empty() {
                   log::debug!("Rendering metric: {}", m.name);
                   Some(m.render())
               } else {
                   log::debug!("Skipping empty metric: {}", m.name);
                   None
               }
           })
           .collect::<Vec<String>>()
           .join("\n")
    }
}

pub fn process_metrics(data: &Box<[Box<str>]>) -> String {
    log::debug!("Processing metrics for data length: {}", data.len());
    let mut metrics = build_metrics();

    let mut total_load: f32 = 0.0;
    let mut total_load_current: f32 = 0.0;

    let mut addr = 1;
    for i in (0..RAW_DATA_LENGTH).step_by(METRIC_STEP) {
        let address = addr.to_string();
        log::debug!("Processing address: {}", address);
        addr += 1;

        metrics.current.with_label_values(&[&address]).set(&data[i+10]);
        metrics.voltage.with_label_values(&[&address]).set(&data[i+11]);
        metrics.power.with_label_values(&[&address]).set(&data[i+12]);
        metrics.power_factor.with_label_values(&[&address]).set(&data[i+13]);
        metrics.energy.with_label_values(&[&address]).set(&data[i+14]);

        total_load += (data[i+12]).parse::<f32>().unwrap_or(0.0);
        total_load_current += (data[i+10]).parse::<f32>().unwrap_or(0.0);

        const TEMP_INDEX_OFFSET: usize = 15;

        for j in 0..16 {
            let index = i + TEMP_INDEX_OFFSET + (j * 3);
            let channel = (j + 1).to_string();
            if !data[index].is_empty() {
                log::debug!("Processing temperature/humidity for address={}, channel={}", address, channel);
                metrics.temperature.with_label_values(&[&address, &channel]).set(&data[index+1]);
                metrics.humidity.with_label_values(&[&address, &channel]).set(&data[index+2]);
            } else {
                log::debug!("No temperature/humidity data for address={}, channel={}", address, channel);
            }
        }
    }

    let has_temperature = (!metrics.temperature.samples.is_empty() as u8).to_string();
    let has_humidity = (!metrics.humidity.samples.is_empty() as u8).to_string();

    log::debug!("Sensor exists: temperature={}, humidity={}", has_temperature, has_humidity);

    metrics.total_load.with_label_values(&[]).set(&total_load.to_string());
    metrics.total_load_current.with_label_values(&[]).set(&total_load_current.to_string());

    metrics.sensor_exists.with_label_values(&["temperature"]).set(&has_temperature);
    metrics.sensor_exists.with_label_values(&["humidity"]).set(&has_humidity);
    let result = metrics.export_metrics();
    log::debug!("Final metrics output:\n{}", result);
    result
}

fn build_metrics() -> PduMetrics {
    log::debug!("Building PduMetrics struct");
    PduMetrics {
        current: GaugeVec::new("current", "Current in ampere", &["address"]),
        voltage: GaugeVec::new("voltage", "Voltage in volt", &["address"]),
        power: GaugeVec::new("power", "Power in watt", &["address"]),
        power_factor: GaugeVec::new("power_factor", "Power factor in ratio (0.0 - 1.0)", &["address"]),
        energy: GaugeVec::new("energy", "Energy in kWh", &["address"]),
        temperature: GaugeVec::new("temperature", "Temperature in celsius", &["address", "channel"]),
        humidity: GaugeVec::new("humidity", "Humidity in percent", &["address", "channel"]),
        sensor_exists: GaugeVec::new("sensor_exists", "Sensor exists (bool)", &["type"]),
        total_load: GaugeVec::new("total_load", "Total load in watt(W)", &[]),
        total_load_current: GaugeVec::new("total_load_current", "Total load current in ampere(A)", &[]),
    }
}
