use crate::error::Result;
use crate::user::ConfigUser;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub cloudflare: CloudFlareConfig,
    pub users: UsersConfig,
}

#[derive(Debug, Deserialize)]
pub struct CloudFlareConfig {
    pub account_id: String,
    pub api_token: String,
}

#[derive(Debug, Deserialize)]
pub struct UsersConfig {
    pub permanent: Vec<ConfigUser>,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}
