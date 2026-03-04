use crate::types::*;
use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;

const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Clone)]
pub struct GitHubClient {
    base_url: Url,
    client: Client,
}

impl GitHubClient {
    /// Create client for GitHub API.
    /// base_url: e.g. "https://github.com" or "https://api.github.com" — we use api.github.com for GitHub.com.
    pub fn new(base_url: &str, pat: &str) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", pat))
                .context("Invalid token")?,
        );
        headers.insert(
            "Accept",
            reqwest::header::HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            reqwest::header::HeaderValue::from_static("2022-11-28"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .user_agent("acpms-github-client/1.0")
            .build()
            .context("Failed to build reqwest client")?;

        let base = base_url.trim().to_lowercase();
        let base_url = if base.contains("github.com") && !base.contains("api.github.com") {
            Url::parse(GITHUB_API_BASE).context("Invalid GitHub API URL")?
        } else {
            Url::parse(base_url)
                .or_else(|_| Url::parse(GITHUB_API_BASE))
                .context("Invalid base URL")?
        };

        Ok(Self { base_url, client })
    }

    /// Create a pull request.
    /// owner_repo: "owner/repo" e.g. "octocat/Hello-World"
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        params: CreatePrParams,
    ) -> Result<PullRequest> {
        let url = self
            .base_url
            .join(&format!("repos/{}/{}/pulls", owner, repo))?;

        let resp = self.client.post(url).json(&params).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to create PR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json().await.context("Failed to parse PR response")
    }

    /// Get the authenticated user for the provided token.
    pub async fn get_authenticated_user(&self) -> Result<GitHubUser> {
        let url = self.base_url.join("user")?;
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to get authenticated user: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse authenticated user response")
    }

    /// Get repository metadata and effective permissions for the authenticated user.
    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<GitHubRepository> {
        let url = self.base_url.join(&format!("repos/{}/{}", owner, repo))?;
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to get repository: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse repository response")
    }

    /// Create a fork under the authenticated user's namespace.
    pub async fn create_fork(&self, owner: &str, repo: &str) -> Result<GitHubRepository> {
        let url = self
            .base_url
            .join(&format!("repos/{}/{}/forks", owner, repo))?;
        let resp = self
            .client
            .post(url)
            .json(&serde_json::json!({}))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to create fork: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse create fork response")
    }

    /// Get a pull request by number.
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<PullRequest> {
        let url = self
            .base_url
            .join(&format!("repos/{}/{}/pulls/{}", owner, repo, pull_number))?;

        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to get PR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json().await.context("Failed to parse PR response")
    }

    /// List pull requests by head branch (owner:branch or repo:branch).
    pub async fn list_pulls_by_head(
        &self,
        owner: &str,
        repo: &str,
        head: &str,
    ) -> Result<Vec<PullRequest>> {
        let url = self
            .base_url
            .join(&format!("repos/{}/{}/pulls", owner, repo))?;

        let resp = self
            .client
            .get(url)
            .query(&[("state", "open"), ("head", head)])
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to list PRs: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse PR list response")
    }

    /// Merge a pull request.
    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<MergeResult> {
        let url = self.base_url.join(&format!(
            "repos/{}/{}/pulls/{}/merge",
            owner, repo, pull_number
        ))?;

        let resp = self.client.put(url).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to merge PR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json().await.context("Failed to parse merge response")
    }

    /// Close a pull request without merging it.
    pub async fn close_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<PullRequest> {
        let url = self
            .base_url
            .join(&format!("repos/{}/{}/pulls/{}", owner, repo, pull_number))?;

        let resp = self
            .client
            .patch(url)
            .json(&serde_json::json!({ "state": "closed" }))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to close PR: {} - {}",
                resp.status(),
                resp.text().await?
            );
        }

        resp.json()
            .await
            .context("Failed to parse close PR response")
    }
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct MergeResult {
    pub sha: Option<String>,
    pub merged: Option<bool>,
    pub message: Option<String>,
}
