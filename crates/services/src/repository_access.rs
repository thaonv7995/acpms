use crate::SystemSettingsService;
use acpms_db::models::{
    RepositoryAccessMode, RepositoryContext, RepositoryProvider, RepositoryVerificationStatus,
};
use anyhow::{Context, Result};
use chrono::Utc;
use std::time::Duration;
use tokio::process::Command;
use url::Url;

const GITLAB_DEVELOPER_ACCESS_LEVEL: u64 = 30;
const GITLAB_MAINTAINER_ACCESS_LEVEL: u64 = 40;

fn github_repository_clone_url(repository: &acpms_github::GitHubRepository) -> String {
    if repository.clone_url.trim().is_empty() {
        repository.html_url.clone()
    } else {
        repository.clone_url.clone()
    }
}

fn gitlab_project_clone_url(project: &acpms_gitlab::GitLabProject) -> String {
    if project.http_url_to_repo.trim().is_empty() {
        project.web_url.clone()
    } else {
        project.http_url_to_repo.clone()
    }
}

#[derive(Clone)]
pub struct RepositoryAccessService {
    settings_service: SystemSettingsService,
}

impl RepositoryAccessService {
    pub fn new(settings_service: SystemSettingsService) -> Self {
        Self { settings_service }
    }

    pub async fn check_cloneable(&self, repo_url: &str) -> Result<(), String> {
        let pat = self
            .settings_service
            .get_pat_for_repo(repo_url)
            .await
            .map_err(|e| format!("Failed to resolve credentials: {}", e))?
            .unwrap_or_default();

        let auth_url = build_authenticated_repo_url(repo_url, &pat);

        let output = tokio::time::timeout(
            Duration::from_secs(30),
            git_command_non_interactive()
                .args(["ls-remote", "--exit-code", &auth_url])
                .output(),
        )
        .await
        .map_err(|_| "Repository check timed out (30s). Check network or repo accessibility.")?
        .map_err(|e| format!("Failed to run git ls-remote: {}", e))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = stderr.trim();
        let err = if msg.is_empty() {
            "Repository not accessible (git ls-remote failed)."
        } else if msg.contains("Authentication failed")
            || msg.contains("fatal: could not read Username")
        {
            "Authentication failed. For private repos, configure PAT in Settings."
        } else if msg.contains("could not resolve host")
            || msg.contains("Name or service not known")
        {
            "Invalid host or DNS resolution failed."
        } else if msg.contains("Repository not found") || msg.contains("404") {
            "Repository not found or you don't have access."
        } else if msg.contains("Permission denied") || msg.contains("publickey") {
            "Permission denied. For SSH URLs use configured keys; for HTTPS use PAT in Settings."
        } else {
            msg
        };

        Err(err.to_string())
    }

    pub async fn check_cloneable_with_retry(
        &self,
        repo_url: &str,
        attempts: usize,
        delay: Duration,
    ) -> Option<String> {
        let attempts = attempts.max(1);

        for attempt_index in 0..attempts {
            match self.check_cloneable(repo_url).await {
                Ok(()) => return None,
                Err(error) if attempt_index + 1 == attempts => return Some(error),
                Err(_) => tokio::time::sleep(delay).await,
            }
        }

        Some("Repository did not become cloneable after retrying.".to_string())
    }

    pub async fn create_fork_repository(&self, repo_url: &str) -> Result<String> {
        let provider = self.detect_provider(repo_url).await;

        match provider {
            RepositoryProvider::Github => self.create_github_fork(repo_url).await,
            RepositoryProvider::Gitlab => self.create_gitlab_fork(repo_url).await,
            RepositoryProvider::Unknown => {
                anyhow::bail!(
                    "Could not infer repository provider from URL or configured instance."
                )
            }
        }
    }

