use crate::error::Result;
use serde::Deserialize;
use std::env;
use std::path::Path;
use tracing::warn;

/// Environment variable name for `CloudFlare` account ID
pub const ENV_CF_ACCOUNT_ID: &str = "CF_ACCOUNT_ID";
/// Environment variable name for `CloudFlare` API token
pub const ENV_CF_API_TOKEN: &str = "CF_API_TOKEN";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub cloudflare: CloudFlareConfig,
    pub users: UsersConfig,
}

/// Raw config as read from TOML file (credentials are optional)
#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default)]
    cloudflare: RawCloudFlareConfig,
    #[serde(default)]
    users: UsersConfig,
}

#[derive(Debug, Deserialize, Default)]
struct RawCloudFlareConfig {
    account_id: Option<String>,
    api_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloudFlareConfig {
    pub account_id: String,
    pub api_token: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct UsersConfig {
    pub permanent: Vec<String>,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            warn!(
                "Configuration file not found at '{}'. Continuing without config file (permanent user list will be empty).",
                path.display()
            );

            // No config file: credentials must come from environment variables, and the permanent
            // user list defaults to empty.
            let cloudflare = Self::resolve_cloudflare_config(RawCloudFlareConfig::default())?;
            return Ok(Self {
                cloudflare,
                users: UsersConfig::default(),
            });
        }
        let content = std::fs::read_to_string(path)?;
        let raw_config: RawConfig = toml::from_str(&content)?;

        // Resolve credentials from config file and environment variables
        let cloudflare = Self::resolve_cloudflare_config(raw_config.cloudflare)?;

        Ok(Self {
            cloudflare,
            users: raw_config.users,
        })
    }

    /// Resolve `CloudFlare` credentials from config file and environment variables.
    /// Environment variables take priority. Warns if both sources provide a value.
    fn resolve_cloudflare_config(raw: RawCloudFlareConfig) -> Result<CloudFlareConfig> {
        let env_account_id = env::var(ENV_CF_ACCOUNT_ID).ok();
        let env_api_token = env::var(ENV_CF_API_TOKEN).ok();

        // Check for clashes and warn
        if raw.account_id.is_some() && env_account_id.is_some() {
            warn!(
                "CloudFlare account_id specified in both config file and {} environment variable. \
                 Using environment variable.",
                ENV_CF_ACCOUNT_ID
            );
        }
        if raw.api_token.is_some() && env_api_token.is_some() {
            warn!(
                "CloudFlare api_token specified in both config file and {} environment variable. \
                 Using environment variable.",
                ENV_CF_API_TOKEN
            );
        }

        // Environment variables take priority over config file
        let account_id = env_account_id
            .or(raw.account_id)
            .ok_or_else(|| crate::error::Error::MissingCredential("account_id".to_string()))?;

        let api_token = env_api_token
            .or(raw.api_token)
            .ok_or_else(|| crate::error::Error::MissingCredential("api_token".to_string()))?;

        Ok(CloudFlareConfig {
            account_id,
            api_token,
        })
    }
}
