use super::*;
use crate::task_skills::get_skill_content;
use crate::worktree::{format_repository_clone_log, format_repository_sync_log};
use acpms_db::models::{RepositoryProvider, RepositoryVerificationStatus};
use chrono::Utc;
use std::io::Cursor;
use tokio::process::Command;
use tokio::sync::watch;

const GITLAB_DEVELOPER_ACCESS_LEVEL: u64 = 30;
const GITLAB_MAINTAINER_ACCESS_LEVEL: u64 = 40;

impl ExecutorOrchestrator {
    /// Execute init task with routing based on metadata.
    ///
    /// ## Init Task Types
    /// - **gitlab_import**: Simple clone/pull from existing GitLab repository
    /// - **from_scratch**: Full project initialization with GitLab repo creation
    ///
    /// ## Parameters
    /// - `task_id`: The task to execute
    /// - `task`: The task record
    /// - `existing_attempt_id`: Optional existing attempt_id to use (avoids duplicate attempts)
    pub(super) async fn execute_init_task(
        &self,
        task_id: Uuid,
        task: &Task,
        existing_attempt_id: Option<Uuid>,
    ) -> Result<()> {
        // Parse init metadata
        let init_metadata = InitTaskMetadata::parse(&task.metadata)
            .context("Failed to parse init task metadata")?;

        match init_metadata.source {
            InitSource::GitlabImport => {
                self.execute_gitlab_import(task_id, task, &init_metadata, existing_attempt_id)
                    .await?;
            }
            InitSource::FromScratch => {
                self.execute_from_scratch(task_id, task, &init_metadata, existing_attempt_id)
                    .await?;
            }
        }

        Ok(())
    }

    /// Execute GitLab import init task (simple clone/pull).
    ///
    /// ## Flow
    /// 1. Create attempt record (or use existing)
    /// 2. Prepare local path
    /// 3. Clone or sync repository using WorktreeManager
    /// 4. Update project repository_url
    /// 5. Mark task completed
    ///
    /// Note: This uses WorktreeManager directly (no agent spawn needed for simple git operations)
    async fn execute_gitlab_import(
        &self,
        task_id: Uuid,
        task: &Task,
        metadata: &InitTaskMetadata,
        existing_attempt_id: Option<Uuid>,
    ) -> Result<()> {
        // Use existing attempt_id if provided, otherwise create new one
        let attempt_id = match existing_attempt_id {
            Some(id) => id,
            None => self.create_attempt(task_id).await?,
        };

        // Update attempt status to Running
        self.update_status(attempt_id, AttemptStatus::Running)
            .await?;

        // Get project info
        let project = self.fetch_project(task.project_id).await?;
        let project_settings = self
            .fetch_project_settings(task.project_id)
            .await
            .unwrap_or_default();
        let repo_url = metadata
            .repository_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing repository_url in metadata"))?;

        // Resolve PAT for private repos: use system GitLab or GitHub PAT when repo host matches
        let pat = self
            .resolve_repo_pat_for_clone(attempt_id, repo_url)
            .await?;

        // Resolve repo path (slug only, with collision handling: slug-2, slug-3, ...)
        let repo_path = self
            .resolve_repo_path_with_collision(
                attempt_id,
                task.project_id,
                &project.metadata,
                &project.name,
                Some(repo_url),
            )
            .await?;

        let skill_context = self.build_skill_instruction_context(
            task,
            &project_settings,
            project.project_type,
            Some(repo_path.as_path()),
        );
        if let Err(error) = self
            .persist_skill_instruction_context(attempt_id, &skill_context, "init_gitlab_import")
            .await
        {
            warn!(
                attempt_id = %attempt_id,
                error = %error,
                "Failed to persist skill instruction metadata for GitLab import init attempt"
            );
        }
        if let Err(error) = self.log_loaded_skills(attempt_id, &skill_context).await {
            warn!(
                attempt_id = %attempt_id,
                error = %error,
                "Failed to append skill timeline log for GitLab import init attempt"
            );
        }

        // Create execution_process when orchestrator created the attempt (no server-created record)
        if existing_attempt_id.is_none() {
            if let Err(e) = self
                .create_execution_process(attempt_id, Some(&repo_path), None)
                .await
            {
                warn!(
                    attempt_id = %attempt_id,
                    error = %e,
                    "Failed to create execution process for GitLab import (follow-up may be disabled)"
                );
            }
        }

        self.log(
            attempt_id,
            "system",
            &format!("Starting GitLab import: {}", repo_url),
        )
        .await?;

        // Check if repo exists to log appropriate message
        if repo_path.join(".git").exists() {
            self.log(attempt_id, "system", &format_repository_sync_log(repo_url))
                .await?;
        } else {
            self.log(attempt_id, "system", &format_repository_clone_log(repo_url))
                .await?;
        }

        // Use WorktreeManager to clone or sync (PAT injected for private repos when configured)
        match self
            .worktree_manager
            .ensure_cloned(&repo_path, repo_url, &pat)
            .await
        {
            Ok(_) => {
                self.log(attempt_id, "system", "Repository synced successfully")
                    .await?;

                // Update project repository_url if not set
                if project.repository_url.is_none() {
                    self.update_project_repo_url(task.project_id, repo_url)
                        .await?;
                }

                if let Err(e) = self
                    .maybe_auto_link_project_gitlab_configuration(
                        attempt_id,
                        task.project_id,
                        repo_url,
                    )
                    .await
                {
                    warn!(
                        "Failed to auto-link GitLab configuration for project {} after import: {}",
                        task.project_id, e
                    );
                    let _ = self
                        .log(
                            attempt_id,
                            "system",
                            "Warning: could not auto-link GitLab project configuration. Auto-merge may require manual GitLab link.",
                        )
                        .await;
                }

                // Project type: respect user selection (metadata.project_type) or auto-detect
                let files = self.list_repo_files(&repo_path);
                let detected_type = ProjectTypeDetector::detect_from_files(&files);
                self.log(
                    attempt_id,
                    "system",
                    &format!("Detected project type: {:?}", detected_type),
                )
                .await?;

                let effective_type = metadata.project_type.unwrap_or(detected_type);
                if metadata.project_type.is_none() {
                    self.update_project_type(task.project_id, detected_type)
                        .await?;
                } else {
                    self.log(
                        attempt_id,
                        "system",
                        &format!(
                            "Using user-selected project type: {:?} (overriding detected {:?})",
                            effective_type, detected_type
                        ),
                    )
                    .await?;
                }

                // Spawn agent to analyze project structure and produce architecture
                self.log(
                    attempt_id,
                    "system",
                    "Spawning agent to analyze project structure and services...",
                )
                .await?;
                if let Err(e) = self
                    .run_import_analysis_agent(
                        attempt_id,
                        &repo_path,
                        task,
                        &project,
                        &project_settings,
                        effective_type,
                    )
                    .await
                {
                    warn!(
                        "Agent analysis failed for GitLab import (continuing with heuristic): {}",
                        e
                    );
                    let _ = self
                        .log(
                            attempt_id,
                            "system",
                            &format!(
                                "Warning: agent analysis failed ({}). Using heuristic architecture.",
                                e
                            ),
                        )
                        .await;
                } else if let Some(arch) = self.extract_import_analysis_from_repo(&repo_path).await
                {
                    sqlx::query(
                        "UPDATE projects SET architecture_config = $2, updated_at = NOW() WHERE id = $1",
                    )
                    .bind(task.project_id)
                    .bind(&arch)
                    .execute(&self.db_pool)
                    .await
                    .context("Failed to persist agent-generated architecture")?;
                    self.log(
                        attempt_id,
                        "system",
                        "Updated System Architecture from agent analysis.",
                    )
                    .await?;
                }

                self.log(
                    attempt_id,
                    "system",
                    "Generating initial System Architecture and PRD drafts...",
                )
                .await?;
                // Re-fetch project so bootstrap sees agent-updated architecture (if any)
                let project_for_bootstrap = self.fetch_project(task.project_id).await?;
                if let Err(e) = self
                    .bootstrap_project_context_after_init(
                        task,
                        &project_for_bootstrap,
                        attempt_id,
                        &repo_path,
                        effective_type,
                        true,
                    )
                    .await
                {
                    warn!(
                        "Failed to bootstrap project context after GitLab import for project {}: {}",
                        project.id, e
                    );
                    let _ = self
                        .log(
                            attempt_id,
                            "system",
                            "Warning: auto-generation of architecture/PRD drafts failed; continue manually in Project Detail tabs.",
                        )
                        .await;
                }

                // Update attempt status to Success
                self.update_status(attempt_id, AttemptStatus::Success)
                    .await?;
                self.mark_task_completed(task_id).await?;
                self.log(
                    attempt_id,
                    "system",
                    "✅ GitLab import completed successfully",
                )
                .await?;

                // Note: No diff capture for import tasks - they just clone existing repos
            }
            Err(e) => {
                let error_msg = format!("Failed to clone/sync repository: {}", e);
                self.log(attempt_id, "stderr", &error_msg).await?;
                self.fail_attempt(attempt_id, &error_msg).await?;
                self.mark_task_failed(task_id, &error_msg).await?;
                bail!("GitLab import failed: {}", e);
            }
        }

        Ok(())
    }

