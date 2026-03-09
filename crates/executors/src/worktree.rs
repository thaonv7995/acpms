use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Result of creating a worktree, including branch metadata for diff computation.
pub struct WorktreeInfo {
    /// Path to the created worktree directory.
    pub path: PathBuf,
    /// The base branch the worktree was created from (e.g., "main", "master").
    pub base_branch: String,
    /// The feature branch name created for this worktree.
    pub feature_branch: String,
}

pub struct WorktreeManager {
    base_path: Arc<RwLock<PathBuf>>,
}

pub(crate) fn summarize_repository_source(repo_url: &str) -> String {
    let trimmed = repo_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "the configured source".to_string();
    }

    let segment = trimmed
        .rsplit(['/', ':'])
        .find(|part| !part.is_empty())
        .unwrap_or(trimmed)
        .trim_end_matches(".git")
        .trim();

    if segment.is_empty() {
        "the configured source".to_string()
    } else {
        segment.to_string()
    }
}

pub(crate) fn format_repository_sync_log(repo_url: &str) -> String {
    format!(
        "Repository is already available locally. Syncing latest changes from {}.",
        summarize_repository_source(repo_url)
    )
}

pub(crate) fn format_repository_clone_log(repo_url: &str) -> String {
    format!(
        "Preparing a fresh local repository copy from {}.",
        summarize_repository_source(repo_url)
    )
}

impl WorktreeManager {
    pub fn new(base_path: Arc<RwLock<PathBuf>>) -> Self {
        Self { base_path }
    }

    /// Returns the current base path for worktrees (read from shared state, applies immediately).
    pub async fn base_path(&self) -> PathBuf {
        self.base_path.read().await.clone()
    }

    /// Ensures the base repository is cloned. If not, clones it using the provided credentials.
    pub async fn ensure_repo_cloned(&self, _repo_url: &str, _pat: &str) -> Result<PathBuf> {
        // We assume the repo name is the last part of the URL, or we just map it to a specific directory.
        // For simplicity, let's say the base_path IS the worktrees root,
        // and we need a "main" repo directory to hold the bare repo or the main worktree.

        // Actually, Orchestrator passes `repo_path`. Let's assume that's where we want the main repo.
        // But `repo_path` is derived in Orchestrator.
        // Let's change signature to accept `repo_path`.

        // Wait, self.base_path is where worktrees go?
        // In `create_worktree`: `worktree_path = self.base_path.join(subdir)`.
        // `repo_path` argument is where `git worktree add` is run FROM.

        // So `repo_path` is the "Main" repository.
        Ok(PathBuf::from(""))
    }

    /// Ensures the repository is cloned and up-to-date.
    ///
    /// ## Behavior
    /// - If repo exists: `git fetch` + `git pull` to sync with remote
    /// - If repo not exists: `git clone`
    ///
    /// This ensures each worktree is created from the latest code.
    pub async fn ensure_cloned(&self, repo_path: &Path, remote_url: &str, pat: &str) -> Result<()> {
        self.ensure_cloned_with_upstream(repo_path, remote_url, None, pat)
            .await
    }

    /// Clone-only: ensures the repo directory exists on disk.
    /// If `.git` already exists, returns immediately (no fetch/pull).
    /// Used by the Project Assistant where local code is sufficient.
    pub async fn ensure_repo_exists(
        &self,
        repo_path: &Path,
        remote_url: &str,
        upstream_url: Option<&str>,
        pat: &str,
    ) -> Result<()> {
        if repo_path.join(".git").exists() {
            return Ok(());
        }

        // Not cloned yet — do the initial clone.
        let auth_url = inject_pat_into_url(remote_url, pat);
        let upstream_auth_url = upstream_url.map(|url| inject_pat_into_url(url, pat));

        tracing::info!(
            "Repository not found at {:?}, cloning from {}",
            repo_path,
            remote_url
        );

        if let Some(parent) = repo_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let output = git_command_non_interactive()
            .arg("clone")
            .arg(&auth_url)
            .arg(repo_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = if !pat.is_empty() {
                stderr.replace(pat, "***PAT***")
            } else {
                stderr.to_string()
            };
            anyhow::bail!("git clone failed: {}", sanitized);
        }

        // Add upstream remote if specified.
        self.ensure_remote_url(
            repo_path,
            "upstream",
            upstream_url,
            upstream_auth_url.as_deref(),
        )
        .await?;

        tracing::info!("Repository cloned successfully");
        Ok(())
    }