    pub async fn preflight(&self, repo_url: &str, can_clone: bool) -> RepositoryContext {
        let provider = self.detect_provider(repo_url).await;

        let result = match provider {
            RepositoryProvider::Github => self.preflight_github(repo_url, can_clone).await,
            RepositoryProvider::Gitlab => self.preflight_gitlab(repo_url, can_clone).await,
            RepositoryProvider::Unknown => Ok(Self::unknown_context(
                provider,
                repo_url,
                can_clone,
                "Could not infer repository provider from URL or configured instance.",
            )),
        };

        result.unwrap_or_else(|error| {
            Self::failed_context(provider, repo_url, can_clone, error.to_string())
        })
    }

    async fn preflight_github(&self, repo_url: &str, can_clone: bool) -> Result<RepositoryContext> {
        let settings = self
            .settings_service
            .get()
            .await
            .context("Failed to load configured GitHub settings")?;
        let pat = self
            .settings_service
            .get_pat_for_repo(repo_url)
            .await
            .context("Failed to resolve GitHub credentials")?
            .unwrap_or_default();

        if pat.trim().is_empty() {
            return Ok(Self::unauthenticated_context(
                RepositoryProvider::Github,
                repo_url,
                can_clone,
                "No GitHub token configured for this repository host. Capability cannot be verified.",
            ));
        }

        let (owner, repo) = parse_owner_repo(repo_url)
            .ok_or_else(|| anyhow::anyhow!("Invalid GitHub repository URL"))?;
        let client_base_url = if parse_host(&settings.gitlab_url) == parse_host(repo_url) {
            settings.gitlab_url.as_str()
        } else {
            "https://github.com"
        };

        let client = acpms_github::GitHubClient::new(client_base_url, &pat)
            .context("Failed to initialize GitHub client")?;
        let repository = client
            .get_repo(&owner, &repo)
            .await
            .context("Failed to fetch GitHub repository metadata")?;

        let clone_url = github_repository_clone_url(&repository);
        let permissions = repository.permissions.clone().unwrap_or_default();
        let can_push = permissions.push || permissions.maintain || permissions.admin;
        let can_open_change_request = can_push;
        let can_merge = permissions.push || permissions.maintain || permissions.admin;
        let can_manage_webhooks = permissions.admin || permissions.maintain;
        let can_fork = repository.allow_forking.unwrap_or(!repository.private);
        let access_mode = if can_push {
            RepositoryAccessMode::DirectGitops
        } else {
            RepositoryAccessMode::AnalysisOnly
        };
        let writable_repository_url = can_push.then(|| clone_url.clone());

        Ok(RepositoryContext {
            provider: RepositoryProvider::Github,
            access_mode,
            verification_status: RepositoryVerificationStatus::Verified,
            verification_error: None,
            can_clone,
            can_push,
            can_open_change_request,
            can_merge,
            can_manage_webhooks,
            can_fork,
            upstream_repository_url: Some(clone_url.clone()),
            writable_repository_url: writable_repository_url.clone(),
            effective_clone_url: writable_repository_url.or_else(|| Some(clone_url)),
            default_branch: Some(repository.default_branch),
            upstream_project_id: Some(repository.id as i64),
            writable_project_id: can_push.then_some(repository.id as i64),
            verified_at: Some(Utc::now()),
        })
    }

