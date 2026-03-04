use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
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
            // Repository exists - fetch and pull latest changes
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
            if !repo_url_matches(&current_remote, remote_url) {
                tracing::info!(
                    "Repository remote changed at {:?}: retargeting origin from {} to {}",
                    repo_path,
                    current_remote,
                    remote_url
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
            let fetch_output = Command::new("git")
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

            // Detect default branch and pull
            let default_branch = self
                .detect_default_branch(
                    repo_path,
                    upstream_url.map(|_| "upstream").unwrap_or("origin"),
                )
                .await
                .unwrap_or_else(|_| "origin/main".to_string());

            // Extract just the branch name (e.g., "main" from "origin/main")
            let branch_name = default_branch
                .strip_prefix("origin/")
                .unwrap_or(&default_branch);

            // Checkout the default branch first (in case we're on a detached HEAD or different branch)
            let checkout_output = Command::new("git")
                .current_dir(repo_path)
                .arg("checkout")
                .arg(branch_name)
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
            let pull_output = Command::new("git")
                .current_dir(repo_path)
                .arg("pull")
                .arg("origin")
                .arg(branch_name)
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

        let output = Command::new("git")
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

        // git worktree add -b <branch> <path> <base-ref>
        let output = Command::new("git")
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
            .context("Failed to execute git worktree command")?;

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
            if repo_url_matches(&current_remote, expected_url) {
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

        let fetch_output = Command::new("git")
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
        let output = Command::new("git")
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
            .context("Failed to execute git push")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git push failed: {}", stderr);
        }

        Ok(())
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

fn inject_pat_into_url(url: &str, pat: &str) -> String {
    if pat.is_empty() || !url.starts_with("https://") {
        return url.to_string();
    }

    url.replace("https://", &format!("https://oauth2:{}@", pat))
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
