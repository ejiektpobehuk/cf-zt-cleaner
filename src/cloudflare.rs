use crate::error::{Error, Result};
use crate::user::CloudFlareUser;
use backon::{BlockingRetryable, ExponentialBuilder};
use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tracing::{debug, info, warn};

const CLOUDFLARE_API_BASE: &str = "https://api.cloudflare.com/client/v4";
const DEFAULT_PER_PAGE: u32 = 1000;

/// Default retry configuration for API calls
fn default_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_secs(1))
        .with_max_delay(Duration::from_secs(60))
        .with_max_times(5)
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    errors: Vec<ApiError>,
    result: Option<T>,
    result_info: Option<ResultInfo>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: i32,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ResultInfo {
    count: u32,
    page: u32,
    per_page: u32,
    total_count: u32,
}

pub struct CloudFlareClient {
    client: Client,
    account_id: String,
    api_token: String,
}

impl CloudFlareClient {
    pub fn new(account_id: String, api_token: String) -> Self {
        Self {
            client: Client::new(),
            account_id,
            api_token,
        }
    }

    /// Handle API response, checking for rate limits and errors
    fn handle_response<T: DeserializeOwned>(response: Response) -> Result<ApiResponse<T>> {
        let status = response.status();

        // Check for rate limit via HTTP status
        if status == StatusCode::TOO_MANY_REQUESTS {
            // If Retry-After header is present, sleep for that duration
            if let Some(retry_after) = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
            {
                debug!("Rate limited, Retry-After: {} seconds", retry_after);
                std::thread::sleep(Duration::from_secs(retry_after));
            }
            return Err(Error::RateLimited);
        }

        // Parse JSON response
        let api_response: ApiResponse<T> = response.json()?;

        // Check for rate limit in response body (CF error code 10015)
        if !api_response.success {
            if let Some(err) = api_response.errors.first() {
                if err.code == 10015 {
                    return Err(Error::RateLimited);
                }
                return Err(Error::CloudFlareApi {
                    code: err.code,
                    message: err.message.clone(),
                });
            }
            return Err(Error::CloudFlareUnsuccessful);
        }

        Ok(api_response)
    }

    /// Fetch a single page of users
    fn fetch_users_page(&self, page: u32) -> Result<(Vec<CloudFlareUser>, Option<ResultInfo>)> {
        let url = format!(
            "{}/accounts/{}/access/users?page={}&per_page={}",
            CLOUDFLARE_API_BASE, self.account_id, page, DEFAULT_PER_PAGE
        );
        debug!("Fetching users from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()?;

        let api_response: ApiResponse<Vec<CloudFlareUser>> = Self::handle_response(response)?;
        let users = api_response.result.unwrap_or_default();

        Ok((users, api_response.result_info))
    }

    /// Fetch all Access users from `CloudFlare` with retry logic
    pub fn get_users(&self) -> Result<Vec<CloudFlareUser>> {
        let mut all_users = Vec::new();
        let mut page = 1u32;

        loop {
            // Each page fetch retries independently
            let current_page = page;
            let (users, result_info) = (|| self.fetch_users_page(current_page))
                .retry(default_backoff())
                .sleep(std::thread::sleep)
                .when(super::error::Error::is_retryable)
                .notify(|err, dur| {
                    warn!(
                        "Retrying page {} after {:?} due to: {}",
                        current_page, dur, err
                    );
                })
                .call()?;

            let users_count = users.len();
            all_users.extend(users);

            // Check if we need to fetch more pages
            if let Some(result_info) = result_info {
                debug!(
                    "Page {}/{} fetched ({} users, {} total)",
                    result_info.page,
                    result_info.total_count.div_ceil(result_info.per_page),
                    result_info.count,
                    result_info.total_count
                );

                // If we've fetched all users or this page was empty, we're done
                if all_users.len() >= result_info.total_count as usize || users_count == 0 {
                    break;
                }
                page += 1;
            } else {
                // No pagination info means single page response
                break;
            }
        }

        info!("Fetched {} users from CloudFlare", all_users.len());
        Ok(all_users)
    }

    /// Delete a user by their ID (internal implementation)
    fn delete_user_inner(&self, user_id: &str) -> Result<()> {
        let url = format!(
            "{}/accounts/{}/access/users/{}",
            CLOUDFLARE_API_BASE, self.account_id, user_id
        );

        debug!("Deleting user: {}", user_id);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()?;

        let _: ApiResponse<()> = Self::handle_response(response)?;
        Ok(())
    }

    /// Delete a user by their ID with retry logic
    pub fn delete_user(&self, user_id: &str) -> Result<()> {
        let id = user_id.to_string();
        (|| self.delete_user_inner(&id))
            .retry(default_backoff())
            .sleep(std::thread::sleep)
            .when(super::error::Error::is_retryable)
            .notify(|err, dur| {
                warn!("Retrying delete of {} after {:?}: {}", id, dur, err);
            })
            .call()?;

        info!("Successfully deleted user: {}", user_id);
        Ok(())
    }
}