    async fn create_github_fork(&self, repo_url: &str) -> Result<String> {
        let settings = self
            .settings_service
            .get()
            .await
            .context("Failed to load configured GitHub settings")?;
        let pat = self
            .settings_service
            .get_pat_for_repo(repo_url)
            .await
            .context("Failed to resolve GitHub credentials")?
            .unwrap_or_default();

        if pat.trim().is_empty() {
            anyhow::bail!("No GitHub token configured for this repository host.");
        }

        let (owner, repo) = parse_owner_repo(repo_url)
            .ok_or_else(|| anyhow::anyhow!("Invalid GitHub repository URL"))?;
        let client_base_url = if parse_host(&settings.gitlab_url) == parse_host(repo_url) {
            settings.gitlab_url.as_str()
        } else {
            "https://github.com"
        };

        let client = acpms_github::GitHubClient::new(client_base_url, &pat)
            .context("Failed to initialize GitHub client")?;
        let upstream = client
            .get_repo(&owner, &repo)
            .await
            .context("Failed to fetch GitHub repository metadata")?;
        if !upstream.allow_forking.unwrap_or(!upstream.private) {
            anyhow::bail!("GitHub repository does not allow forking.");
        }

        let current_user = client
            .get_authenticated_user()
            .await
            .context("Failed to resolve authenticated GitHub user")?;

        match client.create_fork(&owner, &repo).await {
            Ok(fork) => Ok(github_repository_clone_url(&fork)),
            Err(error) => {
                let error_text = error.to_string();
                if error_text.contains("422") || error_text.contains("already exists") {
                    let existing_fork = client.get_repo(&current_user.login, &repo).await.context(
                        "Fork already exists, but the existing GitHub fork could not be loaded",
                    )?;
                    if existing_fork.fork {
                        Ok(github_repository_clone_url(&existing_fork))
                    } else {
                        Err(error)
                    }
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn preflight_gitlab(&self, repo_url: &str, can_clone: bool) -> Result<RepositoryContext> {
        let settings = self
            .settings_service
            .get()
            .await
            .context("Failed to load configured GitLab settings")?;
        let pat = self
            .settings_service
            .get_pat_for_repo(repo_url)
            .await
            .context("Failed to resolve GitLab credentials")?
            .unwrap_or_default();

        if pat.trim().is_empty() {
            return Ok(Self::unauthenticated_context(
                RepositoryProvider::Gitlab,
                repo_url,
                can_clone,
                "No GitLab token configured for this repository host. Capability cannot be verified.",
            ));
        }

        let repo_path = parse_path_with_namespace(repo_url)
            .ok_or_else(|| anyhow::anyhow!("Invalid GitLab repository URL"))?;
        let client = acpms_gitlab::GitLabClient::new(&settings.gitlab_url, &pat)
            .context("Failed to initialize GitLab client")?;
        let project = client
            .get_project_by_path(&repo_path)
            .await
            .context("Failed to fetch GitLab project metadata")?;

        let project_access = project
            .permissions
            .as_ref()
            .and_then(|permissions| permissions.project_access.as_ref())
            .map(|access| access.access_level)
            .unwrap_or(0);
        let group_access = project
            .permissions
            .as_ref()
            .and_then(|permissions| permissions.group_access.as_ref())
            .map(|access| access.access_level)
            .unwrap_or(0);
        let access_level = project_access.max(group_access);

        let can_push = access_level >= GITLAB_DEVELOPER_ACCESS_LEVEL;
        let can_open_change_request = can_push;
        let can_merge = access_level >= GITLAB_DEVELOPER_ACCESS_LEVEL;
        let can_manage_webhooks = access_level >= GITLAB_MAINTAINER_ACCESS_LEVEL;
        let can_fork = !matches!(project.forking_access_level.as_deref(), Some("disabled"));
        let access_mode = if can_push {
            RepositoryAccessMode::DirectGitops
        } else {
            RepositoryAccessMode::AnalysisOnly
        };
        let clone_url = gitlab_project_clone_url(&project);
        let writable_repository_url = can_push.then(|| clone_url.clone());

        Ok(RepositoryContext {
            provider: RepositoryProvider::Gitlab,
            access_mode,
            verification_status: RepositoryVerificationStatus::Verified,
            verification_error: None,
            can_clone,
            can_push,
            can_open_change_request,
            can_merge,
            can_manage_webhooks,
            can_fork,
            upstream_repository_url: Some(clone_url.clone()),
            writable_repository_url: writable_repository_url.clone(),
            effective_clone_url: writable_repository_url.or_else(|| Some(clone_url)),
            default_branch: project.default_branch,
            upstream_project_id: Some(project.id as i64),
            writable_project_id: can_push.then_some(project.id as i64),
            verified_at: Some(Utc::now()),
        })
    }

    async fn create_gitlab_fork(&self, repo_url: &str) -> Result<String> {
        let settings = self
            .settings_service
            .get()
            .await
            .context("Failed to load configured GitLab settings")?;
        let pat = self
            .settings_service
            .get_pat_for_repo(repo_url)
            .await
            .context("Failed to resolve GitLab credentials")?
            .unwrap_or_default();

        if pat.trim().is_empty() {
            anyhow::bail!("No GitLab token configured for this repository host.");
        }

        let repo_path = parse_path_with_namespace(repo_url)
            .ok_or_else(|| anyhow::anyhow!("Invalid GitLab repository URL"))?;
        let client = acpms_gitlab::GitLabClient::new(&settings.gitlab_url, &pat)
            .context("Failed to initialize GitLab client")?;
        let project = client
            .get_project_by_path(&repo_path)
            .await
            .context("Failed to fetch GitLab project metadata")?;

        if matches!(project.forking_access_level.as_deref(), Some("disabled")) {
            anyhow::bail!("GitLab project does not allow forking.");
        }

        let current_user = client
            .get_current_user()
            .await
            .context("Failed to resolve authenticated GitLab user")?;

        match client.create_fork(project.id).await {
            Ok(fork) => Ok(gitlab_project_clone_url(&fork)),
            Err(error) => {
                let error_text = error.to_string();
                if error_text.contains("409") || error_text.contains("already exists") {
                    let existing_path = format!("{}/{}", current_user.username, project.path);
                    let existing_fork = client.get_project_by_path(&existing_path).await.context(
                        "Fork already exists, but the existing GitLab fork could not be loaded",
                    )?;
                    Ok(gitlab_project_clone_url(&existing_fork))
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn detect_provider(&self, repo_url: &str) -> RepositoryProvider {
        let host = parse_host(repo_url);
        let configured = self.settings_service.get().await.ok().map(|settings| {
            let configured_host = parse_host(&settings.gitlab_url);
            let configured_url = settings.gitlab_url.to_ascii_lowercase();
            (configured_host, configured_url)
        });

        if let Some(host) = host {
            let host_lower = host.to_ascii_lowercase();
            if host_lower.contains("github") {
                return RepositoryProvider::Github;
            }
            if host_lower.contains("gitlab") {
                return RepositoryProvider::Gitlab;
            }

            if configured
                .as_ref()
                .and_then(|(configured_host, configured_url)| {
                    configured_host
                        .as_ref()
                        .filter(|configured_host| configured_host.eq_ignore_ascii_case(&host_lower))
                        .map(|_| configured_url)
                })
                .is_some()
            {
                if configured
                    .as_ref()
                    .map(|(_, configured_url)| configured_url.contains("github"))
                    .unwrap_or(false)
                {
                    return RepositoryProvider::Github;
                }

                if host_lower.contains("github") {
                    return RepositoryProvider::Github;
                }

                return RepositoryProvider::Gitlab;
            }
        }

        RepositoryProvider::Unknown
    }

    fn unauthenticated_context(
        provider: RepositoryProvider,
        repo_url: &str,
        can_clone: bool,
        error: impl Into<String>,
    ) -> RepositoryContext {
        RepositoryContext {
            provider,
            access_mode: RepositoryAccessMode::Unknown,
            verification_status: RepositoryVerificationStatus::Unauthenticated,
            verification_error: Some(error.into()),
            can_clone,
            can_push: false,
            can_open_change_request: false,
            can_merge: false,
            can_manage_webhooks: false,
            can_fork: false,
            upstream_repository_url: Some(repo_url.to_string()),
            writable_repository_url: None,
            effective_clone_url: Some(repo_url.to_string()),
            default_branch: None,
            upstream_project_id: None,
            writable_project_id: None,
            verified_at: None,
        }
    }

    fn failed_context(
        provider: RepositoryProvider,
        repo_url: &str,
        can_clone: bool,
        error: impl Into<String>,
    ) -> RepositoryContext {
        RepositoryContext {
            provider,
            access_mode: RepositoryAccessMode::Unknown,
            verification_status: RepositoryVerificationStatus::Failed,
            verification_error: Some(error.into()),
            can_clone,
            can_push: false,
            can_open_change_request: false,
            can_merge: false,
            can_manage_webhooks: false,
            can_fork: false,
            upstream_repository_url: Some(repo_url.to_string()),
            writable_repository_url: None,
            effective_clone_url: Some(repo_url.to_string()),
            default_branch: None,
            upstream_project_id: None,
            writable_project_id: None,
            verified_at: Some(Utc::now()),
        }
    }

    fn unknown_context(
        provider: RepositoryProvider,
        repo_url: &str,
        can_clone: bool,
        error: impl Into<String>,
    ) -> RepositoryContext {
        RepositoryContext {
            provider,
            access_mode: RepositoryAccessMode::Unknown,
            verification_status: RepositoryVerificationStatus::Unknown,
            verification_error: Some(error.into()),
            can_clone,
            can_push: false,
            can_open_change_request: false,
            can_merge: false,
            can_manage_webhooks: false,
            can_fork: false,
            upstream_repository_url: Some(repo_url.to_string()),
            writable_repository_url: None,
            effective_clone_url: Some(repo_url.to_string()),
            default_branch: None,
            upstream_project_id: None,
            writable_project_id: None,
            verified_at: None,
        }
    }
}

fn parse_host(input: &str) -> Option<String> {
    if let Ok(parsed) = Url::parse(input.trim()) {
        return parsed.host_str().map(|host| host.to_ascii_lowercase());
    }

    let with_scheme = format!("https://{}", input.trim().trim_start_matches('/'));
    Url::parse(&with_scheme)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()))
}

fn git_command_non_interactive() -> Command {
    let mut cmd = Command::new("git");
    // Repository checks run on API/executor side and must never wait on interactive prompts.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GCM_INTERACTIVE", "Never");
    cmd.env("GIT_ASKPASS", "echo");
    cmd.env("SSH_ASKPASS", "echo");
    cmd.env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes");
    cmd
}

fn parse_repo_host_and_path(repo_url: &str) -> Option<(String, String)> {
    let trimmed = repo_url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        let without_auth = rest.rsplit('@').next().unwrap_or(rest);
        let (host, path) = without_auth.split_once('/')?;
        let host = host.trim().to_ascii_lowercase();
        let path = path.trim().trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path).to_string();
        if host.is_empty() || path.is_empty() {
            return None;
        }
        return Some((host, path));
    }

    if let Some((left, right)) = trimmed.split_once(':') {
        if let Some(host) = left.split('@').nth(1) {
            let host = host.trim().to_ascii_lowercase();
            let path = right.trim().trim_matches('/');
            let path = path.strip_suffix(".git").unwrap_or(path).to_string();
            if host.is_empty() || path.is_empty() {
                return None;
            }
            return Some((host, path));
        }
    }

    None
}

fn build_authenticated_repo_url(repo_url: &str, pat: &str) -> String {
    if pat.trim().is_empty() {
        return repo_url.to_string();
    }

    let normalized = if repo_url.starts_with("https://") || repo_url.starts_with("http://") {
        repo_url.trim().to_string()
    } else if let Some((host, path)) = parse_repo_host_and_path(repo_url) {
        format!("https://{}/{}.git", host, path)
    } else {
        return repo_url.to_string();
    };

    let username = parse_repo_host_and_path(&normalized)
        .map(|(host, _)| {
            if host.contains("github") {
                "x-access-token"
            } else {
                "oauth2"
            }
        })
        .unwrap_or("oauth2");

    if let Some(rest) = normalized.strip_prefix("https://") {
        format!("https://{}:{}@{}", username, pat, rest)
    } else if let Some(rest) = normalized.strip_prefix("http://") {
        format!("http://{}:{}@{}", username, pat, rest)
    } else {
        normalized
    }
}

fn parse_owner_repo(repo_url: &str) -> Option<(String, String)> {
    let parsed = Url::parse(repo_url).ok()?;
    let mut segments = parsed.path_segments()?;
    let owner = segments.next()?.trim().to_string();
    let repo = segments.next()?.trim().trim_end_matches(".git").to_string();

    if owner.is_empty() || repo.is_empty() {
        None
    } else {
        Some((owner, repo))
    }
}

fn parse_path_with_namespace(repo_url: &str) -> Option<String> {
    let parsed = Url::parse(repo_url).ok()?;
    let path = parsed.path().trim_start_matches('/').trim_end_matches('/');
    let path = path.trim_end_matches(".git").trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_authenticated_repo_url, github_repository_clone_url, gitlab_project_clone_url,
        parse_owner_repo, parse_path_with_namespace, parse_repo_host_and_path,
    };
    use acpms_github::{GitHubRepository, GitHubRepositoryOwner};
    use acpms_gitlab::GitLabProject;

    #[test]
    fn parse_owner_repo_handles_git_suffix() {
        let parsed = parse_owner_repo("https://github.com/openai/codex.git");
        assert_eq!(parsed, Some(("openai".to_string(), "codex".to_string())));
    }

    #[test]
    fn parse_path_with_namespace_preserves_nested_gitlab_path() {
        let parsed =
            parse_path_with_namespace("https://gitlab.example.com/group/subgroup/repo.git");
        assert_eq!(parsed, Some("group/subgroup/repo".to_string()));
    }

    #[test]
    fn parse_repo_host_and_path_supports_ssh() {
        let parsed = parse_repo_host_and_path("git@gitlab.example.com:group/sub/repo.git");
        assert_eq!(
            parsed,
            Some((
                "gitlab.example.com".to_string(),
                "group/sub/repo".to_string()
            ))
        );
    }

    #[test]
    fn build_authenticated_repo_url_supports_ssh_repo() {
        let url =
            build_authenticated_repo_url("git@gitlab.example.com:group/repo.git", "glpat-123");
        assert_eq!(
            url,
            "https://oauth2:glpat-123@gitlab.example.com/group/repo.git"
        );
    }

    #[test]
    fn build_authenticated_repo_url_uses_github_username_for_pat() {
        let url = build_authenticated_repo_url("https://github.com/openai/codex.git", "ghp_123");
        assert_eq!(
            url,
            "https://x-access-token:ghp_123@github.com/openai/codex.git"
        );
    }

    #[test]
    fn github_repository_clone_url_prefers_clone_url() {
        let repository = GitHubRepository {
            id: 1,
            name: "codex".to_string(),
            full_name: "openai/codex".to_string(),
            private: false,
            html_url: "https://github.com/openai/codex".to_string(),
            clone_url: "https://github.com/openai/codex.git".to_string(),
            default_branch: "main".to_string(),
            owner: GitHubRepositoryOwner {
                id: 1,
                login: "openai".to_string(),
            },
            permissions: None,
            allow_forking: Some(true),
            fork: false,
        };

        assert_eq!(
            github_repository_clone_url(&repository),
            "https://github.com/openai/codex.git"
        );
    }

    #[test]
    fn gitlab_project_clone_url_prefers_http_clone_url() {
        let project = GitLabProject {
            id: 1,
            name: "repo".to_string(),
            path: "repo".to_string(),
            path_with_namespace: "group/repo".to_string(),
            web_url: "https://gitlab.example.com/group/repo".to_string(),
            default_branch: Some("main".to_string()),
            ssh_url_to_repo: "git@gitlab.example.com:group/repo.git".to_string(),
            http_url_to_repo: "https://gitlab.example.com/group/repo.git".to_string(),
            permissions: None,
            forking_access_level: Some("enabled".to_string()),
        };

        assert_eq!(
            gitlab_project_clone_url(&project),
            "https://gitlab.example.com/group/repo.git"
        );
    }
}