    /// Run agent to analyze imported project structure and produce .acpms/import-analysis.json
    async fn run_import_analysis_agent(
        &self,
        attempt_id: Uuid,
        repo_path: &Path,
        _task: &Task,
        project: &Project,
        project_settings: &ProjectSettings,
        project_type: ProjectType,
    ) -> Result<()> {
        let skill_content = get_skill_content("init-import-analyze", Some(repo_path));
        let instruction = format!(
            r#"# Analyze Imported Project

You are analyzing a repository that was just cloned from GitLab. The project is at: {}

## Project
- **Name**: {}
- **Type**: {:?}

## Task
Follow the init-import-analyze skill. Analyze the directory structure, identify services/components, evaluate the current state, and write `.acpms/import-analysis.json` with the architecture (nodes, edges) and assessment.

## Skill: init-import-analyze
```text
{}
```

## Output
Create `.acpms/import-analysis.json` with valid JSON. Do not modify any source code. Read-only analysis only.
"#,
            repo_path.display(),
            project.name,
            project_type,
            skill_content
        );

        let (provider, provider_env) = self.resolve_agent_cli(attempt_id).await?;
        self.set_attempt_executor(attempt_id, provider).await?;

        let task_timeout = Duration::from_secs(project_settings.timeout_mins as u64 * 60);
        let (_cancel_tx, cancel_rx) = watch::channel(false);

        let env_vars = provider_env.unwrap_or_default();

        let result = tokio::time::timeout(
            task_timeout,
            self.execute_agent(
                attempt_id,
                repo_path,
                &instruction,
                cancel_rx,
                provider,
                Some(env_vars),
            ),
        )
        .await;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => bail!(
                "Import analysis timed out after {} mins",
                project_settings.timeout_mins
            ),
        }
    }

    /// Extract architecture config from .acpms/import-analysis.json if present
    async fn extract_import_analysis_from_repo(
        &self,
        repo_path: &Path,
    ) -> Option<serde_json::Value> {
        let analysis_path = repo_path.join(".acpms").join("import-analysis.json");
        if !analysis_path.exists() {
            return None;
        }

        let content = match std::fs::read_to_string(&analysis_path) {
            Ok(c) => c,
            Err(_) => return None,
        };

        let parsed: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return None,
        };

        let arch = parsed.get("architecture")?;
        let nodes = arch.get("nodes")?.as_array()?;
        let edges = arch.get("edges")?.as_array()?;

        if nodes.is_empty() && edges.is_empty() {
            return None;
        }

        Some(serde_json::json!({
            "nodes": nodes,
            "edges": edges
        }))
    }

    /// Resolve repo path with collision handling: use slug, or slug-2, slug-3 when path exists.
    /// - `repo_url`: Some = allow reusing existing path if same repo; None = only use non-existing path.
    async fn resolve_repo_path_with_collision(
        &self,
        attempt_id: Uuid,
        project_id: Uuid,
        metadata: &serde_json::Value,
        project_name: &str,
        repo_url: Option<&str>,
    ) -> Result<std::path::PathBuf> {
        use acpms_db::models::resolve_project_slug;
        use tokio::process::Command;

        let base = self.worktree_manager.base_path().await;

        if let Some(rel) = metadata.get("repo_relative_path").and_then(|v| v.as_str()) {
            if !rel.is_empty() {
                return Ok(base.join(rel));
            }
        }

        let base_slug = resolve_project_slug(metadata, project_name);
        let mut n = 1u32;
        loop {
            let candidate = if n == 1 {
                base_slug.clone()
            } else {
                format!("{}-{}", base_slug, n)
            };
            let path = base.join(&candidate);

            if !path.exists() {
                self.update_project_repo_relative_path(project_id, &candidate)
                    .await?;
                if n > 1 {
                    self.log(
                        attempt_id,
                        "system",
                        &format!("Path {} taken, using {}", base_slug, candidate),
                    )
                    .await?;
                }
                return Ok(path);
            }

            if let Some(url) = repo_url {
                if path.join(".git").exists() {
                    let out = Command::new("git")
                        .args(["remote", "get-url", "origin"])
                        .current_dir(&path)
                        .output()
                        .await?;
                    if out.status.success() {
                        let existing = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if crate::worktree::repo_url_matches(&existing, url) {
                            self.update_project_repo_relative_path(project_id, &candidate)
                                .await?;
                            return Ok(path);
                        }
                    }
                }
            }

            n += 1;
            if n > 999 {
                anyhow::bail!(
                    "Could not find available path for project (tried {}-1..{}-999)",
                    base_slug,
                    base_slug
                );
            }
        }
    }

    /// Resolve PAT for clone: use system PAT when repo host matches configured URL (GitLab hoặc GitHub).
    /// Returns empty string for public repos or when PAT is not configured.
    async fn resolve_repo_pat_for_clone(&self, attempt_id: Uuid, repo_url: &str) -> Result<String> {
        let settings = match self.fetch_system_settings().await {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to fetch system settings for clone PAT: {}", e);
                return Ok(String::new());
            }
        };

        let Some((repo_host, _)) = parse_repo_host_and_path(repo_url) else {
            return Ok(String::new());
        };

        let Some(configured_host) = parse_host_from_urlish(&settings.gitlab_url) else {
            return Ok(String::new());
        };

        if repo_host.eq_ignore_ascii_case(&configured_host) {
            if let Some(enc) = settings.gitlab_pat_encrypted.as_ref() {
                match self.decrypt_value(enc) {
                    Ok(pat) => {
                        self.log(
                            attempt_id,
                            "system",
                            "Using system PAT for private repository access.",
                        )
                        .await?;
                        return Ok(pat);
                    }
                    Err(e) => warn!("Failed to decrypt PAT: {}", e),
                }
            } else {
                self.log(
                    attempt_id,
                    "system",
                    "No PAT configured; cloning as public (may fail for private repos).",
                )
                .await?;
            }
        }

        Ok(String::new())
    }

    async fn maybe_auto_link_project_gitlab_configuration(
        &self,
        attempt_id: Uuid,
        project_id: Uuid,
        repo_url: &str,
    ) -> Result<()> {
        let already_linked: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM gitlab_configurations WHERE project_id = $1
            )
            "#,
        )
        .bind(project_id)
        .fetch_one(&self.db_pool)
        .await
        .context("Failed to check existing project GitLab configuration")?;

        if already_linked {
            return Ok(());
        }

        let settings = self.fetch_system_settings().await?;
        let Some(gitlab_pat_encrypted) = settings.gitlab_pat_encrypted.as_ref() else {
            self.log(
                attempt_id,
                "system",
                "Skipping GitLab auto-link: system GitLab PAT is not configured.",
            )
            .await?;
            return Ok(());
        };

        let gitlab_pat = match self.decrypt_value(gitlab_pat_encrypted) {
            Ok(pat) => pat,
            Err(err) => {
                self.log(
                    attempt_id,
                    "stderr",
                    &format!(
                        "Skipping GitLab auto-link: failed to decrypt system GitLab PAT: {}",
                        err
                    ),
                )
                .await?;
                return Ok(());
            }
        };

        let Some((repo_host, repo_path)) = parse_repo_host_and_path(repo_url) else {
            self.log(
                attempt_id,
                "system",
                "Skipping GitLab auto-link: could not parse REPO_URL into host/path.",
            )
            .await?;
            return Ok(());
        };

        let Some(configured_host) = parse_host_from_urlish(&settings.gitlab_url) else {
            self.log(
                attempt_id,
                "stderr",
                "Skipping GitLab auto-link: configured GitLab URL has invalid host.",
            )
            .await?;
            return Ok(());
        };

        if !repo_host.eq_ignore_ascii_case(&configured_host) {
            self.log(
                attempt_id,
                "system",
                &format!(
                    "Skipping GitLab auto-link: repo host `{}` does not match configured GitLab host `{}`.",
                    repo_host, configured_host
                ),
            )
            .await?;
            return Ok(());
        }

        let client = acpms_gitlab::GitLabClient::new(&settings.gitlab_url, &gitlab_pat)
            .context("Failed to initialize GitLab client for auto-link")?;

        let gitlab_project = match client.get_project_by_path(&repo_path).await {
            Ok(project) => project,
            Err(err) => {
                self.log(
                    attempt_id,
                    "system",
                    &format!(
                        "Skipping GitLab auto-link: could not resolve project by path `{}`: {}",
                        repo_path, err
                    ),
                )
                .await?;
                return Ok(());
            }
        };

        sqlx::query(
            r#"
            INSERT INTO gitlab_configurations
                (project_id, gitlab_project_id, base_url, pat_encrypted, webhook_secret)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (project_id)
            DO UPDATE SET
                gitlab_project_id = EXCLUDED.gitlab_project_id,
                base_url = EXCLUDED.base_url,
                updated_at = NOW()
            "#,
        )
        .bind(project_id)
        .bind(gitlab_project.id as i64)
        .bind(&settings.gitlab_url)
        .bind("GLOBAL")
        .bind(Uuid::new_v4().to_string())
        .execute(&self.db_pool)
        .await
        .context("Failed to persist auto-linked GitLab configuration")?;

        self.log(
            attempt_id,
            "system",
            &format!(
                "✅ Auto-linked GitLab project configuration (gitlab_project_id={})",
                gitlab_project.id
            ),
        )
        .await?;

        Ok(())
    }

    /// Execute from-scratch init task (complex initialization).
    ///
    /// ## Flow
    /// 1. Create attempt record
    /// 2. Fetch GitLab PAT from system settings
    /// 3. Prepare local project directory
    /// 4. Load agent prompt template
    /// 5. Execute agent with PAT injection
    /// 6. Extract repository URL from agent output
    /// 7. Update project repository_url
    /// 8. Mark task completed
    async fn execute_from_scratch(
        &self,
        task_id: Uuid,
        task: &Task,
        metadata: &InitTaskMetadata,
        existing_attempt_id: Option<Uuid>,
    ) -> Result<()> {
        info!("Starting execute_from_scratch for task_id: {}", task_id);

        // Use existing attempt_id if provided, otherwise create new one
        let attempt_id = match existing_attempt_id {
            Some(id) => {
                info!("Using existing attempt_id: {}", id);
                id
            }
            None => {
                let id = self
                    .create_attempt(task_id)
                    .await
                    .context("Failed to create attempt")?;
                info!("Created new attempt_id: {}", id);
                id
            }
        };

        // Update attempt status to Running
        self.update_status(attempt_id, AttemptStatus::Running)
            .await?;

        // Update task status to InProgress
        sqlx::query("UPDATE tasks SET status = 'in_progress', updated_at = NOW() WHERE id = $1")
            .bind(task_id)
            .execute(&self.db_pool)
            .await
            .context("Failed to update task status to in_progress")?;

        // Get project info
        let project = self
            .fetch_project(task.project_id)
            .await
            .context("Failed to fetch project")?;
        info!("Fetched project: {}", project.name);

        // Fetch project settings for timeout configuration
        let project_settings = self
            .fetch_project_settings(task.project_id)
            .await
            .unwrap_or_default();
        let task_timeout = Duration::from_secs(project_settings.timeout_mins as u64 * 60);
        info!(
            "Using task timeout: {:?} (from project settings)",
            task_timeout
        );

        let visibility = metadata.visibility.as_deref().unwrap_or("private");

        // Fetch GitLab settings directly from database
        let settings = self
            .fetch_system_settings()
            .await
            .context("Failed to fetch system settings")?;
        info!("Fetched system settings");

        // Get encrypted PAT and decrypt it
        let gitlab_pat_encrypted = settings
            .gitlab_pat_encrypted
            .ok_or_else(|| anyhow::anyhow!("GitLab PAT not configured in system settings"))?;
        let gitlab_pat = self
            .decrypt_value(&gitlab_pat_encrypted)
            .context("Failed to decrypt GitLab PAT")?;
        info!("GitLab PAT decrypted successfully");

        // Resolve worktree path (slug only, with collision handling: slug-2, slug-3, ...)
        let worktree_path = self
            .resolve_repo_path_with_collision(
                attempt_id,
                task.project_id,
                &project.metadata,
                &project.name,
                None, // from_scratch: no repo_url, only non-existing paths
            )
            .await?;
        info!("Creating worktree directory: {:?}", worktree_path);

        let slug_safe = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");
        if project
            .metadata
            .get("slug")
            .and_then(|v| v.as_str())
            .is_none()
        {
            if let Err(e) = self.store_project_slug(project.id, slug_safe).await {
                warn!("Failed to persist project slug (non-fatal): {}", e);
            }
        }

        // Create execution_process when orchestrator created the attempt (no server-created record)
        if existing_attempt_id.is_none() {
            if let Err(e) = self
                .create_execution_process(attempt_id, Some(&worktree_path), Some("main"))
                .await
            {
                warn!(
                    attempt_id = %attempt_id,
                    error = %e,
                    "Failed to create execution process for from-scratch init (follow-up may be disabled)"
                );
            }
        }

        fs::create_dir_all(&worktree_path)
            .with_context(|| format!("Failed to create directory: {:?}", worktree_path))?;
        info!("Worktree directory created successfully");

        // --- Native Git Init Fallback ---
        info!("Initializing Git repository natively");
        let init_output = Command::new("git")
            .current_dir(&worktree_path)
            .arg("init")
            .arg("-b")
            .arg("main")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git init")?;

        if !init_output.status.success() {
            anyhow::bail!(
                "git init failed: {}",
                String::from_utf8_lossy(&init_output.stderr)
            );
        }

        self.worktree_manager
            .setup_git_config(&worktree_path)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to setup git config for init repo: {}", e);
            });

        let commit_output = Command::new("git")
            .current_dir(&worktree_path)
            .arg("commit")
            .arg("--allow-empty")
            .arg("-m")
            .arg("Initial commit from Orchestrator Init Flow")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute initial commit")?;

        if !commit_output.status.success() {
            tracing::warn!(
                "git initial commit failed: {}",
                String::from_utf8_lossy(&commit_output.stderr)
            );
        }
        // --- End Native Git Init ---

        let skill_context = self.build_skill_instruction_context(
            task,
            &project_settings,
            project.project_type,
            Some(worktree_path.as_path()),
        );
        if let Err(error) = self
            .persist_skill_instruction_context(attempt_id, &skill_context, "init_from_scratch")
            .await
        {
            warn!(
                attempt_id = %attempt_id,
                error = %error,
                "Failed to persist skill instruction metadata for from-scratch init attempt"
            );
        }
        if let Err(error) = self.log_loaded_skills(attempt_id, &skill_context).await {
            warn!(
                attempt_id = %attempt_id,
                error = %error,
                "Failed to append skill timeline log for from-scratch init attempt"
            );
        }

        let skill_block = skill_context.block.clone();

        // Store worktree path in attempt metadata for later use (resume/review)
        self.store_worktree_path(attempt_id, &worktree_path).await?;
        // From-scratch init starts from an empty/new repository context.
        // Pin diff base to empty tree so final diff captures all scaffolded files.
        self.store_diff_base_commit_value(attempt_id, GIT_EMPTY_TREE_HASH)
            .await?;

        // Prepare reference files if provided (download from storage, extract to .acpms-refs/)
        let has_refs = self
            .prepare_reference_files(
                metadata.reference_keys.as_deref().unwrap_or(&[]),
                &worktree_path,
                attempt_id,
            )
            .await
            .unwrap_or_else(|e| {
                warn!(
                    attempt_id = %attempt_id,
                    error = %e,
                    "Failed to prepare reference files (continuing without them)"
                );
                false
            });

        // Build agent instruction for from-scratch initialization
        let refs_section = if has_refs {
            r#"

## Reference Materials

Reference files have been placed in `.acpms-refs/` directory. Use them to:
- Understand the desired project structure and patterns
- Replicate similar architecture, naming, or configuration
- Follow design mockups or specifications

Read these files before scaffolding the project."#
        } else {
            ""
        };

        let stack_section = {
            let mut s = String::new();
            if let Some(ref selections) = metadata.stack_selections {
                if !selections.is_empty() {
                    s.push_str("\n\n## Required Stack By Layer\n");
                    for sel in selections {
                        let layer_name = sel.layer.display_name();
                        let stack = sel.stack.trim();
                        if !stack.is_empty() {
                            s.push_str("- ");
                            s.push_str(layer_name);
                            s.push_str(": **");
                            s.push_str(stack);
                            s.push_str("**\n");
                        }
                    }
                    s.push_str(
                        "- Keep these layer choices unless technically impossible for this project.\n",
                    );
                    s.push_str(
                        "- If any layer must change, explain the reason and the replacement clearly.\n",
                    );
                }
            } else if let Some(ref stack) = metadata.preferred_stack {
                let stack = stack.trim();
                if !stack.is_empty() {
                    s.push_str("\n\n## Required Tech Stack\n");
                    s.push_str("- Use **");
                    s.push_str(stack);
                    s.push_str("** as the primary stack/framework for this project.\n");
                    s.push_str(
                        "- Do not switch to a different framework unless it is technically impossible.\n",
                    );
                    s.push_str(
                        "- If a tradeoff is required, explain it clearly in the final output.\n",
                    );
                }
            }
            s
        };

        let is_github = parse_host_from_urlish(&settings.gitlab_url)
            .map(|h| h.contains("github.com"))
            .unwrap_or(false);
        let provider = if is_github { "GitHub" } else { "GitLab" };

        let instruction = format!(
            r#"# Initialize Project from Scratch

You are tasked with creating a new {} project and initializing it with a basic project structure.

## Project Details

- **Name**: {}
- **Description**: {}
- **Slug**: {}
- **Visibility**: {}

## Tasks

1. **Create {} Repository**:
   - GITLAB_URL points to {} — use the appropriate API (GitHub API when github.com, GitLab API otherwise)
   - PAT is available in environment variable `GITLAB_PAT`
   - Base URL is available in environment variable `GITLAB_URL`
   - Set project name to "{}"
   - Set project path/repo name (slug) to "{}"
   - Set visibility to "{}"

2. **Local repo, push, REPO_URL**: Follow init-source-repository skill (git init, README/.gitignore, commit, add remote, push, output REPO_URL).

## Important Notes

- Use the environment variables for credentials (GITLAB_PAT, GITLAB_URL)
- Do not hardcode any credentials
- Create a meaningful README.md with the project description
- Follow active skills for execution/reporting requirements{}{}{}
"#,
            provider,
            project.name,
            task.description.as_deref().unwrap_or(""),
            slug_safe,
            visibility,
            provider,
            provider,
            project.name,
            slug_safe,
            visibility,
            skill_block,
            stack_section,
            refs_section
        );

        self.log(
            attempt_id,
            "system",
            &format!("Starting from-scratch init: {}", project.name),
        )
        .await?;

        // Prepare environment variables
        let mut env_vars = HashMap::new();
        env_vars.insert("GITLAB_PAT".to_string(), gitlab_pat.clone());
        env_vars.insert("GITLAB_URL".to_string(), settings.gitlab_url.clone());

        // Resolve selected agent CLI and verify it is ready (Claude / Codex / Gemini)
        info!(attempt_id = %attempt_id, "Resolving agent CLI...");
        let (provider, provider_env) = self.resolve_agent_cli(attempt_id).await?;
        info!(attempt_id = %attempt_id, provider = %provider.as_str(), "Agent CLI resolved");
        self.set_attempt_executor(attempt_id, provider).await?;

        if let Some(extra_env) = provider_env {
            env_vars.extend(extra_env);
        }
        let agent_env = env_vars.clone();

        let status = match provider {
            AgentCliProvider::ClaudeCode => {
                // Execute with SDK mode (bidirectional protocol)
                info!(
                    "Spawning Claude agent in SDK mode for attempt_id: {}",
                    attempt_id
                );
                self.log(attempt_id, "system", "Starting agent...").await?;

                // Load agent settings from project
                let agent_settings = self.load_agent_settings(attempt_id).await?;

                // Live input channel for send_input API (POST /attempts/:id/input)
                let (session_input_sender, claude_input_rx) = mpsc::unbounded_channel::<String>();

                // Store router metadata in attempt
                if agent_settings.enable_router_service {
                    let router_version = &agent_settings.router_version;
                    sqlx::query(
                        r#"UPDATE task_attempts
                           SET metadata = metadata || jsonb_build_object(
                               'router_enabled', true,
                               'router_version', $2::text
                           )
                           WHERE id = $1"#,
                    )
                    .bind(attempt_id)
                    .bind(router_version)
                    .execute(&self.db_pool)
                    .await?;
                }

                let spawned = match self
                    .claude_client
                    .spawn_session_sdk(
                        &worktree_path,
                        &instruction,
                        attempt_id,
                        Some(agent_env.clone()),
                        Some(self.approval_service.clone()),
                        Some(self.db_pool.clone()),
                        Some(self.broadcast_tx.clone()),
                        Some(&agent_settings),
                        Some(claude_input_rx),
                        Some(crate::claude::ClaudeRuntimeSkillConfig {
                            repo_path: worktree_path.clone(),
                            skill_knowledge: self.skill_knowledge.clone(),
                        }),
                    )
                    .await
                {
                    Ok(spawned) => {
                        info!("Claude agent spawned successfully in SDK mode");
                        self.log(
                            attempt_id,
                            "system",
                            "Claude agent spawned successfully (SDK mode)",
                        )
                        .await?;
                        spawned
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to spawn Claude agent: {:?}", e);
                        error!("{}", error_msg);
                        self.log(attempt_id, "stderr", &error_msg).await?;

                        // Check if router was enabled and might have failed
                        let failure_reason = if agent_settings.enable_router_service {
                            "router_or_agent_error"
                        } else {
                            "agent_error"
                        };

                        self.fail_attempt_with_reason(attempt_id, failure_reason, &e.to_string())
                            .await?;
                        return Err(e.context("Failed to spawn Claude agent"));
                    }
                };

                let SpawnedAgent {
                    child,
                    interrupt_sender,
                    msg_store,
                    ..
                } = spawned;

                // Note: MsgStore logging removed to avoid duplicate raw JSON entries
                // Logs are already saved via on_non_control in ClaudeAgentClient with proper extraction

                // Register session for send_input API and termination control
                let child_arc = Arc::new(Mutex::new(Some(child)));
                {
                    let session = ActiveSession {
                        interrupt_sender,
                        child: child_arc.clone(),
                        input_sender: Some(session_input_sender),
                    };
                    self.active_sessions
                        .lock()
                        .await
                        .insert(attempt_id, session);
                }
                let _cleanup_guard = scopeguard::guard((), {
                    let sessions = self.active_sessions.clone();
                    move |_| {
                        tokio::spawn(async move {
                            sessions.lock().await.remove(&attempt_id);
                            debug!("Cleaned up init session for attempt {}", attempt_id);
                        });
                    }
                });

                let mut child_opt = child_arc.lock().await.take();
                let child_ref = child_opt
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Child process not available"))?;

                let Some(store) = msg_store else {
                    let error_msg =
                        "Claude SDK session missing message store; cannot detect completion";
                    error!("{}", error_msg);
                    self.log(attempt_id, "stderr", error_msg).await?;
                    self.fail_attempt(attempt_id, error_msg).await?;
                    return Err(anyhow::anyhow!(error_msg));
                };

                if let Err(e) = self
                    .wait_for_claude_sdk_turn_completion(store, task_timeout)
                    .await
                {
                    self.log(
                        attempt_id,
                        "system",
                        &format!(
                            "Task timed out after {} mins, terminating agent...",
                            project_settings.timeout_mins
                        ),
                    )
                    .await?;
                    let interrupt_sender = self
                        .active_sessions
                        .lock()
                        .await
                        .remove(&attempt_id)
                        .and_then(|s| s.interrupt_sender);
                    let _ =
                        terminate_process(child_ref, interrupt_sender, GRACEFUL_SHUTDOWN_TIMEOUT)
                            .await;
                    self.mark_task_failed(
                        task_id,
                        &format!(
                            "Task execution timed out after {} mins",
                            project_settings.timeout_mins
                        ),
                    )
                    .await?;
                    return Err(e.context(
                        "From-scratch init timed out waiting for Claude SDK turn completion",
                    ));
                }

                if let Some(session) = self.active_sessions.lock().await.get_mut(&attempt_id) {
                    session.input_sender = None;
                }

                let status = match tokio::time::timeout(
                    AGENT_EXIT_TIMEOUT_AFTER_STREAM,
                    child_ref.wait(),
                )
                .await
                {
                    Ok(Ok(status)) => status,
                    Ok(Err(e)) => {
                        let error_msg = format!("Failed to wait for agent process exit: {:?}", e);
                        error!("{}", error_msg);
                        self.log(attempt_id, "stderr", &error_msg).await?;
                        self.fail_attempt(attempt_id, &e.to_string()).await?;
                        return Err(anyhow::anyhow!(
                            "Failed to wait for agent process exit: {}",
                            e
                        ));
                    }
                    Err(_) => {
                        self.log(
                            attempt_id,
                            "stderr",
                            &format!(
                                "Agent process did not exit after Claude SDK turn completion (>{:?}). Forcing shutdown to avoid hang.",
                                AGENT_EXIT_TIMEOUT_AFTER_STREAM
                            ),
                        )
                        .await?;
                        let _ = terminate_process(child_ref, None, GRACEFUL_SHUTDOWN_TIMEOUT).await;
                        child_ref.wait().await.map_err(|e| {
                            anyhow::anyhow!("Failed to wait for forced Claude shutdown: {}", e)
                        })?
                    }
                };

                Some(status)
            }
            AgentCliProvider::OpenAiCodex
            | AgentCliProvider::GeminiCli
            | AgentCliProvider::CursorCli => {
                self.log(
                    attempt_id,
                    "system",
                    &format!("Spawning {} agent...", provider.display_name()),
                )
                .await?;

                // Live input channel for send_input API (POST /attempts/:id/input)
                let (session_input_sender, mut stdio_input_rx) =
                    mpsc::unbounded_channel::<String>();

                let spawned = match provider {
                    AgentCliProvider::OpenAiCodex => {
                        self.codex_client
                            .spawn_session(
                                &worktree_path,
                                &instruction,
                                attempt_id,
                                Some(agent_env.clone()),
                            )
                            .await
                    }
                    AgentCliProvider::GeminiCli => {
                        self.gemini_client
                            .spawn_session(
                                &worktree_path,
                                &instruction,
                                attempt_id,
                                Some(agent_env.clone()),
                            )
                            .await
                    }
                    AgentCliProvider::CursorCli => {
                        self.cursor_client
                            .spawn_session(
                                &worktree_path,
                                &instruction,
                                attempt_id,
                                Some(agent_env.clone()),
                            )
                            .await
                    }
                    AgentCliProvider::ClaudeCode => unreachable!(),
                }?;

                let SpawnedAgent {
                    child,
                    interrupt_sender,
                    interrupt_receiver,
                    ..
                } = spawned;

                // Register session for send_input API and termination control
                let child_arc = Arc::new(Mutex::new(Some(child)));
                {
                    let session = ActiveSession {
                        interrupt_sender,
                        child: child_arc.clone(),
                        input_sender: Some(session_input_sender),
                    };
                    self.active_sessions
                        .lock()
                        .await
                        .insert(attempt_id, session);
                }
                let _cleanup_guard = scopeguard::guard((), {
                    let sessions = self.active_sessions.clone();
                    move |_| {
                        tokio::spawn(async move {
                            sessions.lock().await.remove(&attempt_id);
                            debug!("Cleaned up init session for attempt {}", attempt_id);
                        });
                    }
                });

                let mut child_opt = child_arc.lock().await.take();
                let child_ref = child_opt
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Child process not available"))?;

                // Drain live input to process stdin (Codex/Gemini use stdin for user input)
                if let Some(mut stdin) = child_ref.inner().stdin.take() {
                    let pool = self.db_pool.clone();
                    let tx = self.broadcast_tx.clone();
                    tokio::spawn(async move {
                        while let Some(message) = stdio_input_rx.recv().await {
                            let trimmed = message.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            let to_send = crate::follow_up_utils::wrap_trivial_follow_up(trimmed);
                            let line = format!("{}\n", to_send);
                            if let Err(e) = stdin.write_all(line.as_bytes()).await {
                                let _ = StatusManager::log(
                                    &pool,
                                    &tx,
                                    attempt_id,
                                    "stderr",
                                    &format!("Failed to forward live input to stdin: {}", e),
                                )
                                .await;
                                break;
                            }
                            if let Err(e) = stdin.flush().await {
                                let _ = StatusManager::log(
                                    &pool,
                                    &tx,
                                    attempt_id,
                                    "stderr",
                                    &format!("Failed to flush live input to stdin: {}", e),
                                )
                                .await;
                                break;
                            }
                        }
                    });
                }

                // Stream logs while waiting (non-SDK providers)
                let db_pool = self.db_pool.clone();
                let tx = self.broadcast_tx.clone();

                let run = async {
                    match provider {
                        AgentCliProvider::OpenAiCodex => {
                            self.stream_codex_json_with_interrupt(
                                child_ref,
                                attempt_id,
                                &worktree_path,
                                interrupt_receiver,
                            )
                            .await?;
                        }
                        AgentCliProvider::GeminiCli => {
                            ClaudeClient::stream_logs_with_interrupt(
                                child_ref,
                                interrupt_receiver,
                                move |line, is_stderr| {
                                    let pool = db_pool.clone();
                                    let tx = tx.clone();
                                    let role = if is_stderr { "stderr" } else { "stdout" };
                                    if should_skip_log_line(&line) {
                                        return;
                                    }
                                    let log_content = sanitize_log(&line);
                                    tokio::spawn(async move {
                                        let _ = StatusManager::log(
                                            &pool,
                                            &tx,
                                            attempt_id,
                                            role,
                                            &log_content,
                                        )
                                        .await;
                                    });
                                },
                            )
                            .await?;
                        }
                        AgentCliProvider::CursorCli => {
                            self.stream_cursor_json_with_interrupt(
                                child_ref,
                                attempt_id,
                                &worktree_path,
                                interrupt_receiver,
                            )
                            .await?;
                        }
                        AgentCliProvider::ClaudeCode => unreachable!(),
                    }

                    // The agent finished streaming user-visible output. Close live stdin so
                    // single-turn providers do not sit idle waiting for more input forever.
                    if let Some(session) = self.active_sessions.lock().await.get_mut(&attempt_id) {
                        session.input_sender = None;
                    }

                    // Mirror normal task execution: do not let init attempts hang forever after
                    // the stream has already completed.
                    let status = match tokio::time::timeout(
                        AGENT_EXIT_TIMEOUT_AFTER_STREAM,
                        child_ref.wait(),
                    )
                    .await
                    {
                        Ok(Ok(status)) => Some(status),
                        Ok(Err(err)) => {
                            let msg = format!("Failed to wait for agent process exit: {}", err);
                            self.log(attempt_id, "stderr", &msg).await?;
                            return Err(anyhow::anyhow!(msg));
                        }
                        Err(_) => {
                            self.log(
                                attempt_id,
                                "stderr",
                                &format!(
                                    "Agent process did not exit after stream completion (>{:?}). Forcing shutdown to avoid hang.",
                                    AGENT_EXIT_TIMEOUT_AFTER_STREAM
                                ),
                            )
                            .await?;
                            let _ =
                                terminate_process(child_ref, None, GRACEFUL_SHUTDOWN_TIMEOUT).await;
                            None
                        }
                    };

                    let _ = kill_process_group(child_ref).await;
                    Ok::<_, anyhow::Error>(status)
                };

                let status = match tokio::time::timeout(task_timeout, run).await {
                    Ok(Ok(status)) => status,
                    Ok(Err(e)) => {
                        let error_msg = format!("Agent execution error: {:?}", e);
                        self.log(attempt_id, "stderr", &error_msg).await?;
                        self.fail_attempt(attempt_id, &e.to_string()).await?;
                        // Terminate orphan process to avoid hang (stream failed but child may still run)
                        let interrupt_sender = self
                            .active_sessions
                            .lock()
                            .await
                            .remove(&attempt_id)
                            .and_then(|s| s.interrupt_sender);
                        let _ = terminate_process(
                            child_ref,
                            interrupt_sender,
                            GRACEFUL_SHUTDOWN_TIMEOUT,
                        )
                        .await;
                        return Err(e);
                    }
                    Err(_) => {
                        self.log(
                            attempt_id,
                            "system",
                            &format!(
                                "Task timed out after {} mins, terminating agent...",
                                project_settings.timeout_mins
                            ),
                        )
                        .await?;
                        let interrupt_sender = self
                            .active_sessions
                            .lock()
                            .await
                            .remove(&attempt_id)
                            .and_then(|s| s.interrupt_sender);
                        let _ = terminate_process(
                            child_ref,
                            interrupt_sender,
                            GRACEFUL_SHUTDOWN_TIMEOUT,
                        )
                        .await;
                        self.mark_task_failed(
                            task_id,
                            &format!(
                                "Task execution timed out after {} mins",
                                project_settings.timeout_mins
                            ),
                        )
                        .await?;
                        bail!("From-scratch init timed out after {:?}", task_timeout);
                    }
                };

                status
            }
        };

        if status.map(|value| value.success()).unwrap_or(true) {
            // Flush log buffer so REPO_URL (in stdout/system) is in agent_logs before extraction.
            let _ = crate::agent_log_buffer::flush_agent_log_buffer().await;

            // Extract repo URL: file contract first, then logs, then git remote.
            let repo_url = match self
                .extract_repo_url_from_init_output_file(&worktree_path)
                .await?
            {
                Some(url) => {
                    self.log(
                        attempt_id,
                        "system",
                        "REPO_URL from .acpms/init-output.json (file contract)",
                    )
                    .await?;
                    normalize_repo_url(&url)
                }
                None => match self.extract_repo_url_from_attempt_logs(attempt_id).await? {
                    Some(url) => normalize_repo_url(&url),
                    None => match self
                        .extract_repo_url_from_git_remote(&worktree_path)
                        .await?
                    {
                        Some(url) => {
                            self.log(
                                attempt_id,
                                "system",
                                "REPO_URL not found in logs, using git remote origin URL",
                            )
                            .await?;
                            normalize_repo_url(&url)
                        }
                        None => {
                            let msg = "Agent completed but did not output REPO_URL";
                            self.log(attempt_id, "stderr", msg).await?;
                            self.fail_attempt(attempt_id, msg).await?;
                            self.mark_task_failed(task_id, msg).await?;
                            bail!(msg);
                        }
                    },
                },
            };

            self.update_project_repo_url(task.project_id, &repo_url)
                .await
                .context("Failed to update project repository_url")?;

            match self
                .refresh_repository_context_after_repo_creation(task.project_id, &repo_url)
                .await
            {
                Ok(repository_context) => {
                    self.log(
                        attempt_id,
                        "system",
                        &format!(
                            "Repository access detected automatically: {:?}",
                            repository_context.access_mode
                        ),
                    )
                    .await?;
                }
                Err(error) => {
                    warn!(
                        project_id = %task.project_id,
                        repo_url = %repo_url,
                        error = %error,
                        "Failed to auto-refresh repository context after from-scratch init"
                    );
                    let _ = self
                        .log(
                            attempt_id,
                            "system",
                            "Warning: could not auto-check repository GitOps access after repository creation. You can re-check access from the project page.",
                        )
                        .await;
                }
            }

            self.log(attempt_id, "system", &format!("Repository: {}", repo_url))
                .await?;

            if let Err(e) = self
                .maybe_auto_link_project_gitlab_configuration(
                    attempt_id,
                    task.project_id,
                    &repo_url,
                )
                .await
            {
                warn!(
                    "Failed to auto-link GitLab configuration for project {} after from-scratch init: {}",
                    task.project_id, e
                );
                let _ = self
                    .log(
                        attempt_id,
                        "system",
                        "Warning: could not auto-link GitLab project configuration. Auto-merge may require manual GitLab link.",
                    )
                    .await;
            }

            if let Err(e) = self
                .run_post_init_validation_with_auto_fix(
                    attempt_id,
                    &project,
                    &worktree_path,
                    provider,
                    &agent_env,
                    task_timeout,
                    &project_settings,
                )
                .await
            {
                let msg = format!("Post-init validation failed: {}", e);
                self.log(attempt_id, "stderr", &msg).await?;
                self.fail_attempt(attempt_id, &msg).await?;
                self.mark_task_failed(task_id, &msg).await?;
                bail!(msg);
            }

            match self
                .maybe_run_agent_driven_deploy_validation(
                    attempt_id,
                    task_id,
                    &worktree_path,
                    provider,
                    &agent_env,
                )
                .await
            {
                Ok(Some(preview_target)) => {
                    self.log(
                        attempt_id,
                        "system",
                        &format!("🌐 PREVIEW_TARGET: {}", preview_target),
                    )
                    .await?;
                }
                Ok(None) => {}
                Err(e) => {
                    let msg = format!("Deployment validation failed: {}", e);
                    self.log(attempt_id, "stderr", &msg).await?;
                    self.fail_attempt(attempt_id, &msg).await?;
                    self.mark_task_failed(task_id, &msg).await?;
                    bail!(msg);
                }
            }

            self.log(
                attempt_id,
                "system",
                "Generating initial System Architecture and PRD drafts...",
            )
            .await?;
            if let Err(e) = self
                .bootstrap_project_context_after_init(
                    task,
                    &project,
                    attempt_id,
                    &worktree_path,
                    project.project_type,
                    true,
                )
                .await
            {
                warn!(
                    "Failed to bootstrap project context after from-scratch init for project {}: {}",
                    project.id, e
                );
                let _ = self
                    .log(
                        attempt_id,
                        "system",
                        "Warning: auto-generation of architecture/PRD drafts failed; continue manually in Project Detail tabs.",
                    )
                    .await;
            }

            // Update attempt status to Success
            self.update_status(attempt_id, AttemptStatus::Success)
                .await?;
            self.mark_task_completed(task_id).await?;
            self.log(
                attempt_id,
                "system",
                "✅ From-scratch initialization completed",
            )
            .await?;

            // Save file diffs to S3 for from-scratch init (includes all initial files)
            info!(
                "📸 [DIFF CAPTURE TRIGGER] About to save diffs for attempt {} (from-scratch init)",
                attempt_id
            );
            if let Err(e) = self.save_diffs_to_s3(attempt_id, &worktree_path).await {
                warn!(
                    "📸 [DIFF CAPTURE ERROR] Failed to save diffs for attempt {}: {}",
                    attempt_id, e
                );
                // Don't fail the attempt if diff capture fails
            }
        } else {
            self.fail_attempt(attempt_id, "Agent execution failed")
                .await?;
            self.mark_task_failed(task_id, "Agent execution failed")
                .await?;
            bail!("From-scratch initialization failed");
        }

        Ok(())
    }

    /// Download reference files from storage and extract/copy to worktree/.acpms-refs/
    async fn prepare_reference_files(
        &self,
        ref_keys: &[String],
        worktree_path: &Path,
        attempt_id: Uuid,
    ) -> Result<bool> {
        if ref_keys.is_empty() {
            return Ok(false);
        }

        let refs_dir = worktree_path.join(".acpms-refs");
        fs::create_dir_all(&refs_dir)
            .with_context(|| format!("Failed to create refs dir: {:?}", refs_dir))?;

        self.log(
            attempt_id,
            "system",
            &format!("Preparing {} reference file(s)...", ref_keys.len()),
        )
        .await?;

        let mut prepared = 0;
        for key in ref_keys {
            let bytes = match self.storage_service.download_object_bytes(key).await {
                Ok(b) => b,
                Err(e) => {
                    warn!(
                        attempt_id = %attempt_id,
                        key = %key,
                        error = %e,
                        "Failed to download reference file"
                    );
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!("Failed to download reference {}: {}", key, e),
                    )
                    .await?;
                    continue;
                }
            };

            let filename = key.rsplit('/').next().unwrap_or("ref");

            if key.ends_with(".zip") {
                if let Err(e) = extract_zip_to_dir(&bytes, &refs_dir) {
                    warn!(
                        attempt_id = %attempt_id,
                        key = %key,
                        error = %e,
                        "Failed to extract zip"
                    );
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!("Failed to extract {}: {}", filename, e),
                    )
                    .await?;
                } else {
                    prepared += 1;
                }
            } else if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
                if let Err(e) = extract_tar_gz_to_dir(&bytes, &refs_dir) {
                    warn!(
                        attempt_id = %attempt_id,
                        key = %key,
                        error = %e,
                        "Failed to extract tar.gz"
                    );
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!("Failed to extract {}: {}", filename, e),
                    )
                    .await?;
                } else {
                    prepared += 1;
                }
            } else {
                let out_path = refs_dir.join(sanitize_ref_filename(filename));
                if let Err(e) = fs::write(&out_path, &bytes) {
                    warn!(
                        attempt_id = %attempt_id,
                        path = ?out_path,
                        error = %e,
                        "Failed to write reference file"
                    );
                    self.log(
                        attempt_id,
                        "stderr",
                        &format!("Failed to write {}: {}", filename, e),
                    )
                    .await?;
                } else {
                    prepared += 1;
                }
            }
        }

        if prepared > 0 {
            self.log(
                attempt_id,
                "system",
                &format!("✅ Prepared {} reference file(s) in .acpms-refs/", prepared),
            )
            .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn refresh_repository_context_after_repo_creation(
        &self,
        project_id: Uuid,
        repo_url: &str,
    ) -> Result<RepositoryContext> {
        let repository_context = self.classify_repository_context_for_repo(repo_url).await;
        self.update_project_repository_context(project_id, &repository_context)
            .await
            .context("Failed to persist repository context after repository creation")?;
        Ok(repository_context)
    }

    async fn classify_repository_context_for_repo(&self, repo_url: &str) -> RepositoryContext {
        let settings = self.fetch_system_settings().await.ok();
        let provider = detect_repository_provider_for_repo(
            repo_url,
            settings.as_ref().map(|value| value.gitlab_url.as_str()),
        );
        let clone_error: Option<String> = self
            .check_repository_cloneable_with_retry(repo_url, 3, Duration::from_secs(2))
            .await;
        let can_clone = clone_error.is_none();

        let context_result: Result<RepositoryContext> = match provider {
            RepositoryProvider::Github => {
                self.preflight_github_repository_context(repo_url, can_clone, settings.as_ref())
                    .await
            }
            RepositoryProvider::Gitlab => {
                self.preflight_gitlab_repository_context(repo_url, can_clone, settings.as_ref())
                    .await
            }
            RepositoryProvider::Unknown => Ok(unknown_repository_context(
                provider,
                repo_url,
                can_clone,
                "Could not infer repository provider from URL or configured instance.",
            )),
        };

        repository_context_with_clone_result(
            context_result.unwrap_or_else(|error| {
                failed_repository_context(provider, repo_url, can_clone, error.to_string())
            }),
            clone_error,
        )
    }

    async fn preflight_github_repository_context(
        &self,
        repo_url: &str,
        can_clone: bool,
        settings: Option<&SystemSettings>,
    ) -> Result<RepositoryContext> {
        let pat: String = self
            .get_system_pat_for_repo(repo_url)
            .await
            .unwrap_or_default();
        if pat.trim().is_empty() {
            return Ok(unauthenticated_repository_context(
                RepositoryProvider::Github,
                repo_url,
                can_clone,
                "No GitHub token configured for this repository host. Capability cannot be verified.",
            ));
        }

        let (repo_host, repo_path) = parse_repo_host_and_path(repo_url)
            .ok_or_else(|| anyhow::anyhow!("Invalid GitHub repository URL"))?;
        let (owner, repo) = parse_github_owner_repo(&repo_path)
            .ok_or_else(|| anyhow::anyhow!("Invalid GitHub repository URL"))?;
        let client_base_url = settings
            .and_then(|configured| {
                parse_host_from_urlish(&configured.gitlab_url).and_then(|configured_host| {
                    configured_host
                        .eq_ignore_ascii_case(&repo_host)
                        .then_some(configured.gitlab_url.as_str())
                })
            })
            .unwrap_or("https://github.com");

        let client = acpms_github::GitHubClient::new(client_base_url, &pat)
            .context("Failed to initialize GitHub client")?;
        let repository = client
            .get_repo(&owner, &repo)
            .await
            .context("Failed to fetch GitHub repository metadata")?;

        let permissions = repository.permissions.unwrap_or_default();
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
        let writable_repository_url = can_push.then(|| repository.html_url.clone());

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
            upstream_repository_url: Some(repository.html_url.clone()),
            writable_repository_url: writable_repository_url.clone(),
            effective_clone_url: writable_repository_url.or_else(|| Some(repository.html_url)),
            default_branch: Some(repository.default_branch),
            upstream_project_id: Some(repository.id as i64),
            writable_project_id: can_push.then_some(repository.id as i64),
            verified_at: Some(Utc::now()),
        })
    }

    async fn preflight_gitlab_repository_context(
        &self,
        repo_url: &str,
        can_clone: bool,
        settings: Option<&SystemSettings>,
    ) -> Result<RepositoryContext> {
        let Some(settings) = settings else {
            return Ok(unknown_repository_context(
                RepositoryProvider::Gitlab,
                repo_url,
                can_clone,
                "System repository host is not configured. Capability cannot be verified.",
            ));
        };

        let pat: String = self
            .get_system_pat_for_repo(repo_url)
            .await
            .unwrap_or_default();
        if pat.trim().is_empty() {
            return Ok(unauthenticated_repository_context(
                RepositoryProvider::Gitlab,
                repo_url,
                can_clone,
                "No GitLab token configured for this repository host. Capability cannot be verified.",
            ));
        }

        let (_, repo_path) = parse_repo_host_and_path(repo_url)
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
        let writable_repository_url = can_push.then(|| project.web_url.clone());

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
            upstream_repository_url: Some(project.web_url.clone()),
            writable_repository_url: writable_repository_url.clone(),
            effective_clone_url: writable_repository_url.or_else(|| Some(project.web_url)),
            default_branch: project.default_branch,
            upstream_project_id: Some(project.id as i64),
            writable_project_id: can_push.then_some(project.id as i64),
            verified_at: Some(Utc::now()),
        })
    }

    async fn check_repository_cloneable_with_retry(
        &self,
        repo_url: &str,
        attempts: usize,
        delay: Duration,
    ) -> Option<String> {
        let attempts = attempts.max(1);

        for attempt_index in 0..attempts {
            match self.check_repository_cloneable(repo_url).await {
                Ok(()) => return None,
                Err(error) if attempt_index + 1 == attempts => return Some(error),
                Err(_) => tokio::time::sleep(delay).await,
            }
        }

        Some("Repository did not become cloneable after retrying.".to_string())
    }

    async fn check_repository_cloneable(&self, repo_url: &str) -> std::result::Result<(), String> {
        let pat: String = self
            .get_system_pat_for_repo(repo_url)
            .await
            .unwrap_or_default();
        let auth_url = build_authenticated_repo_url(repo_url, &pat);

        let output = tokio::time::timeout(
            Duration::from_secs(30),
            git_command_non_interactive()
                .args(["ls-remote", "--exit-code", &auth_url])
                .output(),
        )
        .await
        .map_err(|_| {
            "Repository check timed out (30s). Check network or repo accessibility.".to_string()
        })?
        .map_err(|error| format!("Failed to run git ls-remote: {}", error))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = stderr.trim();
        let error = if msg.is_empty() {
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

        Err(error.to_string())
    }
}