    pub async fn ensure_cloned_with_upstream(
        &self,
        repo_path: &Path,
        remote_url: &str,
        upstream_url: Option<&str>,
        pat: &str,
    ) -> Result<()> {
        let auth_url = inject_pat_into_url(remote_url, pat);
        let upstream_auth_url = upstream_url.map(|url| inject_pat_into_url(url, pat));

        if repo_path.join(".git").exists() {
            // Repository exists — check if a recent sync already happened.
            // If FETCH_HEAD was updated within the last 60 seconds we can skip the
            // expensive `git fetch --all` + `git pull` round-trip entirely, which
            // saves 5-15s of network I/O on every assistant session start.
            const REPO_SYNC_FRESHNESS_SECS: u64 = 60;
            let fetch_head = repo_path.join(".git").join("FETCH_HEAD");
            let recently_synced = fetch_head
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|modified| {
                    modified.elapsed().unwrap_or_default()
                        < std::time::Duration::from_secs(REPO_SYNC_FRESHNESS_SECS)
                })
                .unwrap_or(false);

            if recently_synced {
                tracing::info!(
                    "Repository at {:?} was synced within the last {}s, skipping fetch/pull",
                    repo_path,
                    REPO_SYNC_FRESHNESS_SECS
                );
                return Ok(());
            }

            tracing::info!(
                "Repository exists at {:?}, pulling latest changes",
                repo_path
            );

            // Guard against path collision: existing repo must match expected remote.
            let current_remote_output = Command::new("git")
                .current_dir(repo_path)
                .arg("remote")
                .arg("get-url")
                .arg("origin")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to read current git remote URL")?;

            if !current_remote_output.status.success() {
                let stderr = String::from_utf8_lossy(&current_remote_output.stderr);
                anyhow::bail!("git remote get-url origin failed: {}", stderr.trim());
            }

            let current_remote = String::from_utf8_lossy(&current_remote_output.stdout)
                .trim()
                .to_string();
            if !remote_url_matches_configured_value(&current_remote, &auth_url) {
                tracing::info!(
                    "Repository remote changed at {:?}: retargeting origin from {} to {}",
                    repo_path,
                    current_remote,
                    auth_url
                );
                let set_origin_output = Command::new("git")
                    .current_dir(repo_path)
                    .args(["remote", "set-url", "origin", &auth_url])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .context("Failed to retarget git origin URL")?;

                if !set_origin_output.status.success() {
                    let stderr = String::from_utf8_lossy(&set_origin_output.stderr);
                    anyhow::bail!("git remote set-url origin failed: {}", stderr.trim());
                }
            }

            self.ensure_remote_url(
                repo_path,
                "upstream",
                upstream_url,
                upstream_auth_url.as_deref(),
            )
            .await?;

            // Fetch all remotes
            let fetch_output = git_command_non_interactive()
                .current_dir(repo_path)
                .arg("fetch")
                .arg("--all")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to execute git fetch")?;

            if !fetch_output.status.success() {
                let stderr = String::from_utf8_lossy(&fetch_output.stderr);
                tracing::warn!("git fetch warning: {}", stderr);
                // Don't fail on fetch warning, continue to pull
            }

            // Detect default branch on the remote we intend to sync from.
            let sync_remote = upstream_url.map(|_| "upstream").unwrap_or("origin");
            let default_branch = self
                .detect_default_branch(repo_path, sync_remote)
                .await
                .unwrap_or_else(|_| format!("{}/main", sync_remote));
            let (pull_remote, branch_name) = parse_tracking_branch(&default_branch, sync_remote);

            // Checkout the default branch first (in case we're on a detached HEAD or different branch)
            let checkout_output = Command::new("git")
                .current_dir(repo_path)
                .arg("checkout")
                .arg(&branch_name)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to execute git checkout")?;

            if !checkout_output.status.success() {
                let stderr = String::from_utf8_lossy(&checkout_output.stderr);
                tracing::warn!("git checkout warning: {}", stderr);
                // Continue anyway - might be on correct branch already
            }

            // Pull latest changes
            let pull_output = git_command_non_interactive()
                .current_dir(repo_path)
                .arg("pull")
                .arg(&pull_remote)
                .arg(&branch_name)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to execute git pull")?;

            if !pull_output.status.success() {
                let stderr = String::from_utf8_lossy(&pull_output.stderr);
                // Check if it's just "already up to date" or a real error
                if !stderr.contains("Already up to date") && !stderr.contains("Already up-to-date")
                {
                    anyhow::bail!("git pull failed: {}", stderr);
                }
            }

            tracing::info!("Repository synced successfully");
            return Ok(());
        }

        // Repository doesn't exist - clone it
        tracing::info!(
            "Repository not found at {:?}, cloning from {}",
            repo_path,
            remote_url
        );

