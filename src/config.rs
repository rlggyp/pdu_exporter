use crate::Error;

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ScrapeConfigs {
    #[serde(default = "ScrapeConfigs::default_scrape_timeout_seconds")]
    pub scrape_timeout_seconds: u64,
}

impl ScrapeConfigs {
    fn default_scrape_timeout_seconds() -> u64 {
        10
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "Config::default_scrape_configs")]
    pub scrape_configs: ScrapeConfigs,
    #[serde(default = "Config::default_basic_auth_users")]
    pub basic_auth_users: HashMap<String, String>,
}

impl Config {
    fn default_basic_auth_users() -> HashMap<String, String> {
        HashMap::new()
    }

    fn default_scrape_configs() -> ScrapeConfigs {
        ScrapeConfigs {
            scrape_timeout_seconds: 10,
        }
    }
}

impl Config {
    pub fn get_config() -> Result<Self, Error> {
        let config_file = std::env::var("CONFIG_FILE")
            .map_err(|e| {
                let error = format!("Environment variable `CONFIG_FILE` not found {}", e);
                log::error!("{}", error);
                error
            })?;

        let file = std::fs::File::open(config_file)
            .map_err(|e| {
                log::error!("failed to open config file: {}", e);
                e
            })?;

        let config: Config = serde_yaml::from_reader(file)?;

        Ok(config)
    }
}