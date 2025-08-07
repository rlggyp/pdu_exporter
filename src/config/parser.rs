use std::env;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ScrapeConfigs {
    pub scrape_timeout: u64,
}

impl ScrapeConfigs {
    fn new() -> Self {
        Self { scrape_timeout: 0 }
    }
}

#[derive(Debug)]
pub struct BasicAuthUsers {
    pub credentials: HashMap<String, String>,
}

impl BasicAuthUsers {
    fn new() -> Self {
        Self { credentials: HashMap::new() }
    }
}

#[derive(Debug)]
pub struct PduExporterConfig {
    pub scrape_configs: ScrapeConfigs,
    pub basic_auth_users: BasicAuthUsers,
}

impl PduExporterConfig {
    fn load_from_file(filepath: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(filepath).map_err(|e| e.to_string())?;
        Self::parse_yaml(&content)
    }

    fn parse_yaml(yaml_content: &str) -> Result<Self, String> {
        let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_content).map_err(|e| e.to_string())?;

        let mut config = PduExporterConfig {
            scrape_configs: ScrapeConfigs::new(),
            basic_auth_users: BasicAuthUsers::new(),
        };

        if let Some(scrape_configs) = yaml.get("scrape_configs") {
            if let Some(scrape_timeout) = scrape_configs.get("scrape_timeout") {
                if let Some(timeout_str) = scrape_timeout.as_str() {
                    config.scrape_configs.scrape_timeout = Self::parse_seconds(timeout_str)
                        .ok_or_else(|| format!("Invalid scrape_timeout value: {}", timeout_str))?;
                } else {
                    return Err("scrape_timeout must be a string".to_string());
                }
            } else {
                return Err("scrape_timeout is required in scrape_configs".to_string());
            }
        } else {
            return Err("scrape_configs section is required".to_string());
        }

        if let Some(basic_auth_users) = yaml.get("basic_auth_users") {
            let mut credentials: HashMap<String, String> = HashMap::new();

            if let Some(basic_auth_users) = basic_auth_users.as_mapping() {
                for (key, value) in basic_auth_users {
                    credentials.insert(
                        key.as_str().unwrap().to_string(),
                        value.as_str().unwrap().to_string(),
                    );
                }
            }

            if !credentials.is_empty() {
                config.basic_auth_users = BasicAuthUsers { credentials };
            }
        }

        Ok(config)
    }

    fn parse_seconds(s: &str) -> Option<u64> {
        if s.ends_with('s') {
            s.trim_end_matches('s').parse::<u64>().ok()
        } else {
            None
        }
    }
}

pub fn load_config() -> Result<PduExporterConfig, String> {
    let args = parse_args();
    let config_file = args.get("--config.file").ok_or_else(|| {
        "Missing required argument: --config.file".to_string()
    })?;

    PduExporterConfig::load_from_file(config_file)
}

fn parse_args() -> HashMap<String, String> {
    let args: Vec<String> = env::args().collect();
    let mut valid_args: HashMap<String, String> = HashMap::new();

    for arg in args.iter().skip(1) {
        if let Some((key, value)) = arg.split_once('=') {
            valid_args.insert(key.to_string(), value.to_string());
        }
    }

    valid_args
}

