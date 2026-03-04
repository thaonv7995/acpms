use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitLabProject {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub path_with_namespace: String,
    pub web_url: String,
    pub default_branch: Option<String>,
    pub ssh_url_to_repo: String,
    pub http_url_to_repo: String,
    #[serde(default)]
    pub permissions: Option<GitLabProjectPermissions>,
    #[serde(default)]
    pub forking_access_level: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitLabUser {
    pub id: u64,
    pub username: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitLabProjectPermissions {
    #[serde(default)]
    pub project_access: Option<GitLabAccessLevel>,
    #[serde(default)]
    pub group_access: Option<GitLabAccessLevel>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitLabAccessLevel {
    pub access_level: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitBranch {
    pub name: String,
    pub commit: Commit,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Commit {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MergeRequest {
    pub id: u64,
    pub iid: u64,
    pub project_id: u64,
    #[serde(default)]
    pub source_project_id: Option<u64>,
    #[serde(default)]
    pub target_project_id: Option<u64>,
    pub title: String,
    pub description: Option<String>,
    pub state: String, // opened, closed, merged
    pub web_url: String,
    pub source_branch: String,
    pub target_branch: String,
    /// GitLab merge status: can_be_merged, cannot_be_merged, unchecked, checking
    #[serde(default)]
    pub merge_status: Option<String>,
    /// GitLab: true when MR has conflicts
    #[serde(default)]
    pub has_conflicts: Option<bool>,
    /// GitLab: ci_still_running, ci_must_pass, mergeable, etc.
    #[serde(default)]
    pub detailed_merge_status: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CreateMrParams {
    pub source_branch: String,
    pub target_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_project_id: Option<u64>,
    pub title: String,
    pub description: Option<String>,
    pub remove_source_branch: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Webhook {
    pub id: u64,
    pub url: String,
    pub push_events: bool,
    pub merge_requests_events: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "object_kind")]
pub enum WebhookEvent {
    #[serde(rename = "push")]
    Push(PushEvent),
    #[serde(rename = "merge_request")]
    MergeRequest(MergeRequestEvent),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PushEvent {
    pub project_id: u64,
    pub before: String,
    pub after: String,
    pub r#ref: String,
    pub user_name: String,
    pub total_commits_count: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MergeRequestEvent {
    pub project: GitLabProject,
    pub object_attributes: MergeRequestAttributes,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MergeRequestAttributes {
    pub id: u64,
    pub iid: u64,
    pub target_branch: String,
    pub source_branch: String,
    pub title: String,
    pub state: String, // opened, closed, merged
    pub url: String,
    pub action: Option<String>, // open, update, close, merge
}
