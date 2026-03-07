use anyhow::{Context, Result};
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Cloudflare API client for managing tunnels
pub struct CloudflareClient {
    api_token: String,
    account_id: String,
    api_base_url: String,
    http_client: reqwest::Client,
}

/// Credentials returned when creating a tunnel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelCredentials {
    pub tunnel_id: String,
    pub account_tag: String,
    pub secret: String,
    pub credentials_file: String, // JSON content for cloudflared
}

/// Status of a Cloudflare tunnel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStatus {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub connections: Vec<TunnelConnection>,
}

/// Connection status for a tunnel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConnection {
    pub id: String,
    pub client_id: String,
    pub client_version: String,
    pub opened_at: String,
}

/// Response from Cloudflare API when creating a tunnel
#[derive(Debug, Deserialize)]
struct CreateTunnelResponse {
    result: CreateTunnelResult,
    success: bool,
    errors: Vec<CloudflareError>,
    #[allow(dead_code)]
    messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateTunnelResult {
    id: String,
    #[allow(dead_code)]
    name: String,
    secret: String,
    #[allow(dead_code)]
    created_at: String,
}

/// Response from Cloudflare API when getting tunnel status
#[derive(Debug, Deserialize)]
struct GetTunnelResponse {
    result: TunnelStatus,
    success: bool,
    errors: Vec<CloudflareError>,
}

/// Response from Cloudflare API when verifying account access
#[derive(Debug, Deserialize)]
struct GetAccountResponse {
    #[allow(dead_code)]
    result: serde_json::Value,
    success: bool,
    errors: Vec<CloudflareError>,
}

/// Response from Cloudflare API when creating a DNS record
#[derive(Debug, Deserialize)]
struct CreateDnsRecordResponse {
    result: CreateDnsRecordResult,
    success: bool,
    errors: Vec<CloudflareError>,
}

#[derive(Debug, Deserialize)]
struct CreateDnsRecordResult {
    id: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    content: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    record_type: String,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    code: i32,
    message: String,
}

impl CloudflareClient {
    /// Create a new Cloudflare client
    pub fn new(api_token: String, account_id: String) -> Result<Self> {
        let api_base_url = std::env::var("CLOUDFLARE_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.cloudflare.com/client/v4".to_string());
        Self::new_with_base_url(api_token, account_id, api_base_url)
    }

    /// Create a new Cloudflare client with explicit API base URL.
    pub fn new_with_base_url(
        api_token: String,
        account_id: String,
        api_base_url: String,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            api_token,
            account_id,
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            http_client,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.api_base_url, path.trim_start_matches('/'))
    }

    /// Create a new Cloudflare tunnel
    pub async fn create_tunnel(&self, name: &str) -> Result<TunnelCredentials> {
        let url = self.endpoint(&format!("accounts/{}/cfd_tunnel", self.account_id));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .context("Invalid API token")?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let body = serde_json::json!({
            "name": name,
            "tunnel_secret": base64::engine::general_purpose::STANDARD.encode(Uuid::new_v4().as_bytes()),
        });

        let response = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Failed to send create tunnel request")?;

        let status = response.status();
        let response_body: CreateTunnelResponse = response
            .json()
            .await
            .context("Failed to parse create tunnel response")?;

        if !response_body.success {
            let error_messages: Vec<String> = response_body
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            anyhow::bail!(
                "Failed to create tunnel (status {}): {}",
                status,
                error_messages.join(", ")
            );
        }

        let result = response_body.result;

        // Generate credentials file content
        let credentials = TunnelCredentials {
            tunnel_id: result.id.clone(),
            account_tag: self.account_id.clone(),
            secret: result.secret.clone(),
            credentials_file: serde_json::to_string(&serde_json::json!({
                "AccountTag": self.account_id,
                "TunnelSecret": result.secret,
                "TunnelID": result.id,
            }))
            .context("Failed to serialize credentials")?,
        };

        Ok(credentials)
    }

    /// Delete a Cloudflare tunnel
    pub async fn delete_tunnel(&self, tunnel_id: &str) -> Result<()> {
        let url = self.endpoint(&format!(
            "accounts/{}/cfd_tunnel/{}",
            self.account_id, tunnel_id
        ));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .context("Invalid API token")?,
        );

