use crate::types::*;
use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;

#[derive(Clone)]
pub struct GitLabClient {
    base_url: Url,
    client: Client,
}

impl GitLabClient {
    pub fn new(base_url: &str, pat: &str) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "PRIVATE-TOKEN",
            reqwest::header::HeaderValue::from_str(pat).context("Invalid token")?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("Failed to build reqwest client")?;

        let base_url = Url::parse(base_url)
            .context("Invalid base URL")?
            .join("api/v4/")
            .context("Failed to join api path")?;

        Ok(Self { base_url, client })
    }

    pub async fn get_project(&self, project_id: u64) -> Result<GitLabProject> {
        let url = self.base_url.join(&format!("projects/{}", project_id))?;
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to get project: {}", resp.status());
        }

        resp.json()
            .await
            .context("Failed to parse project response")
    }

    pub async fn get_project_by_path(&self, path_with_namespace: &str) -> Result<GitLabProject> {
        let encoded_path: String =
            url::form_urlencoded::byte_serialize(path_with_namespace.as_bytes()).collect();
        let url = self.base_url.join(&format!("projects/{}", encoded_path))?;
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to get project by path: {}", resp.status());
        }

        resp.json()
            .await
            .context("Failed to parse project-by-path response")
    }

    pub async fn get_current_user(&self) -> Result<GitLabUser> {
        let url = self.base_url.join("user")?;
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to get current GitLab user: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse current GitLab user response")
    }

    pub async fn create_fork(&self, project_id: u64) -> Result<GitLabProject> {
        let url = self
            .base_url
            .join(&format!("projects/{}/fork", project_id))?;
        let resp = self.client.post(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to create GitLab fork: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse create GitLab fork response")
    }

    pub async fn delete_project(&self, project_id: u64) -> Result<()> {
        let url = self.base_url.join(&format!("projects/{}", project_id))?;
        let resp = self.client.delete(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to delete GitLab project: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        Ok(())
    }

    pub async fn create_branch(
        &self,
        project_id: u64,
        branch: &str,
        ref_branch: &str,
    ) -> Result<()> {
        let url = self
            .base_url
            .join(&format!("projects/{}/repository/branches", project_id))?;

        let resp = self
            .client
            .post(url)
            .query(&[("branch", branch), ("ref", ref_branch)])
            .send()
            .await?;

        if !resp.status().is_success() {
            // If branch already exists, that's fine? Or error?
            // For now error, caller handles logic
            anyhow::bail!(
                "Failed to create branch: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        Ok(())
    }

    pub async fn create_merge_request(
        &self,
        project_id: u64,
        params: CreateMrParams,
    ) -> Result<MergeRequest> {
        let url = self
            .base_url
            .join(&format!("projects/{}/merge_requests", project_id))?;

        let resp = self.client.post(url).json(&params).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to create MR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json().await.context("Failed to parse MR response")
    }

    /// Get a merge request by IID.
    pub async fn get_merge_request(&self, project_id: u64, mr_iid: u64) -> Result<MergeRequest> {
        let url = self.base_url.join(&format!(
            "projects/{}/merge_requests/{}",
            project_id, mr_iid
        ))?;

        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to get MR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json().await.context("Failed to parse MR response")
    }

    /// Close a merge request by IID (without merging).
    /// GitLab API: PUT /projects/:id/merge_requests/:merge_request_iid
    pub async fn close_merge_request(&self, project_id: u64, mr_iid: u64) -> Result<MergeRequest> {
        let url = self.base_url.join(&format!(
            "projects/{}/merge_requests/{}",
            project_id, mr_iid
        ))?;

        let body = serde_json::json!({ "state_event": "close" });
        let resp = self.client.put(url).json(&body).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to close MR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse close MR response")
    }

    /// Merge a merge request by IID.
    /// GitLab API: PUT /projects/:id/merge_requests/:merge_request_iid/merge
    pub async fn merge_merge_request(
        &self,
        project_id: u64,
        mr_iid: u64,
        remove_source_branch: bool,
    ) -> Result<MergeRequest> {
        let url = self.base_url.join(&format!(
            "projects/{}/merge_requests/{}/merge",
            project_id, mr_iid
        ))?;

        let body = serde_json::json!({
            "should_remove_source_branch": remove_source_branch,
        });

        let resp = self.client.put(url).json(&body).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to merge MR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse merge MR response")
    }

    pub async fn create_webhook(
        &self,
        project_id: u64,
        url: &str,
        secret_token: &str,
    ) -> Result<()> {
        let endpoint = self
            .base_url
            .join(&format!("projects/{}/hooks", project_id))?;

        let params = serde_json::json!({
            "url": url,
            "push_events": true,
            "merge_requests_events": true,
            "token": secret_token,
            "enable_ssl_verification": false // Simplified for dev/preview
        });

        let resp = self.client.post(endpoint).json(&params).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to create webhook: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        Ok(())
    }
}
