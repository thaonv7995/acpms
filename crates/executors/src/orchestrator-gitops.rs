use acpms_db::models::{RepositoryAccessMode, RepositoryContext};
use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// GitOps operations for orchestrator
pub struct GitOpsHandler;

impl GitOpsHandler {
    /// Create MR after agent has pushed changes
    /// Agent is responsible for: implement → verify → commit → push
    /// GitOps only creates the MR via GitLab API
    pub async fn create_mr(
        db_pool: &PgPool,
        attempt_id: Uuid,
        system_pat: Option<&str>,
        system_gitlab_url: Option<&str>,
        log_fn: impl Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>,
    ) -> Result<()> {
        // Fetch project info for MR creation
        let project_info_row = sqlx::query(
            r#"
            SELECT p.id,
                   p.repository_url,
                   p.repository_context,
                   g.pat_encrypted,
                   g.gitlab_project_id,
                   g.base_url
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            JOIN projects p ON p.id = t.project_id
            LEFT JOIN gitlab_configurations g ON g.project_id = p.id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(db_pool)
        .await
        .context("Failed to fetch project info for MR")?;

        // Create MR/PR if project has repository_url
        if let Some(row) = project_info_row {
            let project_uuid: Uuid = row.get("id");
            let repository_url: Option<String> = row.get("repository_url");
            let pat_encrypted: Option<String> = row.get("pat_encrypted");
            let gitlab_project_id: Option<i64> = row.get("gitlab_project_id");
            let base_url: Option<String> = row.get("base_url");
            let repository_context = Self::repository_context_from_json(
                row.try_get::<serde_json::Value, _>("repository_context")
                    .ok(),
            );
            let repo_targets = Self::derive_repo_targets(
                repository_url.as_deref(),
                &repository_context,
                gitlab_project_id,
            );

            let Some(source_repository_url) = repo_targets.source_repository_url.as_deref() else {
                log_fn("Repository URL is missing. Skipping MR/PR creation.").await?;
                return Ok(());
            };

            if repository_context.access_mode == RepositoryAccessMode::BranchPushOnly
                || !repository_context.can_open_change_request
            {
                log_fn(
                    "Repository allows branch push but not automatic pull/merge request creation. Skipping GitOps creation.",
                )
                .await?;
                return Ok(());
            }

            // Fetch project settings for branch config
            let settings: Option<serde_json::Value> =
                sqlx::query_scalar("SELECT settings FROM projects WHERE id = $1")
                    .bind(project_uuid)
                    .fetch_optional(db_pool)
                    .await
                    .ok()
                    .flatten();

            let target_branch = Self::resolve_target_branch_from_settings_impl(settings.as_ref());
            let source_branch = format!("feat/attempt-{}", attempt_id);

            let metadata: Option<serde_json::Value> =
                sqlx::query_scalar("SELECT metadata FROM task_attempts WHERE id = $1")
                    .bind(attempt_id)
                    .fetch_optional(db_pool)
                    .await
                    .ok()
                    .flatten();

            let default_title = format!("Task Attempt {}", attempt_id);
            let default_desc = format!("Implemented by Agent for attempt {}", attempt_id);
            let (title, description) = metadata
                .as_ref()
                .and_then(|m| m.as_object())
                .map(|obj| {
                    let title = obj
                        .get("mr_title")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .map(|s: &str| s.trim().to_string())
                        .filter(|s: &String| !s.is_empty())
                        .unwrap_or_else(|| default_title.clone());
                    let desc = obj
                        .get("mr_description")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .map(|s: &str| s.trim().to_string())
                        .filter(|s: &String| !s.is_empty())
                        .unwrap_or_else(|| default_desc.clone());
                    (title, desc)
                })
                .unwrap_or_else(|| (default_title, default_desc));

            // Skip MR/PR creation when there are no code changes
            let (additions, deletions) = sqlx::query_as::<_, (Option<i32>, Option<i32>)>(
                "SELECT diff_total_additions, diff_total_deletions FROM task_attempts WHERE id = $1",
            )
            .bind(attempt_id)
            .fetch_optional(db_pool)
            .await
            .context("Failed to fetch attempt diff stats")?
            .unwrap_or((None, None));

            let has_changes = additions.unwrap_or(1) > 0 || deletions.unwrap_or(1) > 0;
            if !has_changes {
                log_fn(
                    "Skipping MR/PR creation: no code changes detected (0 additions, 0 deletions). \
                    Code may have been pushed directly to main branch.",
                )
                .await?;
                return Ok(());
            }

            let Some(target_repository_url) = repo_targets.target_repository_url.as_deref() else {
                log_fn("Target repository URL is missing. Skipping MR/PR creation.").await?;
                return Ok(());
            };
            let persisted_targets = PersistedMergeRequestTargets {
                source_repository_url: repo_targets.source_repository_url.clone(),
                target_repository_url: repo_targets.target_repository_url.clone(),
                source_branch: Some(source_branch.clone()),
                target_branch: Some(target_branch.clone()),
                source_project_id: repo_targets.source_project_id,
                target_project_id: repo_targets.target_project_id,
                source_namespace: repo_targets
                    .source_repository_url
                    .as_deref()
                    .and_then(Self::parse_repo_host_and_path)
                    .and_then(|(_, path)| Self::namespace_from_repo_path(&path)),
                target_namespace: repo_targets
                    .target_repository_url
                    .as_deref()
                    .and_then(Self::parse_repo_host_and_path)
                    .and_then(|(_, path)| Self::namespace_from_repo_path(&path)),
            };

            let Some((repo_host, repo_path)) =
                Self::parse_repo_host_and_path(target_repository_url)
            else {
                log_fn("Could not parse repository URL. Skipping MR/PR creation.").await?;
                return Ok(());
            };

            let is_github = repo_host.contains("github.com");

            if is_github {
                // --- GitHub PR path ---
                let Some((pat, _)) = Self::resolve_gitlab_auth(
                    pat_encrypted.as_deref(),
                    base_url.as_deref(),
                    system_pat,
                    system_gitlab_url,
                ) else {
                    log_fn("GitHub PAT is not available (GITLAB_PAT). Skipping PR creation.")
                        .await?;
                    return Ok(());
                };

                let (owner, repo) = match repo_path.split_once('/') {
                    Some((o, r)) => (o, r),
                    None => {
                        log_fn("Could not parse owner/repo from repository URL.").await?;
                        return Ok(());
                    }
                };

                log_fn("Creating Pull Request (GitHub)...").await?;

                let gh_client = acpms_github::GitHubClient::new("https://github.com", &pat)
                    .map_err(|e| anyhow::anyhow!("GitHub client: {}", e))?;
                let head_spec =
                    if repository_context.access_mode == RepositoryAccessMode::ForkGitops {
                        let Some((_, source_repo_path)) =
                            Self::parse_repo_host_and_path(source_repository_url)
                        else {
                            log_fn("Could not parse writable fork URL for GitHub.").await?;
                            return Ok(());
                        };
                        let Some((source_owner, _)) = source_repo_path.split_once('/') else {
                            log_fn("Could not parse writable fork owner for GitHub.").await?;
                            return Ok(());
                        };
                        format!("{}:{}", source_owner, source_branch)
                    } else {
                        source_branch.clone()
                    };

                let pr = match gh_client
                    .create_pull_request(
                        owner,
                        repo,
                        acpms_github::CreatePrParams {
                            title: title.clone(),
                            head: head_spec.clone(),
                            base: target_branch.clone(),
                            body: Some(description),
                        },
                    )
                    .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        let err_str = e.to_string();
                        // 422: A pull request already exists for ...
                        if err_str.contains("422") || err_str.contains("already exists") {
                            let prs = gh_client
                                .list_pulls_by_head(owner, repo, &head_spec)
                                .await
                                .context("Failed to list existing PRs")?;
                            if let Some(existing) = prs.first() {
                                log_fn(&format!(
                                    "Using existing Pull Request: {} (already exists for this branch)",
                                    existing.html_url
                                ))
                                .await?;
                                existing.clone()
                            } else {
                                return Err(e);
                            }
                        } else {
                            return Err(e);
                        }
                    }
                };

                log_fn(&format!("Pull Request Created: {}", pr.html_url)).await?;

                sqlx::query(
                    r#"
                    INSERT INTO merge_requests (
                        task_id, attempt_id, provider, github_pr_number, web_url, status,
                        source_repository_url, target_repository_url,
                        source_branch, target_branch,
                        source_project_id, target_project_id,
                        source_namespace, target_namespace
                    )
                    VALUES (
                        (SELECT task_id FROM task_attempts WHERE id = $1), $1, 'github', $2, $3, 'opened',
                        $4, $5, $6, $7, $8, $9, $10, $11
                    )
                    "#,
                )
                .bind(attempt_id)
                .bind(pr.number as i64)
                .bind(&pr.html_url)
                .bind(&persisted_targets.source_repository_url)
                .bind(&persisted_targets.target_repository_url)
                .bind(&persisted_targets.source_branch)
                .bind(&persisted_targets.target_branch)
                .bind(persisted_targets.source_project_id)
                .bind(persisted_targets.target_project_id)
                .bind(&persisted_targets.source_namespace)
                .bind(&persisted_targets.target_namespace)
                .execute(db_pool)
                .await?;
            } else {
                // --- GitLab MR path ---
                let mut project_id = repo_targets.target_project_id.map(|id| id as u64);
                let mut effective_base_url = base_url.clone();

                if project_id.is_none() {
                    if let Some((resolved_project_id, resolved_base_url)) =
                        Self::resolve_and_persist_gitlab_project_id(
                            db_pool,
                            project_uuid,
                            repo_targets.target_repository_url.as_deref(),
                            system_pat,
                            system_gitlab_url,
                        )
                        .await?
                    {
                        project_id = Some(resolved_project_id as u64);
                        effective_base_url = Some(resolved_base_url);
                        log_fn(&format!(
                            "Auto-linked GitLab project configuration (gitlab_project_id={})",
                            resolved_project_id
                        ))
                        .await?;
                    }
                }

                let Some(project_id) = project_id else {
                    log_fn(
                        "GitLab project link is missing (gitlab_project_id). Skipping MR creation.",
                    )
                    .await?;
                    return Ok(());
                };

                let Some((pat, resolved_base_url)) = Self::resolve_gitlab_auth(
                    pat_encrypted.as_deref(),
                    effective_base_url.as_deref(),
                    system_pat,
                    system_gitlab_url,
                ) else {
                    log_fn("GitLab PAT is not available. Skipping MR creation.").await?;
                    return Ok(());
                };

                let client = acpms_gitlab::GitLabClient::new(&resolved_base_url, &pat)?;

                log_fn("Creating Merge Request (GitLab)...").await?;
                let source_project_id_for_mr = match (
                    repo_targets.source_project_id,
                    repo_targets.target_project_id,
                ) {
                    (Some(source_id), Some(target_id)) if source_id != target_id => {
                        Some(source_id as u64)
                    }
                    _ => None,
                };

                let (mr, is_existing) = match client
                    .create_merge_request(
                        project_id,
                        acpms_gitlab::types::CreateMrParams {
                            source_branch: source_branch.clone(),
                            target_branch,
                            source_project_id: source_project_id_for_mr,
                            title,
                            description: Some(description),
                            remove_source_branch: true,
                        },
                    )
                    .await
                {
                    Ok(m) => (m, false),
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("409") && err_str.contains("already exists") {
                            let iid = err_str.split('!').nth(1).and_then(|s| {
                                let digits: String =
                                    s.chars().take_while(|c| c.is_ascii_digit()).collect();
                                digits.parse::<u64>().ok()
                            });
                            if let Some(iid) = iid {
                                let existing = client
                                    .get_merge_request(project_id, iid)
                                    .await
                                    .context("Failed to fetch existing MR from GitLab")?;
                                log_fn(&format!(
                                    "Using existing Merge Request: {} (already exists for this branch)",
                                    existing.web_url
                                ))
                                .await?;
                                (existing, true)
                            } else {
                                return Err(e);
                            }
                        } else {
                            return Err(e);
                        }
                    }
                };

                if !is_existing {
                    log_fn(&format!("Merge Request Created: {}", mr.web_url)).await?;
                }

                sqlx::query(
                    r#"
                    INSERT INTO merge_requests (
                        task_id, attempt_id, provider, gitlab_mr_iid, web_url, status,
                        source_repository_url, target_repository_url,
                        source_branch, target_branch,
                        source_project_id, target_project_id,
                        source_namespace, target_namespace
                    )
                    VALUES (
                        (SELECT task_id FROM task_attempts WHERE id = $1), $1, 'gitlab', $2, $3, 'opened',
                        $4, $5, $6, $7, $8, $9, $10, $11
                    )
                    "#,
                )
                .bind(attempt_id)
                .bind(mr.iid as i64)
                .bind(&mr.web_url)
                .bind(&persisted_targets.source_repository_url)
                .bind(&persisted_targets.target_repository_url)
                .bind(&persisted_targets.source_branch)
                .bind(&persisted_targets.target_branch)
                .bind(persisted_targets.source_project_id)
                .bind(persisted_targets.target_project_id)
                .bind(&persisted_targets.source_namespace)
                .bind(&persisted_targets.target_namespace)
                .execute(db_pool)
                .await?;
            }
        }

        Ok(())
    }

    /// Merge MR for the given attempt into target branch.
    ///
    /// Returns:
    /// - Ok(true): MR exists and is merged (or was already merged)
    /// - Ok(false): GitLab not configured for the project
    /// - Err: merge resolution/merge API failure
    pub async fn merge_mr_for_attempt(
        db_pool: &PgPool,
        attempt_id: Uuid,
        system_pat: Option<&str>,
        system_gitlab_url: Option<&str>,
        log_fn: impl Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>,
    ) -> Result<bool> {
        let ctx = sqlx::query(
            r#"
            SELECT t.id AS task_id,
                   p.id AS project_id,
                   p.repository_url,
                   p.repository_context,
                   g.pat_encrypted,
                   g.gitlab_project_id,
                   g.base_url
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            JOIN projects p ON p.id = t.project_id
            LEFT JOIN gitlab_configurations g ON g.project_id = t.project_id
            WHERE ta.id = $1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(db_pool)
        .await
        .context("Failed to fetch GitLab context for merge")?;

        let Some(ctx) = ctx else {
            anyhow::bail!("Attempt {} not found while merging MR", attempt_id);
        };
        let task_id: Uuid = ctx.get("task_id");
        let project_uuid: Uuid = ctx.get("project_id");
        let repository_url: Option<String> = ctx.get("repository_url");
        let pat_encrypted: Option<String> = ctx.get("pat_encrypted");
        let gitlab_project_id: Option<i64> = ctx.get("gitlab_project_id");
        let base_url: Option<String> = ctx.get("base_url");
        let repository_context = Self::repository_context_from_json(
            ctx.try_get::<serde_json::Value, _>("repository_context")
                .ok(),
        );

        let expected_source_branch = format!("feat/attempt-{}", attempt_id);

        // Lookup MR/PR by attempt_id (provider, gitlab_mr_iid or github_pr_number)
        #[derive(sqlx::FromRow)]
        struct MrLookup {
            provider: String,
            gitlab_mr_iid: Option<i64>,
            github_pr_number: Option<i64>,
            source_repository_url: Option<String>,
            target_repository_url: Option<String>,
            source_branch: Option<String>,
            target_branch: Option<String>,
            source_project_id: Option<i64>,
            target_project_id: Option<i64>,
            source_namespace: Option<String>,
            target_namespace: Option<String>,
        }

        let mr_row: Option<MrLookup> = sqlx::query_as(
            r#"
            SELECT COALESCE(provider, 'gitlab') AS provider,
                   gitlab_mr_iid,
                   github_pr_number,
                   source_repository_url,
                   target_repository_url,
                   source_branch,
                   target_branch,
                   source_project_id,
                   target_project_id,
                   source_namespace,
                   target_namespace
            FROM merge_requests
            WHERE attempt_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(attempt_id)
        .fetch_optional(db_pool)
        .await
        .context("Failed to lookup MR/PR by attempt_id")?;

        let Some(mr_row) = mr_row else {
            // Legacy: try task_id + gitlab_mr_iid for old rows
            let candidate: Option<MrLookup> = sqlx::query_as(
                r#"
                SELECT COALESCE(provider, 'gitlab') AS provider,
                       gitlab_mr_iid,
                       github_pr_number,
                       source_repository_url,
                       target_repository_url,
                       source_branch,
                       target_branch,
                       source_project_id,
                       target_project_id,
                       source_namespace,
                       target_namespace
                FROM merge_requests
                WHERE task_id = $1
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            )
            .bind(task_id)
            .fetch_optional(db_pool)
            .await
            .context("Failed to lookup legacy merge requests")?;

            let Some(candidate) = candidate else {
                let (additions, deletions) = sqlx::query_as::<_, (Option<i32>, Option<i32>)>(
                    "SELECT diff_total_additions, diff_total_deletions FROM task_attempts WHERE id = $1",
                )
                .bind(attempt_id)
                .fetch_optional(db_pool)
                .await
                .context("Failed to fetch attempt diff stats")?
                .unwrap_or((None, None));
                let has_changes = additions.unwrap_or(1) > 0 || deletions.unwrap_or(1) > 0;
                if !has_changes {
                    log_fn("No MR/PR to merge (no code changes). Task can be marked complete.")
                        .await?;
                    return Ok(false);
                }
                anyhow::bail!(
                    "No merge request or pull request found for attempt {} (branch {})",
                    attempt_id,
                    expected_source_branch
                );
            };

            let persisted_targets = Some(PersistedMergeRequestTargets {
                source_repository_url: candidate.source_repository_url,
                target_repository_url: candidate.target_repository_url,
                source_branch: candidate.source_branch,
                target_branch: candidate.target_branch,
                source_project_id: candidate.source_project_id,
                target_project_id: candidate.target_project_id,
                source_namespace: candidate.source_namespace,
                target_namespace: candidate.target_namespace,
            });

            return Self::do_merge(
                db_pool,
                attempt_id,
                task_id,
                project_uuid,
                repository_url.as_deref(),
                &repository_context,
                pat_encrypted.as_deref(),
                gitlab_project_id,
                base_url.as_deref(),
                system_pat,
                system_gitlab_url,
                &candidate.provider,
                candidate.gitlab_mr_iid,
                candidate.github_pr_number,
                persisted_targets,
                expected_source_branch,
                log_fn,
            )
            .await;
        };

        let persisted_targets = Some(PersistedMergeRequestTargets {
            source_repository_url: mr_row.source_repository_url,
            target_repository_url: mr_row.target_repository_url,
            source_branch: mr_row.source_branch,
            target_branch: mr_row.target_branch,
            source_project_id: mr_row.source_project_id,
            target_project_id: mr_row.target_project_id,
            source_namespace: mr_row.source_namespace,
            target_namespace: mr_row.target_namespace,
        });

        Self::do_merge(
            db_pool,
            attempt_id,
            task_id,
            project_uuid,
            repository_url.as_deref(),
            &repository_context,
            pat_encrypted.as_deref(),
            gitlab_project_id,
            base_url.as_deref(),
            system_pat,
            system_gitlab_url,
            &mr_row.provider,
            mr_row.gitlab_mr_iid,
            mr_row.github_pr_number,
            persisted_targets,
            expected_source_branch,
            log_fn,
        )
        .await
    }

    async fn do_merge(
        db_pool: &PgPool,
        attempt_id: Uuid,
        task_id: Uuid,
        project_uuid: Uuid,
        repository_url: Option<&str>,
        repository_context: &RepositoryContext,
        pat_encrypted: Option<&str>,
        gitlab_project_id: Option<i64>,
        base_url: Option<&str>,
        system_pat: Option<&str>,
        system_gitlab_url: Option<&str>,
        provider: &str,
        gitlab_mr_iid: Option<i64>,
        github_pr_number: Option<i64>,
        persisted_targets: Option<PersistedMergeRequestTargets>,
        expected_source_branch: String,
        log_fn: impl Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>,
    ) -> Result<bool> {
        let derived_repo_targets =
            Self::derive_repo_targets(repository_url, repository_context, gitlab_project_id);
        let repo_targets = RepoTargets {
            source_repository_url: persisted_targets
                .as_ref()
                .and_then(|targets| targets.source_repository_url.clone())
                .or(derived_repo_targets.source_repository_url),
            target_repository_url: persisted_targets
                .as_ref()
                .and_then(|targets| targets.target_repository_url.clone())
                .or(derived_repo_targets.target_repository_url),
            source_project_id: persisted_targets
                .as_ref()
                .and_then(|targets| targets.source_project_id)
                .or(derived_repo_targets.source_project_id),
            target_project_id: persisted_targets
                .as_ref()
                .and_then(|targets| targets.target_project_id)
                .or(derived_repo_targets.target_project_id),
        };
        let expected_source_branch = persisted_targets
            .as_ref()
            .and_then(|targets| targets.source_branch.clone())
            .unwrap_or(expected_source_branch);
        let expected_target_branch = persisted_targets
            .as_ref()
            .and_then(|targets| targets.target_branch.clone());

        if provider == "github" {
            let Some(target_repository_url) = repo_targets.target_repository_url.as_deref() else {
                log_fn("Target repository URL missing for GitHub merge.").await?;
                return Ok(false);
            };
            let Some((_, repo_path)) = Self::parse_repo_host_and_path(target_repository_url) else {
                log_fn("Could not parse repository URL for GitHub.").await?;
                return Ok(false);
            };
            let Some((owner, repo)) = repo_path.split_once('/') else {
                log_fn("Could not parse owner/repo for GitHub.").await?;
                return Ok(false);
            };
            let Some(pr_number) = github_pr_number else {
                log_fn("GitHub PR number missing.").await?;
                return Ok(false);
            };
            let Some((pat, _)) =
                Self::resolve_gitlab_auth(pat_encrypted, base_url, system_pat, system_gitlab_url)
            else {
                log_fn("GitHub PAT not available. Skipping merge.").await?;
                return Ok(false);
            };

            let gh_client = acpms_github::GitHubClient::new("https://github.com", &pat)?;
            let pr = gh_client
                .get_pull_request(owner, repo, pr_number as u64)
                .await
                .with_context(|| format!("Failed to fetch PR {}", pr_number))?;

            if pr.head.r#ref != expected_source_branch {
                anyhow::bail!(
                    "PR head branch mismatch: expected {}, got {}",
                    expected_source_branch,
                    pr.head.r#ref
                );
            }
            if let Some(expected_target_branch) = expected_target_branch.as_deref() {
                if pr.base.r#ref != expected_target_branch {
                    anyhow::bail!(
                        "PR base branch mismatch: expected {}, got {}",
                        expected_target_branch,
                        pr.base.r#ref
                    );
                }
            }
            if let Some(source_repository_url) = repo_targets.source_repository_url.as_deref() {
                let Some((_, source_repo_path)) =
                    Self::parse_repo_host_and_path(source_repository_url)
                else {
                    anyhow::bail!("Could not parse source repository URL for GitHub merge");
                };
                let pr_head_repo = pr
                    .head
                    .repo
                    .as_ref()
                    .map(|repo| repo.full_name.clone())
                    .unwrap_or_default();
                if !pr_head_repo.eq_ignore_ascii_case(&source_repo_path) {
                    anyhow::bail!(
                        "PR source repository mismatch: expected {}, got {}",
                        source_repo_path,
                        pr_head_repo
                    );
                }
            }

            let pr_url = pr.html_url.clone();
            if pr.state == "closed" && pr.merged.unwrap_or(false) {
                log_fn(&format!("Pull Request already merged: {}", pr_url)).await?;
            } else {
                log_fn("Merging Pull Request...").await?;
                gh_client
                    .merge_pull_request(owner, repo, pr_number as u64)
                    .await
                    .context("Failed to merge PR")?;
            }

            sqlx::query(
                r#"
                UPDATE merge_requests
                SET status = 'merged', updated_at = NOW()
                WHERE task_id = $1 AND github_pr_number = $2 AND (attempt_id = $3 OR attempt_id IS NULL)
                "#,
            )
            .bind(task_id)
            .bind(pr_number)
            .bind(attempt_id)
            .execute(db_pool)
            .await
            .context("Failed to update merge request status")?;

            log_fn(&format!("Pull Request merged: {}", pr_url)).await?;
            return Ok(true);
        }

        // GitLab path: resolve project_id if missing
        let (project_id, effective_base_url): (Option<i64>, Option<String>) =
            match repo_targets.target_project_id {
                Some(id) => (Some(id), base_url.map(|s| s.to_string())),
                None => {
                    if let Some((resolved_id, resolved_url)) =
                        Self::resolve_and_persist_gitlab_project_id(
                            db_pool,
                            project_uuid,
                            repo_targets.target_repository_url.as_deref(),
                            system_pat,
                            system_gitlab_url,
                        )
                        .await?
                    {
                        (Some(resolved_id), Some(resolved_url))
                    } else {
                        (None, base_url.map(|s| s.to_string()))
                    }
                }
            };

        let Some(project_id) = project_id else {
            log_fn("GitLab project link missing. Skipping merge.").await?;
            return Ok(false);
        };

        let Some(mr_iid) = gitlab_mr_iid else {
            log_fn("GitLab MR IID missing.").await?;
            return Ok(false);
        };

        let Some((pat, resolved_base_url)) = Self::resolve_gitlab_auth(
            pat_encrypted,
            effective_base_url.as_deref(),
            system_pat,
            system_gitlab_url,
        ) else {
            log_fn("GitLab PAT not available. Skipping merge.").await?;
            return Ok(false);
        };

        let client = acpms_gitlab::GitLabClient::new(&resolved_base_url, &pat)?;
        let current_mr = client
            .get_merge_request(project_id as u64, mr_iid as u64)
            .await
            .with_context(|| format!("Failed to fetch MR {}", mr_iid))?;

        if current_mr.source_branch != expected_source_branch {
            anyhow::bail!(
                "MR source branch mismatch: expected {}, got {}",
                expected_source_branch,
                current_mr.source_branch
            );
        }
        if let Some(expected_target_branch) = expected_target_branch.as_deref() {
            if current_mr.target_branch != expected_target_branch {
                anyhow::bail!(
                    "MR target branch mismatch: expected {}, got {}",
                    expected_target_branch,
                    current_mr.target_branch
                );
            }
        }
        if let Some(expected_source_project_id) = repo_targets.source_project_id {
            if let Some(actual_source_project_id) = current_mr.source_project_id {
                if actual_source_project_id as i64 != expected_source_project_id {
                    anyhow::bail!(
                        "MR source project mismatch: expected {}, got {}",
                        expected_source_project_id,
                        actual_source_project_id
                    );
                }
            }
        }

        let final_mr = if current_mr.state == "merged" {
            current_mr
        } else {
            log_fn("Merging Merge Request...").await?;
            client
                .merge_merge_request(project_id as u64, mr_iid as u64, true)
                .await
                .with_context(|| format!("Failed to merge MR {}", mr_iid))?
        };

        sqlx::query(
            r#"
            UPDATE merge_requests
            SET status = $3, updated_at = NOW()
            WHERE task_id = $1 AND gitlab_mr_iid = $2 AND (attempt_id = $4 OR attempt_id IS NULL)
            "#,
        )
        .bind(task_id)
        .bind(mr_iid)
        .bind(&final_mr.state)
        .bind(attempt_id)
        .execute(db_pool)
        .await
        .context("Failed to update merge request status")?;

        log_fn(&format!("Merge Request merged: {}", final_mr.web_url)).await?;
        Ok(true)
    }

    async fn resolve_and_persist_gitlab_project_id(
        db_pool: &PgPool,
        project_uuid: Uuid,
        repository_url: Option<&str>,
        system_pat: Option<&str>,
        system_gitlab_url: Option<&str>,
    ) -> Result<Option<(i64, String)>> {
        let Some(repository_url) = repository_url else {
            return Ok(None);
        };

        let Some((pat, base_url)) =
            Self::resolve_gitlab_auth(None, None, system_pat, system_gitlab_url)
        else {
            return Ok(None);
        };

        let Some((repo_host, repo_path)) = Self::parse_repo_host_and_path(repository_url) else {
            return Ok(None);
        };
        let Some(configured_host) = Self::parse_host_from_urlish(&base_url) else {
            return Ok(None);
        };

        if !repo_host.eq_ignore_ascii_case(&configured_host) {
            return Ok(None);
        }

        let client = match acpms_gitlab::GitLabClient::new(&base_url, &pat) {
            Ok(client) => client,
            Err(_) => return Ok(None),
        };

        let gitlab_project = match client.get_project_by_path(&repo_path).await {
            Ok(project) => project,
            Err(_) => return Ok(None),
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
        .bind(project_uuid)
        .bind(gitlab_project.id as i64)
        .bind(&base_url)
        .bind("GLOBAL")
        .bind(Uuid::new_v4().to_string())
        .execute(db_pool)
        .await
        .context("Failed to upsert auto-linked GitLab configuration from repository URL")?;

        Ok(Some((gitlab_project.id as i64, base_url)))
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

    fn namespace_from_repo_path(repo_path: &str) -> Option<String> {
        let trimmed = repo_path.trim().trim_matches('/');
        let (namespace, _) = trimmed.rsplit_once('/')?;
        let namespace = namespace.trim().trim_matches('/');
        if namespace.is_empty() {
            None
        } else {
            Some(namespace.to_string())
        }
    }

    /// Resolve MR target branch from project settings.
    /// Priority: mr_target_branch > deploy_branch > "main"
    pub(crate) fn resolve_target_branch_from_settings_impl(
        settings: Option<&serde_json::Value>,
    ) -> String {
        settings
            .and_then(|s| s.get("mr_target_branch").and_then(|v| v.as_str()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                settings
                    .and_then(|s| s.get("deploy_branch").and_then(|v| v.as_str()))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "main".to_string())
    }

    fn parse_host_from_urlish(input: &str) -> Option<String> {
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

    fn resolve_gitlab_auth(
        configured_pat: Option<&str>,
        configured_base_url: Option<&str>,
        system_pat: Option<&str>,
        system_gitlab_url: Option<&str>,
    ) -> Option<(String, String)> {
        let configured_pat = configured_pat
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let system_pat = system_pat.map(str::trim).filter(|value| !value.is_empty());

        let pat = match configured_pat {
            Some("GLOBAL") => system_pat.map(|value| value.to_string())?,
            Some(value) => value.to_string(),
            None => system_pat.map(|value| value.to_string())?,
        };

        let base_url = configured_base_url
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                system_gitlab_url
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or("https://gitlab.com")
            .to_string();

        Some((pat, base_url))
    }

    fn repository_context_from_json(value: Option<serde_json::Value>) -> RepositoryContext {
        value
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default()
    }

    fn derive_repo_targets(
        repository_url: Option<&str>,
        repository_context: &RepositoryContext,
        gitlab_project_id: Option<i64>,
    ) -> RepoTargets {
        let source_repository_url = repository_context
            .writable_repository_url
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .or_else(|| repository_url.map(|value| value.to_string()));

        let target_repository_url =
            if repository_context.access_mode == RepositoryAccessMode::ForkGitops {
                repository_context
                    .upstream_repository_url
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .or_else(|| repository_url.map(|value| value.to_string()))
            } else {
                repository_url.map(|value| value.to_string())
            };

        let source_project_id =
            if repository_context.access_mode == RepositoryAccessMode::ForkGitops {
                repository_context.writable_project_id.or(gitlab_project_id)
            } else {
                gitlab_project_id
                    .or(repository_context.writable_project_id)
                    .or(repository_context.upstream_project_id)
            };

        let target_project_id =
            if repository_context.access_mode == RepositoryAccessMode::ForkGitops {
                repository_context.upstream_project_id
            } else {
                gitlab_project_id
                    .or(repository_context.upstream_project_id)
                    .or(repository_context.writable_project_id)
            };

        RepoTargets {
            source_repository_url,
            target_repository_url,
            source_project_id,
            target_project_id,
        }
    }
}

#[derive(Debug, Clone)]
struct RepoTargets {
    source_repository_url: Option<String>,
    target_repository_url: Option<String>,
    source_project_id: Option<i64>,
    target_project_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct PersistedMergeRequestTargets {
    source_repository_url: Option<String>,
    target_repository_url: Option<String>,
    source_branch: Option<String>,
    target_branch: Option<String>,
    source_project_id: Option<i64>,
    target_project_id: Option<i64>,
    source_namespace: Option<String>,
    target_namespace: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::GitOpsHandler;

    #[test]
    fn resolve_target_branch_uses_mr_target_branch_when_set() {
        let settings = serde_json::json!({
            "mr_target_branch": "develop",
            "deploy_branch": "main"
        });
        let result = GitOpsHandler::resolve_target_branch_from_settings_impl(Some(&settings));
        assert_eq!(result, "develop");
    }

    #[test]
    fn resolve_target_branch_falls_back_to_deploy_branch() {
        let settings = serde_json::json!({
            "deploy_branch": "production"
        });
        let result = GitOpsHandler::resolve_target_branch_from_settings_impl(Some(&settings));
        assert_eq!(result, "production");
    }

    #[test]
    fn resolve_target_branch_defaults_to_main_when_empty() {
        let result = GitOpsHandler::resolve_target_branch_from_settings_impl(None);
        assert_eq!(result, "main");
    }

    #[test]
    fn resolve_target_branch_ignores_empty_mr_target_branch() {
        let settings = serde_json::json!({
            "mr_target_branch": "",
            "deploy_branch": "staging"
        });
        let result = GitOpsHandler::resolve_target_branch_from_settings_impl(Some(&settings));
        assert_eq!(result, "staging");
    }

    #[test]
    fn parse_repo_host_and_path_supports_https() {
        let parsed =
            GitOpsHandler::parse_repo_host_and_path("https://gitlab.example.com/group/repo.git");
        assert_eq!(
            parsed,
            Some(("gitlab.example.com".to_string(), "group/repo".to_string()))
        );
    }

    #[test]
    fn resolve_gitlab_auth_uses_system_pat_for_global_marker() {
        let auth = GitOpsHandler::resolve_gitlab_auth(
            Some("GLOBAL"),
            Some("https://gitlab.example.com"),
            Some("glpat-abc"),
            Some("https://gitlab.example.com"),
        );
        assert_eq!(
            auth,
            Some((
                "glpat-abc".to_string(),
                "https://gitlab.example.com".to_string()
            ))
        );
    }
}
