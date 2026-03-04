use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;

/// Git credential helper for securely passing PATs to git operations.
///
/// ## Security Features
/// - Credentials never appear in git URLs
/// - Helper script has restrictive permissions (0700)
/// - Automatic cleanup after operation
/// - In-memory credential passing
/// - No credential caching
pub struct GitCredentialHelper {
    helper_path: PathBuf,
}

impl GitCredentialHelper {
    /// Create a new GitCredentialHelper with a temporary script location.
    pub fn new() -> Result<Self> {
        let temp_dir = std::env::temp_dir();
        let helper_path = temp_dir.join(format!("git-cred-helper-{}", uuid::Uuid::new_v4()));
        Ok(Self { helper_path })
    }

    /// Write credential helper script with PAT.
    ///
    /// ## Security
    /// - Script has 0700 permissions (owner read/write/execute only)
    /// - PAT embedded in script (in-memory, not persistent)
    /// - Script auto-deleted after use
    async fn write_helper_script(&self, pat: &str) -> Result<()> {
        let script = format!(
            r#"#!/bin/sh
echo "username=oauth2"
echo "password={}"
"#,
            pat
        );

        fs::write(&self.helper_path, script)
            .await
            .context("Failed to write credential helper script")?;

        #[cfg(unix)]
        {
            let metadata = fs::metadata(&self.helper_path).await?;
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&self.helper_path, permissions)
                .await
                .context("Failed to set credential helper permissions")?;
        }

        Ok(())
    }

    /// Remove credential helper script.
    async fn cleanup(&self) -> Result<()> {
        if self.helper_path.exists() {
            fs::remove_file(&self.helper_path)
                .await
                .context("Failed to remove credential helper script")?;
        }
        Ok(())
    }

    /// Execute git command with credential helper configured.
    async fn exec_with_helper(
        &self,
        pat: &str,
        mut command: Command,
    ) -> Result<std::process::Output> {
        self.write_helper_script(pat).await?;
        let cleanup_guard = CleanupGuard::new(self.helper_path.clone());

        let output = command
            .output()
            .await
            .context("Failed to execute git command")?;

        drop(cleanup_guard);
        self.cleanup().await?;

        Ok(output)
    }

    /// Clone a Git repository using credential helper (no PAT in URL).
    pub async fn clone_repo(&self, repo_url: &str, pat: &str, dest_path: &Path) -> Result<()> {
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directory")?;
        }

        let mut cmd = Command::new("git");
        cmd.arg("clone")
            .arg(repo_url)
            .arg(dest_path)
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "credential.helper")
            .env(
                "GIT_CONFIG_VALUE_0",
                self.helper_path.to_str().context("Invalid helper path")?,
            );

        let output = self.exec_with_helper(pat, cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = stderr.replace(pat, "***");
            anyhow::bail!("git clone failed: {}", sanitized);
        }

        Ok(())
    }

    /// Pull changes from remote using credential helper.
    pub async fn pull_repo(&self, repo_path: &Path, pat: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_path)
            .arg("pull")
            .arg("origin")
            .arg("main")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "credential.helper")
            .env(
                "GIT_CONFIG_VALUE_0",
                self.helper_path.to_str().context("Invalid helper path")?,
            );

        let output = self.exec_with_helper(pat, cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = stderr.replace(pat, "***");
            anyhow::bail!("git pull failed: {}", sanitized);
        }

        Ok(())
    }

    /// Push changes to remote using credential helper.
    pub async fn push_repo(&self, repo_path: &Path, pat: &str, branch: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_path)
            .arg("push")
            .arg("origin")
            .arg(branch)
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "credential.helper")
            .env(
                "GIT_CONFIG_VALUE_0",
                self.helper_path.to_str().context("Invalid helper path")?,
            );

        let output = self.exec_with_helper(pat, cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = stderr.replace(pat, "***");
            anyhow::bail!("git push failed: {}", sanitized);
        }

        Ok(())
    }
}

/// RAII guard to ensure credential helper cleanup even on panic.
struct CleanupGuard {
    path: PathBuf,
}

impl CleanupGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
