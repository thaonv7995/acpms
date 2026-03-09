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
    result: Option<CreateTunnelResult>,
    success: bool,
    errors: Vec<CloudflareError>,
    #[allow(dead_code)]
    messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateTunnelResult {
    id: String,
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    created_at: Option<String>,
    #[allow(dead_code)]
    account_tag: Option<String>,
    credentials_file: CreateTunnelCredentialsFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreateTunnelCredentialsFile {
    #[serde(rename = "AccountTag")]
    account_tag: String,
    #[serde(rename = "TunnelSecret")]
    tunnel_secret: String,
    #[serde(rename = "TunnelID")]
    tunnel_id: String,
    #[serde(rename = "TunnelName")]
    #[allow(dead_code)]
    tunnel_name: Option<String>,
}

/// Response from Cloudflare API when getting tunnel status
#[derive(Debug, Deserialize)]
struct GetTunnelResponse {
    result: Option<TunnelStatus>,
    success: bool,
    errors: Vec<CloudflareError>,
}

/// Response from Cloudflare API when verifying account access
#[derive(Debug, Deserialize)]
struct GetAccountResponse {
    #[allow(dead_code)]
    result: Option<serde_json::Value>,
    success: bool,
    errors: Vec<CloudflareError>,
}

/// Response from Cloudflare API when creating a DNS record
#[derive(Debug, Deserialize)]
struct CreateDnsRecordResponse {
    result: Option<CreateDnsRecordResult>,
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
struct ListDnsRecordsResponse {
    result: Vec<CreateDnsRecordResult>,
    success: bool,
    errors: Vec<CloudflareError>,
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

    async fn parse_json_response<T: serde::de::DeserializeOwned>(
        response: reqwest::Response,
        context_message: &str,
    ) -> Result<T> {
        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("{} (failed to read response body)", context_message))?;

        serde_json::from_str::<T>(&body).with_context(|| {
            format!(
                "{} (status {}): {}",
                context_message,
                status,
                body.chars().take(500).collect::<String>()
            )
        })
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

        let secret_bytes: Vec<u8> = Uuid::new_v4()
            .into_bytes()
            .into_iter()
            .chain(Uuid::new_v4().into_bytes().into_iter())
            .collect();

        let body = serde_json::json!({
            "name": name,
            "tunnel_secret": base64::engine::general_purpose::STANDARD.encode(&secret_bytes),
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
        let response_body: CreateTunnelResponse =
            Self::parse_json_response(response, "Failed to parse create tunnel response").await?;

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

        let result = response_body
            .result
            .context("Cloudflare create tunnel response missing result payload")?;

        // Generate credentials file content
        let tunnel_id = if result.credentials_file.tunnel_id.is_empty() {
            result.id.clone()
        } else {
            result.credentials_file.tunnel_id.clone()
        };
        let account_tag = if result.credentials_file.account_tag.is_empty() {
            result
                .account_tag
                .clone()
                .unwrap_or_else(|| self.account_id.clone())
        } else {
            result.credentials_file.account_tag.clone()
        };
        let credentials = TunnelCredentials {
            tunnel_id,
            account_tag,
            secret: result.credentials_file.tunnel_secret.clone(),
            credentials_file: serde_json::to_string(&result.credentials_file)
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
        let response_body: GetTunnelResponse =
            Self::parse_json_response(response, "Failed to parse get tunnel response").await?;

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

        response_body
            .result
            .context("Cloudflare get tunnel response missing result payload")
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
        let response_body: GetAccountResponse =
            Self::parse_json_response(response, "Failed to parse account verification response")
                .await?;

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
        if let Some(existing_record_id) = self
            .find_dns_record(zone_id, name, record_type)
            .await
            .context("Failed to check existing DNS record before create")?
        {
            return self
                .update_dns_record(
                    zone_id,
                    &existing_record_id,
                    name,
                    content,
                    record_type,
                    proxied,
                )
                .await;
        }

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
        let response_body: CreateDnsRecordResponse =
            Self::parse_json_response(response, "Failed to parse create DNS record response")
                .await?;

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

        Ok(response_body
            .result
            .context("Cloudflare create DNS response missing result payload")?
            .id)
    }

    async fn find_dns_record(
        &self,
        zone_id: &str,
        name: &str,
        record_type: &str,
    ) -> Result<Option<String>> {
        let url = self.endpoint(&format!("zones/{}/dns_records", zone_id));

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
            .query(&[("type", record_type), ("name", name)])
            .send()
            .await
            .context("Failed to send list DNS records request")?;

        let status = response.status();
        let response_body: ListDnsRecordsResponse =
            Self::parse_json_response(response, "Failed to parse list DNS records response")
                .await?;

        if !response_body.success {
            let error_messages: Vec<String> = response_body
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            anyhow::bail!(
                "Failed to list DNS records (status {}): {}",
                status,
                error_messages.join(", ")
            );
        }

        Ok(response_body
            .result
            .into_iter()
            .next()
            .map(|record| record.id))
    }

    async fn update_dns_record(
        &self,
        zone_id: &str,
        record_id: &str,
        name: &str,
        content: &str,
        record_type: &str,
        proxied: bool,
    ) -> Result<String> {
        let url = self.endpoint(&format!("zones/{}/dns_records/{}", zone_id, record_id));

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
            "ttl": 1,
        });

        let response = self
            .http_client
            .patch(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Failed to send update DNS record request")?;

        let status = response.status();
        let response_body: CreateDnsRecordResponse =
            Self::parse_json_response(response, "Failed to parse update DNS record response")
                .await?;

        if !response_body.success {
            let error_messages: Vec<String> = response_body
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            anyhow::bail!(
                "Failed to update DNS record (status {}): {}",
                status,
                error_messages.join(", ")
            );
        }

        Ok(response_body
            .result
            .context("Cloudflare update DNS response missing result payload")?
            .id)
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

    #[test]
    fn test_parse_create_tunnel_response_with_credentials_file_shape() {
        let raw = r#"{
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": "935949eb-eebc-458f-86cc-de0502e91208",
                "account_tag": "7e0b8efef44f34f5ab894aeb40f60d16",
                "created_at": "2026-03-07T15:20:09.839552Z",
                "name": "acpms-probe-1772896809",
                "credentials_file": {
                    "AccountTag": "7e0b8efef44f34f5ab894aeb40f60d16",
                    "TunnelID": "935949eb-eebc-458f-86cc-de0502e91208",
                    "TunnelName": "acpms-probe-1772896809",
                    "TunnelSecret": "secret-value"
                }
            }
        }"#;

        let parsed: CreateTunnelResponse = serde_json::from_str(raw).unwrap();
        let result = parsed.result.unwrap();
        assert_eq!(result.id, "935949eb-eebc-458f-86cc-de0502e91208");
        assert_eq!(
            result.credentials_file.tunnel_id,
            "935949eb-eebc-458f-86cc-de0502e91208"
        );
        assert_eq!(result.credentials_file.tunnel_secret, "secret-value");
    }
}
