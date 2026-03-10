use super::*;

#[derive(Debug, Default, Clone)]
pub(super) struct PersistedStructuredOutputs {
    pub preview_target: Option<String>,
    pub preview_url: Option<String>,
    pub cloudflare_tunnel_error: Option<String>,
    pub deployment_report: Option<serde_json::Value>,
    pub mr_title: Option<String>,
    pub mr_description: Option<String>,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct PreviewRuntimeControlContract {
    #[serde(default)]
    controllable: Option<bool>,
    #[serde(default)]
    runtime_type: Option<String>,
    #[serde(default)]
    container_name: Option<String>,
    #[serde(default)]
    compose_project_name: Option<String>,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct PreviewCloudflareCleanupContract {
    #[serde(default)]
    tunnel_id: Option<String>,
    #[serde(default)]
    dns_record_id: Option<String>,
    #[serde(default)]
    zone_id: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub(super) struct ExtractedPreviewContract {
    pub(super) preview_target: Option<String>,
    pub(super) preview_url: Option<String>,
    pub(super) cloudflare_tunnel_error: Option<String>,
    pub(super) runtime_control: Option<serde_json::Value>,
    pub(super) cloudflare_cleanup: Option<serde_json::Value>,
}

fn parse_preview_output_contract(contents: &str) -> Option<ExtractedPreviewContract> {
    #[derive(serde::Deserialize)]
    struct PreviewOutputContract {
        preview_target: Option<String>,
        preview_url: Option<String>,
        #[serde(default)]
        cloudflare_tunnel_error: Option<String>,
        #[serde(default)]
        runtime_control: Option<PreviewRuntimeControlContract>,
        #[serde(default)]
        cloudflare_cleanup: Option<PreviewCloudflareCleanupContract>,
    }

    let parsed: PreviewOutputContract = serde_json::from_str(contents).ok()?;

    let target = parsed
        .preview_target
        .map(|s| trim_repo_url_candidate(s.trim()))
        .filter(|s| !s.is_empty() && !s.contains("<port>"));
    let url = parsed
        .preview_url
        .map(|s| trim_repo_url_candidate(s.trim()))
        .filter(|s| !s.is_empty());
    let cloudflare_tunnel_error = parsed
        .cloudflare_tunnel_error
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let (target, url) = canonicalize_preview_signals(target, url);

    let runtime_control = parsed
        .runtime_control
        .and_then(normalize_preview_runtime_control_value);
    let cloudflare_cleanup = parsed
        .cloudflare_cleanup
        .and_then(normalize_preview_cloudflare_cleanup_value);

    if target.is_none()
        && url.is_none()
        && cloudflare_tunnel_error.is_none()
        && runtime_control.is_none()
        && cloudflare_cleanup.is_none()
    {
        return None;
    }

    Some(ExtractedPreviewContract {
        preview_target: target,
        preview_url: url,
        cloudflare_tunnel_error,
        runtime_control,
        cloudflare_cleanup,
    })
}

fn is_local_preview_signal_url(candidate: &str) -> bool {
    let candidate = candidate.trim().to_ascii_lowercase();
    candidate.starts_with("http://localhost:")
        || candidate.starts_with("https://localhost:")
        || candidate.starts_with("http://127.0.0.1:")
        || candidate.starts_with("https://127.0.0.1:")
        || candidate.starts_with("http://0.0.0.0:")
        || candidate.starts_with("https://0.0.0.0:")
        || candidate.starts_with("http://[::1]:")
        || candidate.starts_with("https://[::1]:")
}

fn canonicalize_preview_signals(
    preview_target: Option<String>,
    preview_url: Option<String>,
) -> (Option<String>, Option<String>) {
    let mut preview_target = preview_target
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let mut preview_url = preview_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(preview_target_value) = preview_target.as_ref() {
        if !is_local_preview_signal_url(preview_target_value) {
            let url_missing_or_local = preview_url
                .as_deref()
                .map(is_local_preview_signal_url)
                .unwrap_or(true);

            if url_missing_or_local {
                preview_url = Some(preview_target_value.clone());
            }
            // PREVIEW_TARGET must always point at the local runtime. If the
            // agent wrote a public URL here, surface it as PREVIEW_URL only.
            preview_target = None;
        }
    }

    (preview_target, preview_url)
}

fn normalize_preview_runtime_control_value(
    raw: PreviewRuntimeControlContract,
) -> Option<serde_json::Value> {
    let runtime_type = raw
        .runtime_type
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let container_name = raw
        .container_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let compose_project_name = raw
        .compose_project_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let has_docker_container =
        matches!(runtime_type.as_deref(), Some("docker_container")) && container_name.is_some();
    let has_compose_project = matches!(runtime_type.as_deref(), Some("docker_compose_project"))
        && compose_project_name.is_some();

    let controllable = raw
        .controllable
        .unwrap_or(has_docker_container || has_compose_project);

    if runtime_type.is_none() && container_name.is_none() && compose_project_name.is_none() {
        return None;
    }

    let mut object = serde_json::Map::new();
    object.insert(
        "controllable".to_string(),
        serde_json::Value::Bool(controllable),
    );
    if let Some(runtime_type) = runtime_type {
        object.insert(
            "runtime_type".to_string(),
            serde_json::Value::String(runtime_type),
        );
    }
    if let Some(container_name) = container_name {
        object.insert(
            "container_name".to_string(),
            serde_json::Value::String(container_name),
        );
    }
    if let Some(compose_project_name) = compose_project_name {
        object.insert(
            "compose_project_name".to_string(),
            serde_json::Value::String(compose_project_name),
        );
    }

    Some(serde_json::Value::Object(object))
}

fn normalize_preview_cloudflare_cleanup_value(
    raw: PreviewCloudflareCleanupContract,
) -> Option<serde_json::Value> {
    let tunnel_id = raw
        .tunnel_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let dns_record_id = raw
        .dns_record_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let zone_id = raw
        .zone_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if tunnel_id.is_none() && dns_record_id.is_none() && zone_id.is_none() {
        return None;
    }

    let mut object = serde_json::Map::new();
    object.insert(
        "provider".to_string(),
        serde_json::Value::String("cloudflare".to_string()),
    );
    if let Some(tunnel_id) = tunnel_id {
        object.insert(
            "tunnel_id".to_string(),
            serde_json::Value::String(tunnel_id),
        );
    }
    if let Some(dns_record_id) = dns_record_id {
        object.insert(
            "dns_record_id".to_string(),
            serde_json::Value::String(dns_record_id),
        );
    }
    if let Some(zone_id) = zone_id {
        object.insert("zone_id".to_string(), serde_json::Value::String(zone_id));
    }

    Some(serde_json::Value::Object(object))
}

impl ExecutorOrchestrator {
    pub(crate) async fn fetch_attempt_log_lines(
        &self,
        attempt_id: Uuid,
        context: &str,
    ) -> Result<Vec<String>> {
        let bytes = crate::read_attempt_log_file(attempt_id)
            .await
            .with_context(|| format!("Failed to fetch attempt logs for {}", context))?;
        let logs = crate::parse_jsonl_to_agent_logs(&bytes);
        Ok(logs.into_iter().map(|l| l.content).collect())
    }

    pub(super) async fn persist_structured_outputs_from_attempt_logs(
        &self,
        attempt_id: Uuid,
        worktree_path: Option<&std::path::Path>,
    ) -> Result<PersistedStructuredOutputs> {
        let lines = self
            .fetch_attempt_log_lines(attempt_id, "structured output extraction")
            .await?;

        let (
            preview_target,
            preview_url,
            preview_runtime_control,
            preview_cloudflare_cleanup,
            cloudflare_tunnel_error,
            preview_target_source,
            preview_url_source,
            preview_cloudflare_cleanup_source,
            cloudflare_tunnel_error_source,
        ) = if let Some(wp) = worktree_path {
            if let Ok(Some(contract)) = self.extract_preview_from_file_contract(wp).await {
                let file_url_present = contract.preview_url.is_some();
                let file_error_present = contract.cloudflare_tunnel_error.is_some();
                let preview_url = contract.preview_url.or_else(|| extract_preview_url(&lines));
                let cloudflare_tunnel_error = contract.cloudflare_tunnel_error.or_else(|| {
                    extract_labeled_value(&lines, "CLOUDFLARE_TUNNEL_ERROR")
                        .or_else(|| extract_labeled_value(&lines, "cloudflare_tunnel_error"))
                });
                let preview_url_source = if preview_url.is_some() {
                    if file_url_present {
                        "file_contract"
                    } else {
                        "agent_output"
                    }
                } else {
                    "agent_output"
                };
                let cloudflare_tunnel_error_source = if cloudflare_tunnel_error.is_some() {
                    if file_error_present {
                        "file_contract"
                    } else {
                        "agent_output"
                    }
                } else {
                    "agent_output"
                };
                (
                    contract.preview_target,
                    preview_url,
                    contract.runtime_control,
                    contract.cloudflare_cleanup,
                    cloudflare_tunnel_error,
                    "file_contract",
                    preview_url_source,
                    "file_contract",
                    cloudflare_tunnel_error_source,
                )
            } else {
                (
                    extract_preview_target(&lines),
                    extract_preview_url(&lines),
                    None,
                    None,
                    extract_labeled_value(&lines, "CLOUDFLARE_TUNNEL_ERROR")
                        .or_else(|| extract_labeled_value(&lines, "cloudflare_tunnel_error")),
                    "agent_output",
                    "agent_output",
                    "agent_output",
                    "agent_output",
                )
            }
        } else {
            (
                extract_preview_target(&lines),
                extract_preview_url(&lines),
                None,
                None,
                extract_labeled_value(&lines, "CLOUDFLARE_TUNNEL_ERROR")
                    .or_else(|| extract_labeled_value(&lines, "cloudflare_tunnel_error")),
                "agent_output",
                "agent_output",
                "agent_output",
                "agent_output",
            )
        };
        let (preview_target, preview_url) =
            canonicalize_preview_signals(preview_target, preview_url);
        let deployment_report = extract_deployment_report(&lines);

        // Prefer file contract (.acpms/mr-output.json) over log extraction for MR fields
        let (mr_title, mr_description, mr_source) = if let Some(wp) = worktree_path {
            if let Ok(Some((file_title, file_desc))) = self.extract_mr_from_file_contract(wp).await
            {
                let title = (!file_title.is_empty()).then_some(file_title);
                let desc = (!file_desc.is_empty()).then_some(file_desc);
                if title.is_some() || desc.is_some() {
                    (
                        title.or_else(|| extract_mr_title(&lines)),
                        desc.or_else(|| extract_mr_description(&lines)),
                        "file_contract",
                    )
                } else {
                    (
                        extract_mr_title(&lines),
                        extract_mr_description(&lines),
                        "agent_output",
                    )
                }
            } else {
                (
                    extract_mr_title(&lines),
                    extract_mr_description(&lines),
                    "agent_output",
                )
            }
        } else {
            (
                extract_mr_title(&lines),
                extract_mr_description(&lines),
                "agent_output",
            )
        };

        let mut patch = serde_json::Map::new();
        if let Some(target) = &preview_target {
            patch.insert(
                "preview_target".to_string(),
                serde_json::Value::String(target.to_string()),
            );
            patch.insert(
                "preview_target_source".to_string(),
                serde_json::Value::String(preview_target_source.to_string()),
            );
        }
        if let Some(url) = &preview_url {
            patch.insert(
                "preview_url".to_string(),
                serde_json::Value::String(url.to_string()),
            );
            patch.insert(
                "preview_url_agent".to_string(),
                serde_json::Value::String(url.to_string()),
            );
            patch.insert(
                "preview_url_source".to_string(),
                serde_json::Value::String(preview_url_source.to_string()),
            );
        }
        if let Some(error) = &cloudflare_tunnel_error {
            patch.insert(
                "cloudflare_tunnel_error".to_string(),
                serde_json::Value::String(error.to_string()),
            );
            patch.insert(
                "cloudflare_tunnel_error_source".to_string(),
                serde_json::Value::String(cloudflare_tunnel_error_source.to_string()),
            );
        }
        if let Some(runtime_control) = &preview_runtime_control {
            patch.insert(
                "preview_runtime_control".to_string(),
                runtime_control.clone(),
            );
            patch.insert(
                "preview_runtime_control_source".to_string(),
                serde_json::Value::String("file_contract".to_string()),
            );
            patch.insert(
                "preview_runtime_state".to_string(),
                serde_json::Value::String("active".to_string()),
            );
        } else if preview_target.is_some() || preview_url.is_some() {
            patch.insert(
                "preview_runtime_control".to_string(),
                serde_json::Value::Null,
            );
            patch.insert(
                "preview_runtime_control_source".to_string(),
                serde_json::Value::Null,
            );
        }
        if let Some(cloudflare_cleanup) = &preview_cloudflare_cleanup {
            patch.insert(
                "preview_cloudflare_cleanup".to_string(),
                cloudflare_cleanup.clone(),
            );
            patch.insert(
                "preview_cloudflare_cleanup_source".to_string(),
                serde_json::Value::String(preview_cloudflare_cleanup_source.to_string()),
            );
        } else if preview_target.is_some() || preview_url.is_some() {
            patch.insert(
                "preview_cloudflare_cleanup".to_string(),
                serde_json::Value::Null,
            );
            patch.insert(
                "preview_cloudflare_cleanup_source".to_string(),
                serde_json::Value::Null,
            );
            patch.insert(
                "preview_runtime_state".to_string(),
                serde_json::Value::String("active".to_string()),
            );
        }
        if preview_target.is_some() || preview_url.is_some() {
            patch.insert(
                "preview_runtime_state".to_string(),
                serde_json::Value::String("active".to_string()),
            );
        }
        if let Some(report) = &deployment_report {
            patch.insert("deployment_report".to_string(), report.clone());
            patch.insert(
                "deployment_report_source".to_string(),
                serde_json::Value::String("agent_output".to_string()),
            );

            if let Some(obj) = report.as_object() {
                for key in [
                    "deployment_status",
                    "deployment_error",
                    "deployment_kind",
                    "production_deployment_status",
                    "production_deployment_error",
                    "production_deployment_url",
                    "production_deployment_type",
                    "production_deployment_id",
                    "deploy_precheck",
                    "deploy_precheck_reason",
                ] {
                    if let Some(value) = obj.get(key) {
                        patch.insert(key.to_string(), value.clone());
                    }
                }
            }
        }
        if let Some(title) = &mr_title {
            patch.insert(
                "mr_title".to_string(),
                serde_json::Value::String(title.clone()),
            );
            patch.insert(
                "mr_title_source".to_string(),
                serde_json::Value::String(mr_source.to_string()),
            );
        }
        if let Some(desc) = &mr_description {
            patch.insert(
                "mr_description".to_string(),
                serde_json::Value::String(desc.clone()),
            );
            patch.insert(
                "mr_description_source".to_string(),
                serde_json::Value::String(mr_source.to_string()),
            );
        }

        if !patch.is_empty() {
            sqlx::query(
                r#"
                UPDATE task_attempts
                SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb
                WHERE id = $1
                "#,
            )
            .bind(attempt_id)
            .bind(serde_json::Value::Object(patch))
            .execute(&self.db_pool)
            .await
            .context("Failed to persist structured outputs to attempt metadata")?;
        }

        Ok(PersistedStructuredOutputs {
            preview_target,
            preview_url,
            cloudflare_tunnel_error,
            deployment_report,
            mr_title,
            mr_description,
        })
    }

    pub(super) async fn persist_skill_instruction_context(
        &self,
        attempt_id: Uuid,
        context: &crate::SkillInstructionContext,
        source: &str,
    ) -> Result<()> {
        let patch = crate::build_skill_metadata_patch(context, source);
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(patch)
        .execute(&self.db_pool)
        .await
        .with_context(|| {
            format!(
                "Failed to persist skill instruction metadata for source {}",
                source
            )
        })?;

        Ok(())
    }

    /// Helper: Fetch task from database
    pub(super) async fn extend_agent_env_with_cloudflare_settings(
        &self,
        env_vars: &mut HashMap<String, String>,
    ) {
        let settings = match self.fetch_system_settings().await {
            Ok(settings) => settings,
            Err(error) => {
                warn!(
                    "Failed to load system settings for Cloudflare env injection: {}",
                    error
                );
                return;
            }
        };

        if let Some(account_id) = settings
            .cloudflare_account_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            env_vars.insert("CLOUDFLARE_ACCOUNT_ID".to_string(), account_id.to_string());
        }

        if let Some(zone_id) = settings
            .cloudflare_zone_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            env_vars.insert("CLOUDFLARE_ZONE_ID".to_string(), zone_id.to_string());
        }

        if let Some(base_domain) = settings
            .cloudflare_base_domain
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            env_vars.insert(
                "CLOUDFLARE_BASE_DOMAIN".to_string(),
                base_domain.to_string(),
            );
        }

        if let Some(encrypted_token) = settings
            .cloudflare_api_token_encrypted
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            match self.decrypt_value(encrypted_token) {
                Ok(token) => {
                    let token = token
                        .trim()
                        .trim_start_matches("Bearer ")
                        .trim()
                        .to_string();
                    if !token.is_empty() {
                        env_vars.insert("CLOUDFLARE_API_TOKEN".to_string(), token);
                    }
                }
                Err(error) => {
                    warn!(
                        "Failed to decrypt Cloudflare API token for agent env: {}",
                        error
                    );
                }
            }
        }