fn sanitize_ref_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "ref.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

fn extract_zip_to_dir(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    use zip::ZipArchive;
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("Invalid zip archive")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;
        let name = file.name().to_string();
        if name.contains("..") {
            continue;
        }
        let out_path = dest_dir.join(&name);
        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = std::io::BufWriter::new(
                std::fs::File::create(&out_path)
                    .with_context(|| format!("Failed to create {:?}", out_path))?,
            );
            std::io::copy(&mut file, &mut out_file)?;
        }
    }
    Ok(())
}

fn extract_tar_gz_to_dir(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    let decoder = GzDecoder::new(bytes);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(dest_dir)
        .context("Failed to unpack tar.gz")?;
    Ok(())
}

fn detect_repository_provider_for_repo(
    repo_url: &str,
    configured_url: Option<&str>,
) -> RepositoryProvider {
    let Some((repo_host, _)) = parse_repo_host_and_path(repo_url) else {
        return RepositoryProvider::Unknown;
    };
    let repo_host = repo_host.to_ascii_lowercase();

    if repo_host.contains("github") {
        return RepositoryProvider::Github;
    }
    if repo_host.contains("gitlab") {
        return RepositoryProvider::Gitlab;
    }

    if let Some(configured_url) = configured_url {
        if let Some(configured_host) = parse_host_from_urlish(configured_url) {
            if configured_host.eq_ignore_ascii_case(&repo_host) {
                if configured_url.to_ascii_lowercase().contains("github") {
                    return RepositoryProvider::Github;
                }

                return RepositoryProvider::Gitlab;
            }
        }
    }

    RepositoryProvider::Unknown
}