        let response = self
            .http_client
            .delete(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to send delete tunnel request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to delete tunnel (status {}): {}", status, body);
        }

        Ok(())
    }

    /// Get tunnel status and connection information
    pub async fn get_tunnel_status(&self, tunnel_id: &str) -> Result<TunnelStatus> {
        let url = self.endpoint(&format!(
            "accounts/{}/cfd_tunnel/{}",
            self.account_id, tunnel_id
        ));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .context("Invalid API token")?,
        );

        let response = self
            .http_client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to send get tunnel request")?;

        let status = response.status();
        let response_body: GetTunnelResponse = response
            .json()
            .await
            .context("Failed to parse get tunnel response")?;

        if !response_body.success {
            let error_messages: Vec<String> = response_body
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            anyhow::bail!(
                "Failed to get tunnel status (status {}): {}",
                status,
                error_messages.join(", ")
            );
        }

        Ok(response_body.result)
    }

    /// Verify the API token can access the configured Cloudflare account.
    pub async fn verify_account_access(&self) -> Result<()> {
        let url = self.endpoint(&format!("accounts/{}", self.account_id));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .context("Invalid API token")?,
        );

        let response = self
            .http_client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to send account verification request")?;

        let status = response.status();
        let response_body: GetAccountResponse = response
            .json()
            .await
            .context("Failed to parse account verification response")?;

        if !response_body.success {
            let error_messages: Vec<String> = response_body
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            anyhow::bail!(
                "Failed to verify Cloudflare account access (status {}): {}",
                status,
                error_messages.join(", ")
            );
        }

        Ok(())
    }

    /// Generate preview URL
    pub fn generate_preview_url(&self, tunnel_id: &str) -> String {
        // Cloudflare tunnel URLs follow the pattern: https://{tunnel-id}.cfargotunnel.com
        format!("https://{}.cfargotunnel.com", tunnel_id)
    }

    /// Create a DNS record for the tunnel
    pub async fn create_dns_record(
        &self,
        zone_id: &str,
        name: &str,
        content: &str,
        record_type: &str,
        proxied: bool,
    ) -> Result<String> {
        let url = self.endpoint(&format!("zones/{}/dns_records", zone_id));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .context("Invalid API token")?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let body = serde_json::json!({
            "type": record_type,
            "name": name,
            "content": content,
            "proxied": proxied,
            "ttl": 1, // Automatic
        });

        let response = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Failed to send create DNS record request")?;

        let status = response.status();
        let response_body: CreateDnsRecordResponse = response
            .json()
            .await
            .context("Failed to parse create DNS record response")?;

        if !response_body.success {
            let error_messages: Vec<String> = response_body
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            anyhow::bail!(
                "Failed to create DNS record (status {}): {}",
                status,
                error_messages.join(", ")
            );
        }

        Ok(response_body.result.id)
    }

    /// Delete a DNS record
    pub async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<()> {
        let url = self.endpoint(&format!("zones/{}/dns_records/{}", zone_id, record_id));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .context("Invalid API token")?,
        );

        let response = self
            .http_client
            .delete(&url)
            .headers(headers)
            .send()
            .await
            .context("Failed to send delete DNS record request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to delete DNS record (status {}): {}", status, body);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_preview_url() {
        let client =
            CloudflareClient::new("test-token".to_string(), "test-account".to_string()).unwrap();

        let url = client.generate_preview_url("abc123-def456");
        assert_eq!(url, "https://abc123-def456.cfargotunnel.com");
    }

    #[test]
    fn test_endpoint_uses_custom_base_url() {
        let client = CloudflareClient::new_with_base_url(
            "test-token".to_string(),
            "test-account".to_string(),
            "http://127.0.0.1:5000/client/v4/".to_string(),
        )
        .unwrap();

        let endpoint = client.endpoint("accounts/test-account/cfd_tunnel");
        assert_eq!(
            endpoint,
            "http://127.0.0.1:5000/client/v4/accounts/test-account/cfd_tunnel"
        );
    }
}