        if env_vars.contains_key("CLOUDFLARE_ACCOUNT_ID")
            && env_vars.contains_key("CLOUDFLARE_API_TOKEN")
            && env_vars.contains_key("CLOUDFLARE_ZONE_ID")
            && env_vars.contains_key("CLOUDFLARE_BASE_DOMAIN")
        {
            env_vars.insert("CLOUDFLARE_CONFIGURED".to_string(), "true".to_string());
        }
    }

    /// Helper: Fetch task from database
    pub(super) async fn fetch_task(&self, task_id: Uuid) -> Result<Task> {
        sqlx::query_as::<_, Task>(
            r#"SELECT id, project_id, requirement_id, sprint_id, title, description,
                      task_type, status,
                      assigned_to, parent_task_id, gitlab_issue_id, metadata,
                      created_by, created_at, updated_at
               FROM tasks WHERE id = $1"#,
        )
        .bind(task_id)
        .fetch_one(&self.db_pool)
        .await
        .context("Failed to fetch task")
    }

    /// Helper: Fetch project from database
    pub(super) async fn fetch_project(&self, project_id: Uuid) -> Result<Project> {
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&self.db_pool)
            .await
            .context("Failed to fetch project")
    }

    /// Helper: Fetch system settings from database
    pub(super) async fn fetch_system_settings(&self) -> Result<SystemSettings> {
        sqlx::query_as::<_, SystemSettings>("SELECT * FROM system_settings LIMIT 1")
            .fetch_one(&self.db_pool)
            .await
            .context("Failed to fetch system settings")
    }

    /// Helper: Decrypt AES-256-GCM encrypted value (base64 encoded).
    pub(super) fn decrypt_value(&self, ciphertext_base64: &str) -> Result<String> {
        let data = BASE64
            .decode(ciphertext_base64)
            .context("Failed to decode base64 ciphertext")?;
        if data.len() < 12 {
            bail!("Invalid ciphertext: too short");
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext_bytes = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("Decryption failed"))?;
        String::from_utf8(plaintext_bytes).context("Decrypted data is not valid UTF-8")
    }

    /// Create execution_process record for follow-up/resume. Required when orchestrator
    /// creates attempt internally (e.g. init via create_project) so frontend can send messages.
    pub(super) async fn create_execution_process(
        &self,
        attempt_id: Uuid,
        worktree_path: Option<&std::path::Path>,
        branch_name: Option<&str>,
    ) -> Result<Uuid> {
        let worktree_path = worktree_path.map(|p| p.to_string_lossy().to_string());
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO execution_processes (attempt_id, process_id, worktree_path, branch_name)
            VALUES ($1, NULL, $2, $3)
            RETURNING id
            "#,
        )
        .bind(attempt_id)
        .bind(worktree_path)
        .bind(branch_name)
        .fetch_one(&self.db_pool)
        .await
        .context("Failed to create execution process record")?;
        Ok(id)
    }

    /// Helper: Create task attempt record
    pub(super) async fn create_attempt(&self, task_id: Uuid) -> Result<Uuid> {
        let attempt_id: Uuid = sqlx::query_scalar(
            "INSERT INTO task_attempts (task_id, status, metadata)
             VALUES ($1, 'queued', '{}')
             RETURNING id",
        )
        .bind(task_id)
        .fetch_one(&self.db_pool)
        .await
        .context("Failed to create task attempt")?;

        Ok(attempt_id)
    }

    /// Helper: Update project metadata with repo_relative_path (for collision resolution).
    pub(super) async fn update_project_repo_relative_path(
        &self,
        project_id: Uuid,
        repo_relative_path: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE projects
            SET metadata = metadata || jsonb_build_object('repo_relative_path', $2::text),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .bind(repo_relative_path)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    /// Helper: Update project repository URL
    pub(super) async fn update_project_repo_url(
        &self,
        project_id: Uuid,
        repo_url: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE projects SET repository_url = $2, updated_at = NOW() WHERE id = $1")
            .bind(project_id)
            .bind(repo_url)
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    /// Helper: Update project repository access context.
    pub(super) async fn update_project_repository_context(
        &self,
        project_id: Uuid,
        repository_context: &RepositoryContext,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE projects SET repository_context = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(project_id)
        .bind(
            serde_json::to_value(repository_context)
                .context("Failed to serialize repository context")?,
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Read MR_TITLE and MR_DESCRIPTION from `.acpms/mr-output.json` (file-based contract).
    /// Same pattern as init-output.json for repo_url. File values override log extraction.
    /// Deletes the file after reading to avoid leaving artifacts.
    pub(super) async fn extract_mr_from_file_contract(
        &self,
        worktree_path: &std::path::Path,
    ) -> Result<Option<(String, String)>> {
        let path = worktree_path.join(".acpms/mr-output.json");
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        #[derive(serde::Deserialize)]
        struct MrOutputContract {
            mr_title: Option<String>,
            mr_description: Option<String>,
        }

        let parsed: MrOutputContract = match serde_json::from_str(&contents) {
            Ok(p) => p,
            Err(_) => {
                let _ = tokio::fs::remove_file(&path).await;
                return Ok(None);
            }
        };

        let _ = tokio::fs::remove_file(&path).await;

        let title = parsed
            .mr_title
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let desc = parsed
            .mr_description
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if title.is_some() || desc.is_some() {
            Ok(Some((title.unwrap_or_default(), desc.unwrap_or_default())))
        } else {
            Ok(None)
        }
    }

    /// Read REPO_URL from `.acpms/init-output.json` (file-based contract).
    /// Deletes the file after reading (success or parse failure) to avoid leaving artifacts.
    pub(super) async fn extract_repo_url_from_init_output_file(
        &self,
        worktree_path: &std::path::Path,
    ) -> Result<Option<String>> {
        let path = worktree_path.join(".acpms/init-output.json");
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        #[derive(serde::Deserialize)]
        struct InitOutputContract {
            repo_url: String,
        }

        let parsed: InitOutputContract = match serde_json::from_str(&contents) {
            Ok(p) => p,
            Err(_) => {
                let _ = tokio::fs::remove_file(&path).await;
                return Ok(None);
            }
        };

        let url = parsed.repo_url.trim().to_string();
        let _ = tokio::fs::remove_file(&path).await;
        if url.is_empty() {
            return Ok(None);
        }
        Ok(Some(url))
    }

    /// Extract `REPO_URL: ...` from the persisted raw logs for an attempt.
    pub(super) async fn extract_repo_url_from_attempt_logs(
        &self,
        attempt_id: Uuid,
    ) -> Result<Option<String>> {
        let lines = self
            .fetch_attempt_log_lines(attempt_id, "repo URL extraction")
            .await?;

        Ok(extract_repo_url(&lines))
    }

    /// Read PREVIEW_TARGET / PREVIEW_URL plus optional runtime control metadata
    /// from `.acpms/preview-output.json` (file-based contract).
    /// Same pattern as mr-output.json. File values override log extraction.
    /// The preview contract stays on disk so agent follow-ups can reuse it.
    pub(super) async fn extract_preview_from_file_contract(
        &self,
        worktree_path: &std::path::Path,
    ) -> Result<Option<ExtractedPreviewContract>> {
        let path = worktree_path.join(".acpms/preview-output.json");
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        Ok(parse_preview_output_contract(&contents))
    }

    /// Extract PREVIEW_TARGET from file contract first, then from logs. Persist to attempt metadata.
    pub(super) async fn persist_preview_target_from_attempt_logs(
        &self,
        attempt_id: Uuid,
        worktree_path: Option<&std::path::Path>,
    ) -> Result<Option<String>> {
        let (
            preview_target,
            preview_url,
            preview_runtime_control,
            preview_cloudflare_cleanup,
            cloudflare_tunnel_error,
            source,
            preview_cloudflare_cleanup_source,
            cloudflare_tunnel_error_source,
        ) = if let Some(wp) = worktree_path {
            if let Ok(Some(contract)) = self.extract_preview_from_file_contract(wp).await {
                (
                    contract.preview_target,
                    contract.preview_url,
                    contract.runtime_control,
                    contract.cloudflare_cleanup,
                    contract.cloudflare_tunnel_error,
                    "file_contract",
                    "file_contract",
                    "file_contract",
                )
            } else {
                let lines = self
                    .fetch_attempt_log_lines(attempt_id, "preview target extraction")
                    .await?;
                (
                    extract_preview_target(&lines),
                    extract_preview_url(&lines),
                    None,
                    None,
                    extract_labeled_value(&lines, "CLOUDFLARE_TUNNEL_ERROR")
                        .or_else(|| extract_labeled_value(&lines, "cloudflare_tunnel_error")),
                    "agent_output",
                    "agent_output",
                    "agent_output",
                )
            }
        } else {
            let lines = self
                .fetch_attempt_log_lines(attempt_id, "preview target extraction")
                .await?;
            (
                extract_preview_target(&lines),
                extract_preview_url(&lines),
                None,
                None,
                extract_labeled_value(&lines, "CLOUDFLARE_TUNNEL_ERROR")
                    .or_else(|| extract_labeled_value(&lines, "cloudflare_tunnel_error")),
                "agent_output",
                "agent_output",
                "agent_output",
            )
        };

        let (preview_target, preview_url) =
            canonicalize_preview_signals(preview_target, preview_url);

        let mut metadata_patch = serde_json::Map::new();
        if let Some(preview_target) = preview_target.as_ref() {
            metadata_patch.insert(
                "preview_target".to_string(),
                serde_json::Value::String(preview_target.clone()),
            );
            metadata_patch.insert(
                "preview_target_source".to_string(),
                serde_json::Value::String(source.to_string()),
            );
        }
        if let Some(url) = preview_url {
            metadata_patch.insert(
                "preview_url".to_string(),
                serde_json::Value::String(url.clone()),
            );
            metadata_patch.insert(
                "preview_url_agent".to_string(),
                serde_json::Value::String(url),
            );
            metadata_patch.insert(
                "preview_url_source".to_string(),
                serde_json::Value::String(source.to_string()),
            );
        }
        if let Some(error) = cloudflare_tunnel_error {
            metadata_patch.insert(
                "cloudflare_tunnel_error".to_string(),
                serde_json::Value::String(error),
            );
            metadata_patch.insert(
                "cloudflare_tunnel_error_source".to_string(),
                serde_json::Value::String(cloudflare_tunnel_error_source.to_string()),
            );
        }
        if let Some(runtime_control) = preview_runtime_control {
            metadata_patch.insert("preview_runtime_control".to_string(), runtime_control);
            metadata_patch.insert(
                "preview_runtime_control_source".to_string(),
                serde_json::Value::String(source.to_string()),
            );
            metadata_patch.insert(
                "preview_runtime_state".to_string(),
                serde_json::Value::String("active".to_string()),
            );
        } else if preview_target.is_some() || metadata_patch.contains_key("preview_url") {
            metadata_patch.insert(
                "preview_runtime_control".to_string(),
                serde_json::Value::Null,
            );
            metadata_patch.insert(
                "preview_runtime_control_source".to_string(),
                serde_json::Value::Null,
            );
        }
        if let Some(cloudflare_cleanup) = preview_cloudflare_cleanup {
            metadata_patch.insert("preview_cloudflare_cleanup".to_string(), cloudflare_cleanup);
            metadata_patch.insert(
                "preview_cloudflare_cleanup_source".to_string(),
                serde_json::Value::String(preview_cloudflare_cleanup_source.to_string()),
            );
        } else {
            metadata_patch.insert(
                "preview_cloudflare_cleanup".to_string(),
                serde_json::Value::Null,
            );
            metadata_patch.insert(
                "preview_cloudflare_cleanup_source".to_string(),
                serde_json::Value::Null,
            );
            metadata_patch.insert(
                "preview_runtime_state".to_string(),
                serde_json::Value::String("active".to_string()),
            );
        }
        if preview_target.is_some() || metadata_patch.contains_key("preview_url") {
            metadata_patch.insert(
                "preview_runtime_state".to_string(),
                serde_json::Value::String("active".to_string()),
            );
        }

        if !metadata_patch.is_empty() {
            sqlx::query(
                r#"
                UPDATE task_attempts
                SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb
                WHERE id = $1
                "#,
            )
            .bind(attempt_id)
            .bind(serde_json::Value::Object(metadata_patch))
            .execute(&self.db_pool)
            .await
            .context("Failed to persist preview target to attempt metadata")?;
        }

        Ok(preview_target)
    }

    /// Extract origin URL directly from git remote configuration.
    ///
    /// Used as a fallback when agent logs do not contain an explicit REPO_URL line.
    pub(super) async fn extract_repo_url_from_git_remote(
        &self,
        worktree_path: &Path,
    ) -> Result<Option<String>> {
        let output = tokio::process::Command::new("git")
            .arg("remote")
            .arg("get-url")
            .arg("origin")
            .current_dir(worktree_path)
            .output()
            .await
            .context("Failed to read git remote origin URL")?;

        if !output.status.success() {
            return Ok(None);
        }

        let raw_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if raw_url.is_empty() {
            return Ok(None);
        }

        Ok(Some(raw_url))
    }

    /// Helper: Update project type
    pub(super) async fn update_project_type(
        &self,
        project_id: Uuid,
        project_type: ProjectType,
    ) -> Result<()> {
        sqlx::query("UPDATE projects SET project_type = $2, updated_at = NOW() WHERE id = $1")
            .bind(project_id)
            .bind(project_type)
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    /// Helper: List files in a directory recursively (for type detection)
    pub(super) fn list_repo_files(&self, repo_path: &std::path::Path) -> Vec<String> {
        let mut files = Vec::new();
        let walker = walkdir::WalkDir::new(repo_path)
            .max_depth(4) // Limit depth for performance
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories and common ignored paths
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.')
                    && name != "node_modules"
                    && name != "target"
                    && name != "vendor"
            });

        for entry in walker.flatten() {
            if entry.file_type().is_file() {
                if let Ok(rel_path) = entry.path().strip_prefix(repo_path) {
                    files.push(rel_path.to_string_lossy().to_string());
                }
            }
        }
        files
    }

    /// Helper: Mark task as completed
    pub(super) async fn mark_task_completed(&self, task_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE tasks SET status = 'done', updated_at = NOW() WHERE id = $1")
            .bind(task_id)
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    /// Helper: Mark task as failed
    pub(super) async fn mark_task_failed(&self, task_id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            r#"UPDATE tasks
               SET status = 'blocked',
                   updated_at = NOW(),
                   metadata = jsonb_set(metadata, '{error}', to_jsonb($2::text))
               WHERE id = $1"#,
        )
        .bind(task_id)
        .bind(error)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Stream agent output to logs with capture capability.
    ///
    /// ## Behavior
    /// - Streams stdout/stderr to database logs
    /// - Returns captured stdout lines
    /// - Sanitizes logs before storage
    pub(super) async fn stream_agent_output_with_capture(
        &self,
        child: &mut AsyncGroupChild,
        attempt_id: Uuid,
    ) -> Result<Vec<String>> {
        let stdout = child
            .inner()
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let stderr = child
            .inner()
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;

        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let db_pool = self.db_pool.clone();
        let tx = self.broadcast_tx.clone();
        let captured_lines = Arc::new(Mutex::new(Vec::new()));

        // Spawn stdout handler
        let stdout_task = {
            let pool = db_pool.clone();
            let tx = tx.clone();
            let captured = captured_lines.clone();

            tokio::spawn(async move {
                let mut lines = stdout_reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if should_skip_log_line(&line) {
                        continue;
                    }
                    let sanitized = sanitize_log(&line);
                    captured.lock().await.push(sanitized.clone());
                    let _ = StatusManager::log(&pool, &tx, attempt_id, "stdout", &sanitized).await;
                }
            })
        };

        // Spawn stderr handler
        let stderr_task = {
            let pool = db_pool.clone();
            let tx = tx.clone();

            tokio::spawn(async move {
                let mut lines = stderr_reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if should_skip_log_line(&line) {
                        continue;
                    }
                    let sanitized = sanitize_log(&line);
                    let _ = StatusManager::log(&pool, &tx, attempt_id, "stderr", &sanitized).await;
                }
            })
        };

        // Wait for both tasks
        let _ = tokio::try_join!(stdout_task, stderr_task)?;

        // Extract captured lines from Arc<Mutex>
        let lines = Arc::try_unwrap(captured_lines)
            .map_err(|_| anyhow::anyhow!("Failed to unwrap captured lines"))?
            .into_inner();

        Ok(lines)
    }

    /// Stream agent output to logs (simpler version without capture).
    #[allow(dead_code)]
    pub(super) async fn stream_agent_output(
        &self,
        child: &mut AsyncGroupChild,
        attempt_id: Uuid,
    ) -> Result<()> {
        let _ = self
            .stream_agent_output_with_capture(child, attempt_id)
            .await?;
        Ok(())
    }

    /// Public accessor for worktree manager (used by API routes for cleanup).
    pub fn worktree_manager(&self) -> &WorktreeManager {
        &self.worktree_manager
    }

    /// Public wrapper for handle_gitops (used by approve endpoint).
    pub async fn handle_gitops_public(&self, attempt_id: Uuid) -> Result<()> {
        self.handle_gitops(attempt_id).await
    }

    /// Public wrapper for handle_gitops_merge (used by approve endpoint).
    /// Uses same logic as agent flow: can resolve gitlab_project_id when missing.
    pub async fn handle_gitops_merge_public(&self, attempt_id: Uuid) -> Result<bool> {
        self.handle_gitops_merge(attempt_id).await
    }

    /// Public wrapper for pushing an attempt branch with the same repository auth
    /// resolution used by orchestrator execution paths.
    pub async fn push_attempt_worktree_public(
        &self,
        attempt_id: Uuid,
        worktree_path: &std::path::Path,
    ) -> Result<()> {
        let (repo_url, pat) = self
            .resolve_repository_origin_and_pat(attempt_id, None)
            .await?;
        self.worktree_manager
            .push_worktree(worktree_path, &repo_url, &pat)
            .await
    }

    /// Public method to cleanup worktree after approve/reject
    /// Called from routes after review is complete
    pub async fn cleanup_worktree_public(&self, attempt_id: Uuid) -> Result<()> {
        // Get project's repo_path from attempt metadata
        let repo_path = self.get_repo_path_from_attempt(attempt_id).await?;

        self.log(attempt_id, "system", "Cleaning up worktree...")
            .await?;

        if let Err(e) = self.cleanup_attempt_worktree(&repo_path, attempt_id).await {
            self.log(
                attempt_id,
                "system",
                &format!("Warning: Cleanup failed: {}", e),
            )
            .await?;
            return Err(e);
        }

        self.log(attempt_id, "system", "Cleanup completed.").await?;
        Ok(())
    }

    pub(super) async fn cleanup_attempt_worktree(
        &self,
        repo_path: &std::path::Path,
        attempt_id: Uuid,
    ) -> Result<()> {
        let had_preview_signal = self
            .best_effort_stop_local_preview_for_attempt(attempt_id)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(
                    "Failed to stop local preview before worktree cleanup for attempt {}: {}",
                    attempt_id,
                    error
                );
                false
            });

        self.worktree_manager
            .cleanup_worktree(repo_path, attempt_id)
            .await?;

        if had_preview_signal {
            if let Err(error) = self
                .mark_attempt_preview_runtime_stopped(attempt_id, "worktree_cleaned")
                .await
            {
                tracing::warn!(
                    "Failed to mark preview stopped after worktree cleanup for attempt {}: {}",
                    attempt_id,
                    error
                );
            }
        }

        Ok(())
    }

    async fn best_effort_stop_local_preview_for_attempt(&self, attempt_id: Uuid) -> Result<bool> {
        let metadata: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT COALESCE(metadata, '{}'::jsonb) FROM task_attempts WHERE id = $1",
        )
        .bind(attempt_id)
        .fetch_optional(&self.db_pool)
        .await?;

        let Some(metadata) = metadata else {
            return Ok(false);
        };

        let preview_url = metadata
            .get("preview_url")
            .or_else(|| metadata.get("preview_url_agent"))
            .or_else(|| metadata.get("preview_target"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);

        let Some(preview_url) = preview_url else {
            return Ok(false);
        };

        if let Some(port) = extract_local_preview_port(&preview_url) {
            let _ = self.stop_processes_listening_on_port(port).await;
        }

        Ok(true)
    }

    async fn stop_processes_listening_on_port(&self, port: u16) -> Result<()> {
        let output = Command::new("lsof")
            .args(["-ti", &format!("tcp:{}", port)])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("Failed to inspect listeners on port {}", port))?;

        if !output.status.success() && output.stdout.is_empty() {
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for pid in stdout
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let _ = Command::new("kill")
                .args(["-TERM", pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .await;
        }

        Ok(())
    }

    async fn mark_attempt_preview_runtime_stopped(
        &self,
        attempt_id: Uuid,
        reason: &str,
    ) -> Result<()> {
        let patch = serde_json::json!({
            "preview_runtime_state": "stopped",
            "preview_runtime_stopped_at": chrono::Utc::now().to_rfc3339(),
            "preview_runtime_stop_reason": reason,
        });

        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(patch)
        .execute(&self.db_pool)
        .await
        .context("Failed to persist preview stop metadata")?;

        Ok(())
    }

    /// Get repo path from attempt's task's project
    pub(super) async fn get_repo_path_from_attempt(&self, attempt_id: Uuid) -> Result<PathBuf> {
        let row = sqlx::query(
            r#"
            SELECT p.id, p.name, p.metadata
            FROM task_attempts ta
            JOIN tasks t ON ta.task_id = t.id
            JOIN projects p ON t.project_id = p.id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_one(&self.db_pool)
        .await?;

        let project_id: Uuid = row.get("id");
        let name: String = row.get("name");
        let metadata: serde_json::Value = row.get("metadata");
        let repo_path = project_repo_relative_path(project_id, &metadata, &name);

        Ok(self.worktree_manager.base_path().await.join(repo_path))
    }
}

fn extract_local_preview_port(url: &str) -> Option<u16> {
    let remainder = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let host_and_path = remainder.split('/').next()?;
    let (host, port) = host_and_path.rsplit_once(':')?;
    if host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" {
        return port.parse::<u16>().ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_preview_signals, parse_preview_output_contract};

    #[test]
    fn canonicalize_preview_signals_promotes_public_target_into_preview_url() {
        let (preview_target, preview_url) = canonicalize_preview_signals(
            Some("https://task-abcd.preview.example.com".to_string()),
            None,
        );

        assert_eq!(preview_target, None);
        assert_eq!(
            preview_url.as_deref(),
            Some("https://task-abcd.preview.example.com")
        );
    }

    #[test]
    fn canonicalize_preview_signals_prefers_public_target_over_local_preview_url() {
        let (_preview_target, preview_url) = canonicalize_preview_signals(
            Some("https://task-abcd.preview.example.com".to_string()),
            Some("http://127.0.0.1:4174".to_string()),
        );

        assert_eq!(
            preview_url.as_deref(),
            Some("https://task-abcd.preview.example.com")
        );
    }

    #[test]
    fn canonicalize_preview_signals_keeps_local_target_separate_from_preview_url() {
        let (preview_target, preview_url) =
            canonicalize_preview_signals(Some("http://127.0.0.1:4174".to_string()), None);

        assert_eq!(preview_target.as_deref(), Some("http://127.0.0.1:4174"));
        assert_eq!(preview_url, None);
    }

    #[test]
    fn parse_preview_output_contract_preserves_cloudflare_tunnel_error() {
        let contract = parse_preview_output_contract(
            r#"{
  "preview_target": "http://127.0.0.1:8080",
  "cloudflare_tunnel_error": "cloudflare_not_configured — missing: CLOUDFLARE_ACCOUNT_ID",
  "runtime_control": {
    "controllable": true,
    "runtime_type": "docker_compose_project",
    "compose_project_name": "landing-page-abc"
  }
}"#,
        )
        .expect("preview contract should parse");

        assert_eq!(
            contract.preview_target.as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            contract.cloudflare_tunnel_error.as_deref(),
            Some("cloudflare_not_configured — missing: CLOUDFLARE_ACCOUNT_ID")
        );
        assert!(contract.runtime_control.is_some());
    }

    #[test]
    fn parse_preview_output_contract_preserves_cloudflare_cleanup_metadata() {
        let contract = parse_preview_output_contract(
            r#"{
  "preview_target": "http://127.0.0.1:8080",
  "preview_url": "https://task-abcd.preview.example.com",
  "cloudflare_cleanup": {
    "tunnel_id": "935949eb-eebc-458f-86cc-de0502e91208",
    "dns_record_id": "dns-record-123",
    "zone_id": "zone-123"
  }
}"#,
        )
        .expect("preview contract should parse");

        let cleanup = contract
            .cloudflare_cleanup
            .and_then(|value| value.as_object().cloned())
            .expect("cloudflare cleanup metadata should be present");
        assert_eq!(
            cleanup.get("provider").and_then(|value| value.as_str()),
            Some("cloudflare")
        );
        assert_eq!(
            cleanup.get("tunnel_id").and_then(|value| value.as_str()),
            Some("935949eb-eebc-458f-86cc-de0502e91208")
        );
        assert_eq!(
            cleanup
                .get("dns_record_id")
                .and_then(|value| value.as_str()),
            Some("dns-record-123")
        );
        assert_eq!(
            cleanup.get("zone_id").and_then(|value| value.as_str()),
            Some("zone-123")
        );
    }
}
