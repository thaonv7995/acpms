use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubUser {
    pub id: u64,
    pub login: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubRepositoryOwner {
    pub id: u64,
    pub login: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct GitHubRepositoryPermissions {
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub maintain: bool,
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub triage: bool,
    #[serde(default)]
    pub pull: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubRepository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
    pub clone_url: String,
    pub default_branch: String,
    pub owner: GitHubRepositoryOwner,
    #[serde(default)]
    pub permissions: Option<GitHubRepositoryPermissions>,
    #[serde(default)]
    pub allow_forking: Option<bool>,
    #[serde(default)]
    pub fork: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PullRequest {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String, // open, closed
    pub html_url: String,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
    #[serde(default)]
    pub merged: Option<bool>,
    #[serde(default)]
    pub merged_at: Option<String>,
    #[serde(default)]
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub mergeable_state: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PullRequestRef {
    pub r#ref: String,
    pub repo: Option<PullRequestRepo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PullRequestRepo {
    pub full_name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CreatePrParams {
    pub title: String,
    pub head: String,
    pub base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}