        // Ensure parent dir exists
        if let Some(parent) = repo_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let output = git_command_non_interactive()
            .arg("clone")
            .arg(&auth_url)
            .arg(repo_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Sanitize PAT from error message
            let sanitized_stderr = if !pat.is_empty() {
                stderr.replace(pat, "***PAT***")
            } else {
                stderr.to_string()
            };
            anyhow::bail!("git clone failed: {}", sanitized_stderr);
        }

        self.ensure_remote_url(
            repo_path,
            "upstream",
            upstream_url,
            upstream_auth_url.as_deref(),
        )
        .await?;

        tracing::info!("Repository cloned successfully");
        Ok(())
    }

    /// Creates a new isolated worktree for a task attempt
    pub async fn create_worktree(
        &self,
        repo_path: &Path,
        attempt_id: Uuid,
        base_ref_override: Option<&str>,
    ) -> Result<WorktreeInfo> {
        let worktree_dir_name = format!("attempt-{}", attempt_id);
        let base = self.base_path().await;
        let worktree_path = base.join(&worktree_dir_name);

        // Ensure base path exists
        tokio::fs::create_dir_all(&base)
            .await
            .context("Failed to create worktrees base directory")?;

        // Best-effort prune stale worktree metadata before creating or reattaching.
        let _ = Command::new("git")
            .current_dir(repo_path)
            .arg("worktree")
            .arg("prune")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        // Fallback: If repo_path exists but is not a git repo, initialize it
        if repo_path.exists() && !repo_path.join(".git").exists() {
            tracing::info!("Initializing fallback git repository at {:?}", repo_path);
            let init_output = Command::new("git")
                .current_dir(repo_path)
                .arg("init")
                .arg("-b")
                .arg("main")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to execute git init fallback")?;

            if !init_output.status.success() {
                let stderr = String::from_utf8_lossy(&init_output.stderr);
                anyhow::bail!("Fallback git init failed: {}", stderr);
            }

            self.setup_git_config(repo_path).await.unwrap_or_else(|e| {
                tracing::warn!("Failed to setup git config for fallback repo: {}", e);
            });

            let commit_output = Command::new("git")
                .current_dir(repo_path)
                .arg("commit")
                .arg("--allow-empty")
                .arg("-m")
                .arg("Initial commit from Orchestrator Fallback")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to execute fallback initial commit")?;

            if !commit_output.status.success() {
                let stderr = String::from_utf8_lossy(&commit_output.stderr);
                tracing::warn!("Fallback initial commit failed: {}", stderr);
            }
        }

        // Branch name for this attempt
        let branch_name = format!("feat/attempt-{}", attempt_id);

        // Detect default branch (main, master, or other)
        let default_branch = if let Some(base_ref) = base_ref_override {
            base_ref.to_string()
        } else {
            self.detect_default_branch(repo_path, "origin")
                .await
                .unwrap_or_else(|_| "HEAD".to_string())
        };

        if tokio::fs::try_exists(&worktree_path).await.unwrap_or(false) {
            let has_git_dir = tokio::fs::try_exists(&worktree_path.join(".git"))
                .await
                .unwrap_or(false);
            if has_git_dir {
                self.setup_git_config(&worktree_path).await?;
                return Ok(WorktreeInfo {
                    path: worktree_path,
                    base_branch: default_branch,
                    feature_branch: branch_name,
                });
            }

            tokio::fs::remove_dir_all(&worktree_path)
                .await
                .with_context(|| {
                    format!(
                        "Failed to remove stale worktree directory before recreation: {:?}",
                        worktree_path
                    )
                })?;
        }

        let branch_exists = self.branch_exists(repo_path, &branch_name).await?;
        let mut output = if branch_exists {
            tracing::info!(
                "Feature branch {} already exists; reattaching worktree at {:?}",
                branch_name,
                worktree_path
            );
            Command::new("git")
                .current_dir(repo_path)
                .arg("worktree")
                .arg("add")
                .arg("--force")
                .arg(&worktree_path)
                .arg(&branch_name)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to execute git worktree command")?
        } else {
            Command::new("git")
                .current_dir(repo_path)
                .arg("worktree")
                .arg("add")
                .arg("-b")
                .arg(&branch_name)
                .arg(&worktree_path)
                .arg(&default_branch)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("Failed to execute git worktree command")?
        };

        if !output.status.success() && !branch_exists {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already exists")
                && self
                    .branch_exists(repo_path, &branch_name)
                    .await
                    .unwrap_or(false)
            {
                tracing::info!(
                    "Feature branch {} appeared during worktree creation; retrying by reusing it",
                    branch_name
                );
                output = Command::new("git")
                    .current_dir(repo_path)
                    .arg("worktree")
                    .arg("add")
                    .arg("--force")
                    .arg(&worktree_path)
                    .arg(&branch_name)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .context("Failed to retry git worktree command")?;
            }
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git worktree add failed: {}", stderr);
        }

        // Setup git config for commits (agent identity)
        self.setup_git_config(&worktree_path).await?;

        Ok(WorktreeInfo {
            path: worktree_path,
            base_branch: default_branch,
            feature_branch: branch_name,
        })
    }

    /// Setup git config for agent commits
    pub async fn setup_git_config(&self, worktree_path: &Path) -> Result<()> {
        // Set user.name
        let name_output = Command::new("git")
            .current_dir(worktree_path)
            .arg("config")
            .arg("user.name")
            .arg("ACPMS Agent")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to set git user.name")?;

        if !name_output.status.success() {
            let stderr = String::from_utf8_lossy(&name_output.stderr);
            anyhow::bail!("git config user.name failed: {}", stderr);
        }

        // Set user.email
        let email_output = Command::new("git")
            .current_dir(worktree_path)
            .arg("config")
            .arg("user.email")
            .arg("agent@acpms.local")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to set git user.email")?;

        if !email_output.status.success() {
            let stderr = String::from_utf8_lossy(&email_output.stderr);
            anyhow::bail!("git config user.email failed: {}", stderr);
        }

        Ok(())
    }

    /// Detect the default branch of the repository (e.g., main, master)
    async fn detect_default_branch(&self, repo_path: &Path, remote_name: &str) -> Result<String> {
        // Try to get the remote HEAD reference
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("symbolic-ref")
            .arg(format!("refs/remotes/{}/HEAD", remote_name))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if output.status.success() {
            let ref_name = String::from_utf8_lossy(&output.stdout);
            // e.g., "refs/remotes/origin/main" -> "origin/main"
            if let Some(branch) = ref_name.trim().strip_prefix("refs/remotes/") {
                return Ok(branch.to_string());
            }
        }

        // Fallback: check for common branch names
        for branch in &[
            format!("{}/main", remote_name),
            format!("{}/master", remote_name),
            "origin/main".to_string(),
            "origin/master".to_string(),
        ] {
            let check = Command::new("git")
                .current_dir(repo_path)
                .arg("rev-parse")
                .arg("--verify")
                .arg(branch)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?;

            if check.status.success() {
                return Ok(branch.to_string());
            }
        }

        // Last resort: use HEAD
        Ok("HEAD".to_string())
    }

    async fn ensure_remote_url(
        &self,
        repo_path: &Path,
        remote_name: &str,
        expected_url: Option<&str>,
        configured_url: Option<&str>,
    ) -> Result<()> {
        let Some(expected_url) = expected_url else {
            return Ok(());
        };
        let configured_url = configured_url.unwrap_or(expected_url);

        let get_output = Command::new("git")
            .current_dir(repo_path)
            .args(["remote", "get-url", remote_name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to inspect git remote {}", remote_name))?;

        if get_output.status.success() {
            let current_remote = String::from_utf8_lossy(&get_output.stdout)
                .trim()
                .to_string();
            if remote_url_matches_configured_value(&current_remote, configured_url) {
                return Ok(());
            }

            let set_output = Command::new("git")
                .current_dir(repo_path)
                .args(["remote", "set-url", remote_name, configured_url])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .with_context(|| format!("Failed to update git remote {}", remote_name))?;

            if !set_output.status.success() {
                let stderr = String::from_utf8_lossy(&set_output.stderr);
                anyhow::bail!(
                    "git remote set-url {} failed: {}",
                    remote_name,
                    stderr.trim()
                );
            }
        } else {
            let add_output = Command::new("git")
                .current_dir(repo_path)
                .args(["remote", "add", remote_name, configured_url])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .with_context(|| format!("Failed to add git remote {}", remote_name))?;

            if !add_output.status.success() {
                let stderr = String::from_utf8_lossy(&add_output.stderr);
                anyhow::bail!("git remote add {} failed: {}", remote_name, stderr.trim());
            }
        }

        let fetch_output = git_command_non_interactive()
            .current_dir(repo_path)
            .args(["fetch", remote_name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to fetch git remote {}", remote_name))?;

        if !fetch_output.status.success() {
            let stderr = String::from_utf8_lossy(&fetch_output.stderr);
            anyhow::bail!("git fetch {} failed: {}", remote_name, stderr.trim());
        }

        Ok(())
    }

    /// Cleans up a worktree and its associated branch
    pub async fn cleanup_worktree(&self, repo_path: &Path, attempt_id: Uuid) -> Result<()> {
        let worktree_dir_name = format!("attempt-{}", attempt_id);
        let worktree_path = self.base_path().await.join(&worktree_dir_name);
        let branch_name = format!("feat/attempt-{}", attempt_id);

        // 1. Remove worktree
        // git worktree remove <path> --force
        let output_rm = Command::new("git")
            .current_dir(repo_path)
            .arg("worktree")
            .arg("remove")
            .arg(&worktree_path)
            .arg("--force")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git worktree remove")?;

        if !output_rm.status.success() {
            // Start logging warning but don't fail immediately, try to delete branch
            // In a real logger we would log this
            eprintln!(
                "Warning: git worktree remove failed: {}",
                String::from_utf8_lossy(&output_rm.stderr)
            );
        }

        // Best-effort prune of stale worktree metadata.
        let _ = Command::new("git")
            .current_dir(repo_path)
            .arg("worktree")
            .arg("prune")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        // 2. Delete branch
        // git branch -D feat/attempt-{id}
        let output_br = Command::new("git")
            .current_dir(repo_path)
            .arg("branch")
            .arg("-D")
            .arg(&branch_name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git branch -D")?;

        if !output_br.status.success() {
            // Check if branch didn't exist (maybe worktree creation failed mid-way)
            eprintln!(
                "Warning: git branch delete failed: {}",
                String::from_utf8_lossy(&output_br.stderr)
            );
        }

        // 3. Fallback hard-delete for leftover directory.
        if tokio::fs::try_exists(&worktree_path).await.unwrap_or(false) {
            tokio::fs::remove_dir_all(&worktree_path)
                .await
                .with_context(|| {
                    format!(
                        "Failed to remove leftover worktree directory {:?}",
                        worktree_path
                    )
                })?;
        }

        // 4. Verify cleanup outcome.
        if tokio::fs::try_exists(&worktree_path).await.unwrap_or(false) {
            anyhow::bail!(
                "Worktree cleanup incomplete: directory still exists at {:?}",
                worktree_path
            );
        }

        Ok(())
    }

    /// Commit all changes in the worktree
    ///
    /// Runs `git add . && git commit -m "<message>"` to stage and commit all changes.
    /// Returns Ok(true) if changes were committed, Ok(false) if nothing to commit.
    pub async fn commit_worktree(&self, worktree_path: &Path, message: &str) -> Result<bool> {
        // Stage all changes
        let add_output = Command::new("git")
            .current_dir(worktree_path)
            .arg("add")
            .arg(".")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git add")?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            anyhow::bail!("git add failed: {}", stderr);
        }

        // Check if there are staged changes
        let status_output = Command::new("git")
            .current_dir(worktree_path)
            .arg("status")
            .arg("--porcelain")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git status")?;

        let status_str = String::from_utf8_lossy(&status_output.stdout);
        if status_str.trim().is_empty() {
            // Nothing to commit
            return Ok(false);
        }

        // Commit changes
        let commit_output = Command::new("git")
            .current_dir(worktree_path)
            .arg("commit")
            // Automated commits must not depend on local developer git hooks
            // (e.g. husky/lint-staged requiring tools unavailable in worker env).
            .arg("--no-verify")
            .arg("-m")
            .arg(message)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git commit")?;

        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            // "nothing to commit" is not an error
            if stderr.contains("nothing to commit") {
                return Ok(false);
            }
            anyhow::bail!("git commit failed: {}", stderr);
        }

        Ok(true)
    }

    pub async fn push_worktree(&self, worktree_path: &Path) -> Result<()> {
        let mut output = self
            .run_push_worktree_command(worktree_path)
            .await
            .context("Failed to execute git push")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if is_recoverable_push_failure(&stderr) {
                let branch = self
                    .current_branch_name(worktree_path)
                    .await
                    .unwrap_or_else(|_| "HEAD".to_string());
                let _ = git_command_non_interactive()
                    .current_dir(worktree_path)
                    .arg("fetch")
                    .arg("origin")
                    .arg(&branch)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await;

                output = self
                    .run_push_worktree_command(worktree_path)
                    .await
                    .context("Failed to retry git push")?;

                if output.status.success() {
                    return Ok(());
                }

                let retry_stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "git push failed after retry: initial error: {}; retry error: {}",
                    stderr.trim(),
                    retry_stderr.trim()
                );
            }
            anyhow::bail!("git push failed: {}", stderr);
        }

        Ok(())
    }

    async fn run_push_worktree_command(&self, worktree_path: &Path) -> Result<Output> {
        git_command_non_interactive()
            .current_dir(worktree_path)
            .arg("push")
            .arg("--no-verify")
            .arg("--force-with-lease")
            .arg("-o")
            .arg("ci.skip")
            .arg("origin")
            .arg("HEAD")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git push command")
    }

    async fn current_branch_name(&self, repo_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to read current git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git rev-parse failed: {}", stderr.trim());
        }

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            anyhow::bail!("Current git branch is empty");
        }

        Ok(branch)
    }

    // ============================================================================
    // Additional Git Operations (Infrastructure for Future Use)
    // ============================================================================

    /// Pull latest changes from remote
    ///
    /// Usage: Sync local branch with remote before creating worktree
    /// ```ignore
    /// worktree_manager.pull_latest(repo_path, "main").await?;
    /// ```
    pub async fn pull_latest(&self, repo_path: &Path, branch: &str) -> Result<()> {
        // Checkout branch first
        let checkout = Command::new("git")
            .current_dir(repo_path)
            .arg("checkout")
            .arg(branch)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to checkout branch for pull")?;

        if !checkout.status.success() {
            let stderr = String::from_utf8_lossy(&checkout.stderr);
            anyhow::bail!("git checkout failed: {}", stderr);
        }

        // Pull latest
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("pull")
            .arg("origin")
            .arg(branch)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git pull")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git pull failed: {}", stderr);
        }

        Ok(())
    }

    /// Checkout an existing branch
    ///
    /// Usage: Switch to specific branch (e.g., for inspection or merge)
    /// ```ignore
    /// worktree_manager.checkout_branch(repo_path, "develop").await?;
    /// ```
    pub async fn checkout_branch(&self, repo_path: &Path, branch: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("checkout")
            .arg(branch)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git checkout")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git checkout failed: {}", stderr);
        }

        Ok(())
    }

    /// Merge a branch into current branch
    ///
    /// Usage: Merge feature branch into main (alternative to MR)
    /// ```ignore
    /// worktree_manager.checkout_branch(repo_path, "main").await?;
    /// worktree_manager.merge_branch(repo_path, "feat/attempt-123", false).await?;
    /// ```
    pub async fn merge_branch(
        &self,
        repo_path: &Path,
        source_branch: &str,
        no_ff: bool,
    ) -> Result<()> {
        let mut args = vec!["merge"];
        if no_ff {
            args.push("--no-ff");
        }
        args.push(source_branch);

        let output = Command::new("git")
            .current_dir(repo_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git merge")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git merge failed: {}", stderr);
        }

        Ok(())
    }

    /// Rebase current branch onto target branch
    ///
    /// Usage: Keep feature branch history clean
    /// ```ignore
    /// worktree_manager.checkout_branch(worktree_path, "feat/attempt-123").await?;
    /// worktree_manager.rebase_onto(worktree_path, "main").await?;
    /// ```
    pub async fn rebase_onto(&self, repo_path: &Path, target_branch: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("rebase")
            .arg(target_branch)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git rebase")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git rebase failed: {}", stderr);
        }

        Ok(())
    }

    /// Delete a local branch
    ///
    /// Usage: Cleanup merged branches
    /// ```ignore
    /// worktree_manager.delete_branch(repo_path, "feat/attempt-123", true).await?;
    /// ```
    pub async fn delete_branch(&self, repo_path: &Path, branch: &str, force: bool) -> Result<()> {
        let flag = if force { "-D" } else { "-d" };

        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("branch")
            .arg(flag)
            .arg(branch)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git branch delete")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git branch delete failed: {}", stderr);
        }

        Ok(())
    }

    /// List all local branches
    ///
    /// Usage: Discover existing branches
    /// ```ignore
    /// let branches = worktree_manager.list_branches(repo_path).await?;
    /// ```
    pub async fn list_branches(&self, repo_path: &Path) -> Result<Vec<String>> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("branch")
            .arg("--list")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git branch --list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git branch --list failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let branches: Vec<String> = stdout
            .lines()
            .map(|line| line.trim().trim_start_matches("* ").to_string())
            .filter(|b| !b.is_empty())
            .collect();

        Ok(branches)
    }

    /// Check if branch exists locally
    ///
    /// Usage: Validate branch before operations
    /// ```ignore
    /// if worktree_manager.branch_exists(repo_path, "feat/attempt-123").await? {
    ///     // Branch exists
    /// }
    /// ```
    pub async fn branch_exists(&self, repo_path: &Path, branch: &str) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .arg("rev-parse")
            .arg("--verify")
            .arg(branch)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git rev-parse")?;

        Ok(output.status.success())
    }
}

