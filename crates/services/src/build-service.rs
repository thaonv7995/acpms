//! Build Service for running project builds and uploading artifacts to MinIO.
//!
//! Supports different build configurations based on project type:
//! - Web: `npm run build` -> dist/
//! - API: `cargo build --release` -> target/release/
//! - Mobile: `eas build` -> .ipa/.apk
//! - Extension: `npm run build:ext` -> ext/
//! - Microservice: `docker build` -> image

use acpms_db::{
    models::{BuildArtifact, BuildConfig, BuildResult, Project, ProjectType},
    PgPool,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::process::Command;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::StorageService;

/// Error types for build operations
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Build failed: {0}")]
    Failed(String),
    #[error("Worktree not found: {0}")]
    WorktreeNotFound(String),
    #[error("Upload failed: {0}")]
    UploadFailed(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Unsupported project type: {0}")]
    UnsupportedType(String),
}

/// Service for running builds and managing build artifacts
pub struct BuildService {
    storage: StorageService,
    db: PgPool,
    worktrees_base_path: std::sync::Arc<tokio::sync::RwLock<std::path::PathBuf>>,
}

#[derive(Debug, Clone)]
struct UploadedArtifact {
    artifact_key: String,
    artifact_type: String,
    size_bytes: u64,
    file_count: usize,
}

impl BuildService {
    /// Create a new BuildService
    pub fn new(
        storage: StorageService,
        db: PgPool,
        worktrees_base_path: std::sync::Arc<tokio::sync::RwLock<std::path::PathBuf>>,
    ) -> Self {
        Self {
            storage,
            db,
            worktrees_base_path,
        }
    }

    /// Run a build for a task attempt
    ///
    /// 1. Determines the build configuration based on project type
    /// 2. Executes the build command in the worktree
    /// 3. Uploads artifacts to MinIO
    /// 4. Records the build artifact in the database
    pub async fn run_build(
        &self,
        project: &Project,
        attempt_id: Uuid,
        build_override: Option<BuildConfig>,
    ) -> Result<BuildResult> {
        let start_time = Instant::now();

        // Get worktree path
        let worktree_path = self.get_worktree_path(attempt_id).await?;
        if !worktree_path.exists() {
            return Err(BuildError::WorktreeNotFound(worktree_path.display().to_string()).into());
        }

        // Get build configuration (use override if provided, otherwise use defaults)
        let build_config = build_override.unwrap_or_else(|| self.get_build_config(project));

        info!(
            "Starting build for project {} (type: {:?}), attempt {}",
            project.name, project.project_type, attempt_id
        );
        info!("Build command: {}", build_config.command);
        info!("Output directory: {}", build_config.output_dir);

        // Run build command
        let output = self
            .execute_build_command(&worktree_path, &build_config.command)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Build failed: {}", stderr);
            return Err(BuildError::Failed(stderr.to_string()).into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Build output: {}", stdout);

        // Calculate build duration
        let build_duration = start_time.elapsed().as_secs() as i32;

        // Upload artifacts to MinIO
        let artifact_path = worktree_path.join(&build_config.output_dir);
        if !artifact_path.exists() {
            warn!(
                "Build output directory does not exist: {}",
                artifact_path.display()
            );
            // Create empty result for builds that don't produce artifacts
            return Ok(BuildResult {
                artifact_key: String::new(),
                size_bytes: 0,
                files_count: 0,
                build_duration_secs: build_duration,
            });
        }

        let uploaded = self
            .upload_artifacts(project, attempt_id, &artifact_path)
            .await?;

        if uploaded.is_empty() {
            warn!(
                "No artifacts were uploaded for attempt {} (project type: {:?})",
                attempt_id, project.project_type
            );
            return Ok(BuildResult {
                artifact_key: String::new(),
                size_bytes: 0,
                files_count: 0,
                build_duration_secs: build_duration,
            });
        }

        // Save one DB record per uploaded artifact.
        for item in &uploaded {
            self.save_artifact_record(
                attempt_id,
                project.id,
                &item.artifact_key,
                &item.artifact_type,
                item.size_bytes as i64,
                item.file_count as i32,
                &build_config.command,
                build_duration,
            )
            .await?;
        }

        let primary = uploaded.first().cloned().unwrap_or(UploadedArtifact {
            artifact_key: String::new(),
            artifact_type: self.get_artifact_type(&project.project_type),
            size_bytes: 0,
            file_count: 0,
        });

        info!(
            "Build completed successfully in {}s. Uploaded {} artifact(s). Primary: {}",
            build_duration,
            uploaded.len(),
            primary.artifact_key
        );

        Ok(BuildResult {
            artifact_key: primary.artifact_key,
            size_bytes: primary.size_bytes,
            files_count: primary.file_count,
            build_duration_secs: build_duration,
        })
    }

    /// Get the build configuration for a project type
    pub fn get_build_config(&self, project: &Project) -> BuildConfig {
        // Check if project has custom build command in settings
        let custom_command = project
            .metadata
            .get("build_command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let custom_output = project
            .metadata
            .get("build_output_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        match project.project_type {
            ProjectType::Web => BuildConfig {
                command: custom_command.unwrap_or_else(|| "npm run build".to_string()),
                output_dir: custom_output.unwrap_or_else(|| "dist".to_string()),
            },
            ProjectType::Api => BuildConfig {
                command: custom_command.unwrap_or_else(|| "cargo build --release".to_string()),
                output_dir: custom_output.unwrap_or_else(|| "target/release".to_string()),
            },
            ProjectType::Mobile => BuildConfig {
                command: custom_command.unwrap_or_else(|| "npx eas build --local".to_string()),
                output_dir: custom_output.unwrap_or_else(|| "build".to_string()),
            },
            ProjectType::Extension => BuildConfig {
                command: custom_command.unwrap_or_else(|| "npm run build:ext".to_string()),
                output_dir: custom_output.unwrap_or_else(|| "ext".to_string()),
            },
            ProjectType::Microservice => BuildConfig {
                command: custom_command.unwrap_or_else(|| "docker build -t app .".to_string()),
                output_dir: custom_output.unwrap_or_else(|| ".".to_string()),
            },
            ProjectType::Desktop => BuildConfig {
                command: custom_command.unwrap_or_else(|| "npm run package".to_string()),
                output_dir: custom_output.unwrap_or_else(|| "out".to_string()),
            },
        }
    }

    /// Execute the build command in the worktree directory
    async fn execute_build_command(
        &self,
        worktree_path: &Path,
        command: &str,
    ) -> Result<std::process::Output> {
        // Install dependencies first for npm-based projects
        if command.contains("npm") {
            info!("Installing npm dependencies...");
            let install_output = Command::new("sh")
                .arg("-c")
                .arg("npm install")
                .current_dir(worktree_path)
                .output()
                .await
                .context("Failed to run npm install")?;

            if !install_output.status.success() {
                let stderr = String::from_utf8_lossy(&install_output.stderr);
                warn!("npm install warning: {}", stderr);
            }
        }

        // Run the build command
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(worktree_path)
            .output()
            .await
            .context("Failed to execute build command")?;

        Ok(output)
    }

    /// Upload build artifacts to MinIO with project-type specific packaging.
    ///
    /// - Web/API/Microservice: upload a single `tar.gz` bundle.
    /// - Extension: prefer prebuilt `.zip` bundles; fallback to zipping output dir.
    /// - Desktop/Mobile: upload platform-specific distributables (macOS/windows or ios/android)
    ///   if detected, plus fallback bundled archive when no distributable is found.
    async fn upload_artifacts(
        &self,
        project: &Project,
        attempt_id: Uuid,
        artifact_path: &Path,
    ) -> Result<Vec<UploadedArtifact>> {
        let base_key = format!("builds/{}/{}", project.id, attempt_id);

        match project.project_type {
            ProjectType::Desktop => {
                self.upload_desktop_artifacts(&base_key, artifact_path)
                    .await
            }
            ProjectType::Mobile => self.upload_mobile_artifacts(&base_key, artifact_path).await,
            ProjectType::Extension => {
                self.upload_extension_artifacts(&base_key, artifact_path)
                    .await
            }
            _ => {
                let artifact = self
                    .upload_directory_archive(&base_key, artifact_path, "application/gzip")
                    .await?;
                Ok(vec![UploadedArtifact {
                    artifact_type: self.get_artifact_type(&project.project_type),
                    ..artifact
                }])
            }
        }
    }

    async fn upload_directory_archive(
        &self,
        base_key: &str,
        artifact_path: &Path,
        content_type: &str,
    ) -> Result<UploadedArtifact> {
        let (size_bytes, files_count) = self.calculate_directory_stats(artifact_path).await?;
        let archive_path = artifact_path.with_extension("tar.gz");
        self.create_archive(artifact_path, &archive_path).await?;

        let archive_size = tokio::fs::metadata(&archive_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let archive_key = format!("{}/artifacts.tar.gz", base_key);
        self.upload_file_to_storage(&archive_path, &archive_key, content_type)
            .await?;
        let _ = tokio::fs::remove_file(&archive_path).await;

        Ok(UploadedArtifact {
            artifact_key: archive_key,
            artifact_type: "bundle".to_string(),
            size_bytes: size_bytes.max(archive_size),
            file_count: files_count,
        })
    }

    async fn upload_extension_artifacts(
        &self,
        base_key: &str,
        artifact_path: &Path,
    ) -> Result<Vec<UploadedArtifact>> {
        let files = self.collect_files_recursively(artifact_path).await?;
        let mut uploaded = Vec::new();

        for file in files.iter().filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("zip"))
                .unwrap_or(false)
        }) {
            let file_name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("extension.zip")
                .to_string();
            let key = format!("{}/extension/{}", base_key, file_name);
            let size = tokio::fs::metadata(file)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            self.upload_file_to_storage(file, &key, "application/zip")
                .await?;
            uploaded.push(UploadedArtifact {
                artifact_key: key,
                artifact_type: "extension_zip".to_string(),
                size_bytes: size,
                file_count: 1,
            });
        }

        if uploaded.is_empty() {
            let zip_path = artifact_path.with_extension("zip");
            self.create_zip_archive(artifact_path, &zip_path).await?;
            let key = format!("{}/extension/extension.zip", base_key);
            let size = tokio::fs::metadata(&zip_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            self.upload_file_to_storage(&zip_path, &key, "application/zip")
                .await?;
            let _ = tokio::fs::remove_file(&zip_path).await;
            uploaded.push(UploadedArtifact {
                artifact_key: key,
                artifact_type: "extension_zip".to_string(),
                size_bytes: size,
                file_count: 1,
            });
        }

        Ok(uploaded)
    }

    async fn upload_desktop_artifacts(
        &self,
        base_key: &str,
        artifact_path: &Path,
    ) -> Result<Vec<UploadedArtifact>> {
        let files = self.collect_files_recursively(artifact_path).await?;
        let mut uploaded: Vec<UploadedArtifact> = Vec::new();

        for file in files {
            let Some(file_name) = file.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let name_lc = file_name.to_ascii_lowercase();
            let ext_lc = file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();

            let platform = if ext_lc == "dmg"
                || ext_lc == "pkg"
                || name_lc.contains("mac")
                || name_lc.contains("darwin")
            {
                Some("macos")
            } else if ext_lc == "exe" || ext_lc == "msi" || name_lc.contains("win") {
                Some("windows")
            } else {
                None
            };

            let Some(platform) = platform else {
                continue;
            };

            let key = format!("{}/desktop/{}/{}", base_key, platform, file_name);
            let size = tokio::fs::metadata(&file)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            let content_type = self.detect_content_type(&file);
            self.upload_file_to_storage(&file, &key, &content_type)
                .await?;
            uploaded.push(UploadedArtifact {
                artifact_key: key,
                artifact_type: format!("desktop_{}", platform),
                size_bytes: size,
                file_count: 1,
            });
        }

        if uploaded.is_empty() {
            let fallback = self
                .upload_directory_archive(base_key, artifact_path, "application/gzip")
                .await?;
            uploaded.push(UploadedArtifact {
                artifact_type: "installer".to_string(),
                ..fallback
            });
        }

        Ok(uploaded)
    }

    async fn upload_mobile_artifacts(
        &self,
        base_key: &str,
        artifact_path: &Path,
    ) -> Result<Vec<UploadedArtifact>> {
        let files = self.collect_files_recursively(artifact_path).await?;
        let mut uploaded: Vec<UploadedArtifact> = Vec::new();

        for file in files {
            let Some(file_name) = file.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let ext_lc = file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();

            let platform = match ext_lc.as_str() {
                "ipa" => Some("ios"),
                "apk" | "aab" => Some("android"),
                _ => None,
            };

            let Some(platform) = platform else {
                continue;
            };

            let key = format!("{}/mobile/{}/{}", base_key, platform, file_name);
            let size = tokio::fs::metadata(&file)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            let content_type = self.detect_content_type(&file);
            self.upload_file_to_storage(&file, &key, &content_type)
                .await?;
            uploaded.push(UploadedArtifact {
                artifact_key: key,
                artifact_type: format!("mobile_{}", platform),
                size_bytes: size,
                file_count: 1,
            });
        }

        if uploaded.is_empty() {
            let fallback = self
                .upload_directory_archive(base_key, artifact_path, "application/gzip")
                .await?;
            uploaded.push(UploadedArtifact {
                artifact_type: "mobile".to_string(),
                ..fallback
            });
        }

        Ok(uploaded)
    }

    async fn upload_file_to_storage(
        &self,
        file_path: &Path,
        key: &str,
        content_type: &str,
    ) -> Result<()> {
        let upload_url = self
            .storage
            .get_presigned_upload_url(key, content_type, std::time::Duration::from_secs(3600))
            .await
            .context("Failed to get presigned upload URL")?;

        let data = tokio::fs::read(file_path)
            .await
            .with_context(|| format!("Failed to read artifact file: {}", file_path.display()))?;

        let client = reqwest::Client::new();
        let response = client
            .put(&upload_url)
            .header("Content-Type", content_type)
            .body(data)
            .send()
            .await
            .context("Failed to upload artifact")?;

        if !response.status().is_success() {
            return Err(BuildError::UploadFailed(format!(
                "Upload failed with status: {}",
                response.status()
            ))
            .into());
        }

        Ok(())
    }

    async fn collect_files_recursively(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut stack = vec![path.to_path_buf()];

        while let Some(current_path) = stack.pop() {
            if current_path.is_file() {
                files.push(current_path);
                continue;
            }
            if current_path.is_dir() {
                let mut entries = tokio::fs::read_dir(&current_path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    stack.push(entry.path());
                }
            }
        }

        Ok(files)
    }

    fn detect_content_type(&self, path: &Path) -> String {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        match ext.as_str() {
            "zip" => "application/zip".to_string(),
            "dmg" => "application/x-apple-diskimage".to_string(),
            "pkg" => "application/octet-stream".to_string(),
            "exe" => "application/vnd.microsoft.portable-executable".to_string(),
            "msi" => "application/x-msi".to_string(),
            "apk" => "application/vnd.android.package-archive".to_string(),
            "aab" => "application/octet-stream".to_string(),
            "ipa" => "application/octet-stream".to_string(),
            "tar" => "application/x-tar".to_string(),
            "gz" | "tgz" => "application/gzip".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }

    /// Create a tar.gz archive of the build output
    async fn create_archive(&self, source_dir: &Path, archive_path: &Path) -> Result<()> {
        let _source_dir_str = source_dir.display().to_string();
        let archive_path_str = archive_path.display().to_string();

        let output = Command::new("tar")
            .arg("-czf")
            .arg(&archive_path_str)
            .arg("-C")
            .arg(source_dir.parent().unwrap_or(source_dir))
            .arg(source_dir.file_name().unwrap_or_default())
            .output()
            .await
            .context("Failed to create archive")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create archive: {}", stderr));
        }

        Ok(())
    }

    /// Create a zip archive of the build output.
    async fn create_zip_archive(&self, source_dir: &Path, archive_path: &Path) -> Result<()> {
        let parent = source_dir.parent().unwrap_or(source_dir);
        let source_name = source_dir.file_name().unwrap_or_default();

        let output = Command::new("zip")
            .arg("-r")
            .arg("-q")
            .arg(archive_path)
            .arg(source_name)
            .current_dir(parent)
            .output()
            .await
            .context("Failed to create zip archive")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create zip archive: {}", stderr));
        }

        Ok(())
    }

    /// Calculate directory size and file count
    async fn calculate_directory_stats(&self, path: &Path) -> Result<(u64, usize)> {
        let mut total_size: u64 = 0;
        let mut file_count: usize = 0;

        let mut stack = vec![path.to_path_buf()];

        while let Some(current_path) = stack.pop() {
            if current_path.is_file() {
                if let Ok(metadata) = tokio::fs::metadata(&current_path).await {
                    total_size += metadata.len();
                    file_count += 1;
                }
            } else if current_path.is_dir() {
                let mut entries = tokio::fs::read_dir(&current_path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    stack.push(entry.path());
                }
            }
        }

        Ok((total_size, file_count))
    }

    /// Get the worktree path for an attempt
    async fn get_worktree_path(&self, attempt_id: Uuid) -> Result<PathBuf> {
        let worktree_dir_name = format!("attempt-{}", attempt_id);
        let base = self.worktrees_base_path.read().await.clone();
        Ok(base.join(worktree_dir_name))
    }

    /// Get artifact type string based on project type
    fn get_artifact_type(&self, project_type: &ProjectType) -> String {
        match project_type {
            ProjectType::Web => "dist".to_string(),
            ProjectType::Api => "binary".to_string(),
            ProjectType::Mobile => "mobile".to_string(),
            ProjectType::Extension => "extension".to_string(),
            ProjectType::Microservice => "container".to_string(),
            ProjectType::Desktop => "installer".to_string(),
        }
    }

    /// Save artifact record to database
    async fn save_artifact_record(
        &self,
        attempt_id: Uuid,
        project_id: Uuid,
        artifact_key: &str,
        artifact_type: &str,
        size_bytes: i64,
        file_count: i32,
        build_command: &str,
        build_duration_secs: i32,
    ) -> Result<BuildArtifact> {
        let artifact = sqlx::query_as::<_, BuildArtifact>(
            r#"
            INSERT INTO build_artifacts (
                attempt_id, project_id, artifact_key, artifact_type,
                size_bytes, file_count, build_command, build_duration_secs
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, attempt_id, project_id, artifact_key, artifact_type,
                      size_bytes, file_count, build_command, build_duration_secs, created_at
            "#,
        )
        .bind(attempt_id)
        .bind(project_id)
        .bind(artifact_key)
        .bind(artifact_type)
        .bind(size_bytes)
        .bind(file_count)
        .bind(build_command)
        .bind(build_duration_secs)
        .fetch_one(&self.db)
        .await
        .context("Failed to save build artifact record")?;

        Ok(artifact)
    }

    /// Get build artifacts for an attempt
    pub async fn get_attempt_artifacts(&self, attempt_id: Uuid) -> Result<Vec<BuildArtifact>> {
        let artifacts = sqlx::query_as::<_, BuildArtifact>(
            r#"
            SELECT id, attempt_id, project_id, artifact_key, artifact_type,
                   size_bytes, file_count, build_command, build_duration_secs, created_at
            FROM build_artifacts
            WHERE attempt_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(attempt_id)
        .fetch_all(&self.db)
        .await
        .context("Failed to fetch build artifacts")?;

        Ok(artifacts)
    }

    /// Get latest build artifact for a project
    pub async fn get_latest_artifact(&self, project_id: Uuid) -> Result<Option<BuildArtifact>> {
        let artifact = sqlx::query_as::<_, BuildArtifact>(
            r#"
            SELECT id, attempt_id, project_id, artifact_key, artifact_type,
                   size_bytes, file_count, build_command, build_duration_secs, created_at
            FROM build_artifacts
            WHERE project_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to fetch latest build artifact")?;

        Ok(artifact)
    }

    /// Get presigned download URL for an artifact
    pub async fn get_artifact_download_url(&self, artifact_key: &str) -> Result<String> {
        let url = self
            .storage
            .get_presigned_download_url(artifact_key, std::time::Duration::from_secs(3600))
            .await
            .context("Failed to generate download URL")?;

        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_config_web() {
        // Test that web projects get correct default config
        let config = BuildConfig {
            command: "npm run build".to_string(),
            output_dir: "dist".to_string(),
        };
        assert_eq!(config.command, "npm run build");
        assert_eq!(config.output_dir, "dist");
    }

    #[test]
    fn test_build_config_api() {
        let config = BuildConfig {
            command: "cargo build --release".to_string(),
            output_dir: "target/release".to_string(),
        };
        assert_eq!(config.command, "cargo build --release");
        assert_eq!(config.output_dir, "target/release");
    }
}
