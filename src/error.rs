use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Failed to parse configuration: {0}")]
    Config(#[from] toml::de::Error),

    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),

    #[error("CloudFlare API error: {message} (code: {code})")]
    CloudFlareApi { code: i32, message: String },

    #[error("CloudFlare API returned unsuccessful response")]
    CloudFlareUnsuccessful,

    #[error("Rate limited by CloudFlare API")]
    RateLimited,

    #[error("Missing CloudFlare credential: {0} (set in config file or via environment variable)")]
    MissingCredential(String),
}

impl Error {
    /// Returns true if this error is retryable (transient)
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited => true,
            Self::Request(e) => e.is_timeout() || e.is_connect(),
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