/// Compare two repo URLs (https or ssh) for equality (same host + path).
pub(crate) fn repo_url_matches(lhs: &str, rhs: &str) -> bool {
    match (parse_repo_identity(lhs), parse_repo_identity(rhs)) {
        (Some((left_host, left_path)), Some((right_host, right_path))) => {
            left_host.eq_ignore_ascii_case(&right_host) && left_path == right_path
        }
        _ => lhs.trim() == rhs.trim(),
    }
}

fn remote_url_matches_configured_value(current_remote: &str, configured_url: &str) -> bool {
    current_remote.trim() == configured_url.trim()
}

fn parse_tracking_branch(default_branch: &str, fallback_remote: &str) -> (String, String) {
    let trimmed = default_branch.trim();
    if trimmed.is_empty() || trimmed == "HEAD" {
        return (fallback_remote.to_string(), "main".to_string());
    }

    for remote in ["origin", "upstream"] {
        let prefix = format!("{}/", remote);
        if let Some(branch) = trimmed.strip_prefix(&prefix) {
            if !branch.trim().is_empty() {
                return (remote.to_string(), branch.to_string());
            }
        }
    }

    (fallback_remote.to_string(), trimmed.to_string())
}

fn git_command_non_interactive() -> Command {
    let mut cmd = Command::new("git");
    // Never block on terminal auth prompts in server/executor mode.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GCM_INTERACTIVE", "Never");
    cmd.env("GIT_ASKPASS", "echo");
    cmd.env("SSH_ASKPASS", "echo");
    cmd.env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes");
    cmd
}

