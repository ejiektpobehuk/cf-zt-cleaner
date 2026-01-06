use crate::error::{Error, Result};
use crate::user::CloudFlareUser;
use reqwest::blocking::Client;
use serde::Deserialize;
use tracing::{debug, info};

const CLOUDFLARE_API_BASE: &str = "https://api.cloudflare.com/client/v4";
const DEFAULT_PER_PAGE: u32 = 1000;

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

    /// Fetch all Access users from `CloudFlare`
    pub fn get_users(&self) -> Result<Vec<CloudFlareUser>> {
        let base_url = format!(
            "{}/accounts/{}/access/users",
            CLOUDFLARE_API_BASE, self.account_id
        );

        let mut all_users = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!("{base_url}?page={page}&per_page={DEFAULT_PER_PAGE}");
            debug!("Fetching users from: {}", url);

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.api_token))
                .header("Content-Type", "application/json")
                .send()?;

            let api_response: ApiResponse<Vec<CloudFlareUser>> = response.json()?;

            if !api_response.success {
                if let Some(err) = api_response.errors.first() {
                    return Err(Error::CloudFlareApi {
                        code: err.code,
                        message: err.message.clone(),
                    });
                }
                return Err(Error::CloudFlareUnsuccessful);
            }

            let users = api_response.result.unwrap_or_default();
            let users_count = users.len();
            all_users.extend(users);

            // Check if we need to fetch more pages
            if let Some(result_info) = api_response.result_info {
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

    /// Delete a user by their ID
    pub fn delete_user(&self, user_id: &str) -> Result<()> {
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

        let api_response: ApiResponse<()> = response.json()?;

        if !api_response.success {
            if let Some(err) = api_response.errors.first() {
                return Err(Error::CloudFlareApi {
                    code: err.code,
                    message: err.message.clone(),
                });
            }
            return Err(Error::CloudFlareUnsuccessful);
        }

        info!("Successfully deleted user: {}", user_id);
        Ok(())
    }
}
