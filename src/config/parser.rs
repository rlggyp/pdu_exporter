use std::env;
use std::collections::HashMap;

#[derive(Debug)]
pub struct PduExporterConfig {
    pub scrape_configs: ScrapeConfigs,
    pub basic_auth_users: BasicAuthUsers,
}

impl PduExporterConfig {
    fn new() -> Self {
        Self {
            scrape_configs: ScrapeConfigs::new(),
            basic_auth_users: BasicAuthUsers::new(),
        }
    }
}

#[derive(Debug)]
pub struct ScrapeConfigs {
    pub scrape_timeout: Option<u64>,
}

impl ScrapeConfigs {
    fn new() -> Self {
        Self { scrape_timeout: None }
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

pub fn load_config() -> Result<PduExporterConfig, String> {
    let args = parse_args();
    let mut config = Ok(PduExporterConfig::new());

    for (k, v) in args.iter() {
        match k.as_str() {
            "--config.file" => config = parse_yaml(v),
            _ => continue,
        }
    }

    config
}

fn parse_args() -> HashMap<String, String> {
    let args: Vec<String> = env::args().collect();
    let mut valid_args: HashMap<String, String> = HashMap::new();

    for arg in &args[1..] {
        if arg.starts_with("--config.file=") {
            let arg_split: Vec<&str> = arg.split("=").collect();
            valid_args.insert(arg_split[0].to_string(), arg_split[1].to_string());
        }
    }

    valid_args
}

fn parse_yaml(filepath: &str) -> Result<PduExporterConfig, String> {
    let reader = match std::fs::File::open(filepath) {
        Ok(f) => std::io::BufReader::new(f),
        Err(e) => return Err(e.to_string()),
    };

    let yaml: serde_yaml::Value = match serde_yaml::from_reader(reader) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };

    let mut config = PduExporterConfig::new();

    if let Some(scrape_configs) = yaml.get("scrape_configs") {
        if let Some(scrape_timeout) = scrape_configs.get("scrape_timeout") {
            let scrape_timeout = scrape_timeout.as_str().unwrap();
            config.scrape_configs.scrape_timeout = parse_seconds(scrape_timeout);
        }
    }

    if let Some(basic_auth_users) = yaml.get("basic_auth_users") {
        let mut credentials: HashMap<String, String> = HashMap::new();
        for auth in basic_auth_users.as_mapping().unwrap() {
            credentials.insert(
                auth.0.as_str().unwrap().to_string(),
                auth.1.as_str().unwrap().to_string()
            );
        }

        if !credentials.is_empty() {
            config.basic_auth_users = BasicAuthUsers{ credentials };
        }
    }


    Ok(config)
}

fn parse_seconds(s: &str) -> Option<u64> {
    if let Some(stripped) = s.strip_suffix('s') {
        match stripped.parse::<u64>() {
            Ok(v) => Some(v),
            Err(e) => {
                println!("Expected format like '{{number}}s', but failed to parse '{}': {}", s, e);
                None
            }
        }
    } else {
        println!("Expected string with 's' suffix, got '{}'", s);
        None
    }
}