fn inject_pat_into_url(url: &str, pat: &str) -> String {
    if pat.is_empty() {
        return url.to_string();
    }

    // Accept both HTTPS and SSH-style URLs; when SSH is provided we convert to HTTPS
    // so token auth can still work in headless production environments.
    let normalized = if url.starts_with("https://") || url.starts_with("http://") {
        url.trim().to_string()
    } else if let Some((host, path)) = parse_repo_identity(url) {
        format!("https://{}/{}.git", host, path)
    } else {
        return url.to_string();
    };

    let username = parse_repo_identity(&normalized)
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

fn is_recoverable_push_failure(stderr: &str) -> bool {
    let normalized = stderr.to_ascii_lowercase();
    normalized.contains("stale info")
        || normalized.contains("fetch first")
        || normalized.contains("failed to push some refs")
        || normalized.contains("non-fast-forward")
        || normalized.contains("remote rejected")
        || normalized.contains("cannot lock ref")
}

fn parse_repo_identity(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
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

#[cfg(test)]
mod tests {
    use super::{
        format_repository_clone_log, format_repository_sync_log, inject_pat_into_url,
        is_recoverable_push_failure, parse_tracking_branch, remote_url_matches_configured_value,
        repo_url_matches, summarize_repository_source, WorktreeManager,
    };
    use std::path::Path;
    use std::sync::Arc;
    use tokio::process::Command;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    async fn run_git(repo_path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .await
            .expect("failed to run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn inject_pat_into_https_gitlab_url() {
        let url = inject_pat_into_url("https://gitlab.example.com/group/repo.git", "glpat-123");
        assert_eq!(
            url,
            "https://oauth2:glpat-123@gitlab.example.com/group/repo.git"
        );
    }

    #[test]
    fn inject_pat_into_ssh_gitlab_url() {
        let url = inject_pat_into_url("git@gitlab.example.com:group/repo.git", "glpat-123");
        assert_eq!(
            url,
            "https://oauth2:glpat-123@gitlab.example.com/group/repo.git"
        );
    }

    #[test]
    fn inject_pat_into_https_github_url() {
        let url = inject_pat_into_url("https://github.com/openai/codex.git", "ghp_123");
        assert_eq!(
            url,
            "https://x-access-token:ghp_123@github.com/openai/codex.git"
        );
    }

    #[test]
    fn inject_pat_into_ssh_github_url() {
        let url = inject_pat_into_url("git@github.com:openai/codex.git", "ghp_123");
        assert_eq!(
            url,
            "https://x-access-token:ghp_123@github.com/openai/codex.git"
        );
    }

    #[test]
    fn inject_pat_keeps_original_when_empty() {
        let raw = "git@gitlab.example.com:group/repo.git";
        assert_eq!(inject_pat_into_url(raw, ""), raw);
    }

    #[test]
    fn repo_url_matches_between_https_and_ssh() {
        assert!(repo_url_matches(
            "https://gitlab.example.com/group/repo.git",
            "git@gitlab.example.com:group/repo.git"
        ));
    }

    #[test]
    fn remote_url_exact_match_requires_retarget_when_pat_is_missing_from_current_remote() {
        assert!(!remote_url_matches_configured_value(
            "https://gitlab.example.com/group/repo.git",
            "https://oauth2:glpat-123@gitlab.example.com/group/repo.git"
        ));
    }

    #[test]
    fn remote_url_exact_match_requires_retarget_when_pat_rotates() {
        assert!(!remote_url_matches_configured_value(
            "https://oauth2:old@gitlab.example.com/group/repo.git",
            "https://oauth2:new@gitlab.example.com/group/repo.git"
        ));
    }

    #[test]
    fn remote_url_exact_match_accepts_same_configured_remote() {
        assert!(remote_url_matches_configured_value(
            "https://oauth2:glpat-123@gitlab.example.com/group/repo.git",
            "https://oauth2:glpat-123@gitlab.example.com/group/repo.git"
        ));
    }

    #[test]
    fn parse_tracking_branch_from_origin_ref() {
        let (remote, branch) = parse_tracking_branch("origin/main", "origin");
        assert_eq!(remote, "origin");
        assert_eq!(branch, "main");
    }

    #[test]
    fn parse_tracking_branch_from_upstream_ref() {
        let (remote, branch) = parse_tracking_branch("upstream/main", "upstream");
        assert_eq!(remote, "upstream");
        assert_eq!(branch, "main");
    }

    #[test]
    fn parse_tracking_branch_keeps_local_branch_name() {
        let (remote, branch) = parse_tracking_branch("release/2026", "origin");
        assert_eq!(remote, "origin");
        assert_eq!(branch, "release/2026");
    }

    #[test]
    fn parse_tracking_branch_handles_head_fallback() {
        let (remote, branch) = parse_tracking_branch("HEAD", "upstream");
        assert_eq!(remote, "upstream");
        assert_eq!(branch, "main");
    }

    #[test]
    fn summarize_repository_source_uses_repo_name_for_local_path() {
        assert_eq!(
            summarize_repository_source("/Users/thaonv/Projects/Personal/Agentic-Coding"),
            "Agentic-Coding"
        );
    }

    #[test]
    fn summarize_repository_source_uses_repo_name_for_remote_url() {
        assert_eq!(
            summarize_repository_source("https://gitlab.example.com/group/repo-name.git"),
            "repo-name"
        );
    }

    #[test]
    fn repository_log_messages_do_not_expose_absolute_paths() {
        let sync = format_repository_sync_log("/Users/thaonv/Projects/Personal/Agentic-Coding");
        let clone = format_repository_clone_log("/Users/thaonv/Projects/Personal/Agentic-Coding");

        assert_eq!(
            sync,
            "Repository is already available locally. Syncing latest changes from Agentic-Coding."
        );
        assert_eq!(
            clone,
            "Preparing a fresh local repository copy from Agentic-Coding."
        );
    }

    #[tokio::test]
    async fn create_worktree_reuses_existing_attempt_branch_for_follow_up() {
        let repo_dir = tempfile::tempdir().expect("repo tempdir");
        let worktrees_dir = tempfile::tempdir().expect("worktrees tempdir");
        let repo_path = repo_dir.path();

        run_git(repo_path, &["init", "-b", "main"]).await;
        run_git(repo_path, &["config", "user.name", "ACPMS Test"]).await;
        run_git(repo_path, &["config", "user.email", "test@acpms.local"]).await;
        tokio::fs::write(repo_path.join("README.md"), "seed\n")
            .await
            .expect("write seed file");
        run_git(repo_path, &["add", "."]).await;
        run_git(repo_path, &["commit", "-m", "seed"]).await;

        let manager =
            WorktreeManager::new(Arc::new(RwLock::new(worktrees_dir.path().to_path_buf())));
        let attempt_id = Uuid::new_v4();

        let first = manager
            .create_worktree(repo_path, attempt_id, Some("HEAD"))
            .await
            .expect("first worktree");
        assert!(first.path.exists(), "first worktree should exist");

        tokio::fs::remove_dir_all(&first.path)
            .await
            .expect("remove first worktree path");

        let recreated = manager
            .create_worktree(repo_path, attempt_id, Some("HEAD"))
            .await
            .expect("recreate worktree from existing branch");

        assert!(recreated.path.exists(), "recreated worktree should exist");
        assert_eq!(
            recreated.feature_branch,
            format!("feat/attempt-{}", attempt_id)
        );

        let output = Command::new("git")
            .current_dir(&recreated.path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .await
            .expect("read current branch");
        assert!(
            output.status.success(),
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(current_branch, recreated.feature_branch);
    }

    #[test]
    fn recoverable_push_failure_classifier_catches_common_git_drift_errors() {
        assert!(is_recoverable_push_failure(
            "failed to push some refs to origin because the remote contains work that you do not have locally (fetch first)"
        ));
        assert!(is_recoverable_push_failure(
            "remote rejected: cannot lock ref 'refs/heads/feat/x': is at abc but expected def"
        ));
        assert!(is_recoverable_push_failure(
            "stale info: remote branch updated during push"
        ));
        assert!(!is_recoverable_push_failure(
            "authentication failed: invalid token"
        ));
    }
}