fn parse_github_owner_repo(repo_path: &str) -> Option<(String, String)> {
    let mut segments = repo_path
        .split('/')
        .filter(|segment| !segment.trim().is_empty());
    let owner = segments.next()?.trim().to_string();
    let repo = segments.next()?.trim().trim_end_matches(".git").to_string();

    if owner.is_empty() || repo.is_empty() {
        None
    } else {
        Some((owner, repo))
    }
}

fn unauthenticated_repository_context(
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

fn failed_repository_context(
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

fn unknown_repository_context(
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

fn repository_context_with_clone_result(
    mut context: RepositoryContext,
    clone_error: Option<String>,
) -> RepositoryContext {
    if let Some(error) = clone_error {
        context.access_mode = RepositoryAccessMode::Unknown;
        context.verification_status = RepositoryVerificationStatus::Failed;
        context.verification_error = Some(error);
        context.can_clone = false;
        context.can_push = false;
        context.can_open_change_request = false;
        context.can_merge = false;
        context.can_manage_webhooks = false;
        context.writable_repository_url = None;
        context.effective_clone_url = context.upstream_repository_url.clone();
        if context.verified_at.is_none() {
            context.verified_at = Some(Utc::now());
        }
    }

    context
}

pub(crate) fn parse_repo_host_and_path(repo_url: &str) -> Option<(String, String)> {
    let trimmed = repo_url.trim();
    if trimmed.is_empty() {
        return None;
    }

    // HTTPS/HTTP form: https://host/group/repo(.git)
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

    // SSH form: git@host:group/repo(.git)
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

pub(crate) fn parse_host_from_urlish(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let without_auth = without_scheme.rsplit('@').next().unwrap_or(without_scheme);
    let host = without_auth.split('/').next()?.trim();

    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
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

fn git_command_non_interactive() -> Command {
    let mut cmd = Command::new("git");
    // Prevent hangs waiting for interactive credentials when PAT/settings are missing.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GCM_INTERACTIVE", "Never");
    cmd.env("GIT_ASKPASS", "echo");
    cmd.env("SSH_ASKPASS", "echo");
    cmd.env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes");
    cmd
}

#[cfg(test)]
mod tests {
    use super::{
        build_authenticated_repo_url, detect_repository_provider_for_repo, parse_github_owner_repo,
        parse_host_from_urlish, parse_repo_host_and_path,
    };
    use acpms_db::models::RepositoryProvider;

    #[test]
    fn parse_repo_host_and_path_from_https_url() {
        let parsed = parse_repo_host_and_path("https://gitlab.example.com/group/sub/repo.git");
        assert_eq!(
            parsed,
            Some((
                "gitlab.example.com".to_string(),
                "group/sub/repo".to_string()
            ))
        );
    }

    #[test]
    fn parse_repo_host_and_path_from_ssh_url() {
        let parsed = parse_repo_host_and_path("git@gitlab.example.com:group/repo.git");
        assert_eq!(
            parsed,
            Some(("gitlab.example.com".to_string(), "group/repo".to_string()))
        );
    }

    #[test]
    fn parse_host_from_urlish_accepts_plain_host() {
        assert_eq!(
            parse_host_from_urlish("gitlab.example.com"),
            Some("gitlab.example.com".to_string())
        );
    }

    #[test]
    fn detect_repository_provider_uses_configured_host() {
        assert_eq!(
            detect_repository_provider_for_repo(
                "https://scm.internal.local/team/app.git",
                Some("https://scm.internal.local"),
            ),
            RepositoryProvider::Gitlab
        );
        assert_eq!(
            detect_repository_provider_for_repo(
                "https://code.internal.local/team/app.git",
                Some("https://code.internal.local/github"),
            ),
            RepositoryProvider::Github
        );
    }

    #[test]
    fn parse_github_owner_repo_from_repo_path() {
        assert_eq!(
            parse_github_owner_repo("openai/codex.git"),
            Some(("openai".to_string(), "codex".to_string()))
        );
    }

    #[test]
    fn build_authenticated_repo_url_supports_ssh_gitlab() {
        let url =
            build_authenticated_repo_url("git@gitlab.example.com:group/repo.git", "glpat-123");
        assert_eq!(
            url,
            "https://oauth2:glpat-123@gitlab.example.com/group/repo.git"
        );
    }

    #[test]
    fn build_authenticated_repo_url_uses_github_username() {
        let url = build_authenticated_repo_url("https://github.com/openai/codex.git", "ghp_123");
        assert_eq!(
            url,
            "https://x-access-token:ghp_123@github.com/openai/codex.git"
        );
    }
}
