use acpms_db::models::{ProjectSettings, ProjectType, Task, TaskType};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX_SKILL_CHARS: usize = 6_000;
const MAX_TOTAL_SKILL_CHARS: usize = 24_000;

#[derive(Debug, Clone)]
struct LoadedSkill {
    id: String,
    content: String,
    source: Option<PathBuf>,
}

pub fn build_skill_instruction_block(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
    repo_path: Option<&Path>,
) -> String {
    let skill_ids = resolve_skill_chain(task, settings, project_type);
    if skill_ids.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        r#"

## Active Skills (Required)
Follow these skill playbooks strictly for this attempt. If a skill cannot be executed, state why in the final report.
"#,
    );

    let mut total_chars = 0usize;
    for skill_id in skill_ids {
        if total_chars >= MAX_TOTAL_SKILL_CHARS {
            break;
        }

        let loaded = load_skill(&skill_id, repo_path).unwrap_or_else(|| LoadedSkill {
            id: skill_id.clone(),
            content: builtin_skill_content(&skill_id)
                .unwrap_or(
                    "No external skill file found. Use standard best-practice execution for this capability.",
                )
                .to_string(),
            source: None,
        });

        let mut content = loaded.content.trim().to_string();
        if content.len() > MAX_SKILL_CHARS {
            content.truncate(MAX_SKILL_CHARS);
            content.push_str("\n... (skill content truncated)");
        }

        total_chars = total_chars.saturating_add(content.len());
        out.push_str("\n### Skill: ");
        out.push_str(&loaded.id);
        if let Some(path) = loaded.source {
            out.push_str("\nSource: `");
            out.push_str(&path.to_string_lossy());
            out.push('`');
        } else {
            out.push_str("\nSource: `builtin`");
        }
        out.push_str("\n```text\n");
        out.push_str(&content);
        out.push_str("\n```\n");
    }

    out
}

pub fn resolve_skill_chain(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
) -> Vec<String> {
    derive_skill_chain(task, settings, project_type)
}

/// Load content for a single skill (from file or builtin). Used for custom flows like import analysis.
pub fn get_skill_content(skill_id: &str, repo_path: Option<&Path>) -> String {
    load_skill(skill_id, repo_path)
        .map(|s| s.content)
        .or_else(|| builtin_skill_content(skill_id).map(str::to_string))
        .unwrap_or_else(|| format!("Execute: {}", skill_id))
}

fn derive_skill_chain(
    task: &Task,
    settings: &ProjectSettings,
    project_type: ProjectType,
) -> Vec<String> {
    let mut ids: Vec<String> = vec![
        "task-preflight-check".to_string(),
        "env-and-secrets-validate".to_string(),
    ];
    let mut seen: HashSet<String> = ids.iter().cloned().collect();

    let require_review = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("require_review"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            task.metadata
                .get("require_review")
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(settings.require_review);

    let run_build_and_tests = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("run_build_and_tests"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if run_build_and_tests {
        push_skill(&mut ids, &mut seen, "verify-test-build");
    }

    let auto_deploy_enabled = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("auto_deploy"))
        .and_then(|v| v.as_bool())
        .or_else(|| task.metadata.get("auto_deploy").and_then(|v| v.as_bool()))
        .unwrap_or(settings.auto_deploy);
    let preview_delivery_enabled = auto_deploy_enabled || settings.preview_enabled;

    if matches!(task.task_type, TaskType::Deploy) {
        push_skill(&mut ids, &mut seen, "build-artifact");
        push_skill(&mut ids, &mut seen, "deploy-ssh-remote");
        if run_build_and_tests {
            push_skill(&mut ids, &mut seen, "verify-test-build");
        }
        push_skill(&mut ids, &mut seen, "release-note-and-delivery-summary");
        for explicit in extract_explicit_skills(&task.metadata) {
            push_skill(&mut ids, &mut seen, &explicit);
        }
        push_skill(&mut ids, &mut seen, "final-report");
        return ids;
    }

    if matches!(task.task_type, TaskType::Init) {
        push_skill(&mut ids, &mut seen, "init-read-references");
        match project_type {
            ProjectType::Web => push_skill(&mut ids, &mut seen, "init-web-scaffold"),
            ProjectType::Api => push_skill(&mut ids, &mut seen, "init-api-scaffold"),
            ProjectType::Mobile => push_skill(&mut ids, &mut seen, "init-mobile-scaffold"),
            ProjectType::Extension => push_skill(&mut ids, &mut seen, "init-extension-scaffold"),
            ProjectType::Desktop => push_skill(&mut ids, &mut seen, "init-desktop-scaffold"),
            ProjectType::Microservice => {
                push_skill(&mut ids, &mut seen, "init-microservice-scaffold")
            }
        }
        push_skill(&mut ids, &mut seen, "init-project-bootstrap");
        push_skill(&mut ids, &mut seen, "init-project-context-file");
        push_skill(&mut ids, &mut seen, "init-source-repository");
    } else {
        push_skill(&mut ids, &mut seen, "code-implement");
    }

    if settings.auto_retry {
        push_skill(&mut ids, &mut seen, "retry-triage-and-recovery");
    }

    match project_type {
        ProjectType::Web => {
            push_skill(&mut ids, &mut seen, "build-artifact");
            if preview_delivery_enabled {
                push_skill(&mut ids, &mut seen, "cloudflare-config-validate");
                push_skill(&mut ids, &mut seen, "cloudflare-tunnel-setup-guide");
                push_skill(&mut ids, &mut seen, "deploy-precheck-cloudflare");
                push_skill(&mut ids, &mut seen, "setup-cloudflare-tunnel");
                push_skill(&mut ids, &mut seen, "deploy-cloudflare-pages");
                push_skill(&mut ids, &mut seen, "cloudflare-dns-route");
                push_skill(&mut ids, &mut seen, "post-deploy-smoke-and-healthcheck");
                push_skill(&mut ids, &mut seen, "update-deployment-metadata");
            }
        }
        ProjectType::Api => {
            push_skill(&mut ids, &mut seen, "build-artifact");
            if preview_delivery_enabled {
                push_skill(&mut ids, &mut seen, "cloudflare-config-validate");
                push_skill(&mut ids, &mut seen, "cloudflare-tunnel-setup-guide");
                push_skill(&mut ids, &mut seen, "deploy-precheck-cloudflare");
                push_skill(&mut ids, &mut seen, "deploy-cloudflare-workers");
                push_skill(&mut ids, &mut seen, "cloudflare-dns-route");
                push_skill(&mut ids, &mut seen, "post-deploy-smoke-and-healthcheck");
                push_skill(&mut ids, &mut seen, "update-deployment-metadata");
            }
        }
        ProjectType::Desktop => {
            push_skill(&mut ids, &mut seen, "build-artifact");
            if preview_delivery_enabled {
                push_skill(&mut ids, &mut seen, "preview-artifact-desktop");
            }
        }
        ProjectType::Mobile => {
            push_skill(&mut ids, &mut seen, "build-artifact");
            if preview_delivery_enabled {
                push_skill(&mut ids, &mut seen, "preview-artifact-mobile");
            }
        }
        ProjectType::Extension => {
            push_skill(&mut ids, &mut seen, "build-artifact");
            if preview_delivery_enabled {
                push_skill(&mut ids, &mut seen, "preview-artifact-extension");
            }
        }
        ProjectType::Microservice => {
            push_skill(&mut ids, &mut seen, "build-artifact");
            if preview_delivery_enabled {
                push_skill(&mut ids, &mut seen, "post-deploy-smoke-and-healthcheck");
                push_skill(&mut ids, &mut seen, "update-deployment-metadata");
            }
        }
    }

    if run_build_and_tests {
        push_skill(&mut ids, &mut seen, "verify-test-build");
    }

    if task_mentions_database_changes(task) {
        push_skill(&mut ids, &mut seen, "db-migration-safety");
    }

    if task_mentions_deploy_cancel_or_cleanup(task) {
        push_skill(&mut ids, &mut seen, "deploy-cancel-stop-cleanup");
    }

    if require_review {
        push_skill(&mut ids, &mut seen, "review-handoff");
        push_skill(&mut ids, &mut seen, "gitlab-rebase-conflict-resolution");
    } else if !matches!(task.task_type, TaskType::Init) {
        push_skill(&mut ids, &mut seen, "gitlab-branch-and-commit");
        push_skill(&mut ids, &mut seen, "gitlab-merge-request");
        push_skill(&mut ids, &mut seen, "gitlab-issue-sync");
    }

    push_skill(&mut ids, &mut seen, "release-note-and-delivery-summary");

    for explicit in extract_explicit_skills(&task.metadata) {
        push_skill(&mut ids, &mut seen, &explicit);
    }

    push_skill(&mut ids, &mut seen, "final-report");
    ids
}

fn extract_explicit_skills(metadata: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut push_from = |value: Option<&serde_json::Value>| {
        let Some(value) = value else {
            return;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                if let Some(skill) = item.as_str() {
                    let skill = skill.trim();
                    if !skill.is_empty() {
                        out.push(skill.to_string());
                    }
                }
            }
        }
    };

    push_from(metadata.get("skills"));
    push_from(metadata.get("execution").and_then(|v| v.get("skills")));
    push_from(metadata.get("execution").and_then(|v| v.get("skill_chain")));
    out
}

fn task_mentions_deploy_cancel_or_cleanup(task: &Task) -> bool {
    let mut haystack = String::new();
    haystack.push_str(&task.title.to_lowercase());
    haystack.push(' ');
    if let Some(description) = &task.description {
        haystack.push_str(&description.to_lowercase());
    }

    [
        "cancel deploy",
        "dừng deploy",
        "stop deploy",
        "stop container",
        "dừng container",
        "docker down",
        "docker stop",
        "xoá resource",
        "remove resource",
        "cleanup deploy",
        "tear down",
        "rollback",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn task_mentions_database_changes(task: &Task) -> bool {
    let metadata_hint = task
        .metadata
        .get("execution")
        .and_then(|v| v.get("database_migration"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if metadata_hint {
        return true;
    }

    let mut haystack = String::new();
    haystack.push_str(&task.title.to_lowercase());
    haystack.push(' ');
    if let Some(description) = &task.description {
        haystack.push_str(&description.to_lowercase());
    }

    [
        "migration",
        "migrate",
        "schema",
        "database",
        "postgres",
        "sql",
        "table",
        "column",
        "index",
        "db ",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn push_skill(ids: &mut Vec<String>, seen: &mut HashSet<String>, skill: &str) {
    if seen.insert(skill.to_string()) {
        ids.push(skill.to_string());
    }
}

fn load_skill(skill_id: &str, repo_path: Option<&Path>) -> Option<LoadedSkill> {
    if !is_safe_skill_id(skill_id) {
        return None;
    }

    for dir in candidate_skill_roots(repo_path) {
        let path = dir.join(skill_id).join("SKILL.md");
        if !path.exists() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        return Some(LoadedSkill {
            id: skill_id.to_string(),
            content,
            source: Some(path),
        });
    }

    None
}

fn candidate_skill_roots(repo_path: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut push = |path: PathBuf| {
        if path.as_os_str().is_empty() {
            return;
        }
        if seen.insert(path.clone()) {
            roots.push(path);
        }
    };

    // 1. Per-project skills (worktree .acpms/skills)
    if let Some(repo) = repo_path {
        push(repo.join(".acpms").join("skills"));
    }
    // 2. Platform skills dir (installer sets ACPMS_SKILLS_DIR, e.g. /opt/acpms/.acpms/skills)
    if let Ok(dir) = std::env::var("ACPMS_SKILLS_DIR") {
        push(PathBuf::from(dir));
    }
    // 3. CWD (dev/local)
    if let Ok(cwd) = std::env::current_dir() {
        push(cwd.join(".acpms").join("skills"));
    }
    // 4. Codex home (user's Codex skills)
    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        push(PathBuf::from(codex_home).join("skills"));
    } else if let Some(home) = dirs::home_dir() {
        push(home.join(".codex").join("skills"));
    }

    roots
}

fn is_safe_skill_id(skill_id: &str) -> bool {
    !skill_id.is_empty()
        && skill_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn builtin_skill_content(skill_id: &str) -> Option<&'static str> {
    match skill_id {
        "task-preflight-check" => Some(
            r#"Run first before any implementation. Validate references and environment.
- If .acpms/references/refs_manifest.json exists and has failures: STOP, output PREFLIGHT BLOCKED report.
- If reference files listed in manifest are missing: STOP.
- If git repo is broken or missing: STOP.
- Report blocking issues clearly so user can fix before retrying."#,
        ),
        "code-implement" => Some(
            r#"Implement only scoped changes for this task.
- Keep edits minimal and coherent.
- Do not modify unrelated files.
- Preserve existing architecture and conventions."#,
        ),
        "verify-test-build" => Some(
            r#"Run relevant verification commands before finishing.
- Prefer existing project scripts (test/lint/build).
- Report what passed, failed, or was skipped with reasons."#,
        ),
        "env-and-secrets-validate" => Some(
            r#"Validate required environment variables before build/deploy.
- Report missing required env names only (never secret values).
- Stop dependent steps when required configuration is missing."#,
        ),
        "init-read-references" => Some(
            r#"If .acpms-refs/ exists and is non-empty, read reference files before scaffolding.
- List and read source code, configs, specs, or mockups.
- Use insights to replicate structure and patterns in init-project-bootstrap.
- If .acpms-refs/ is missing or empty, skip this step."#,
        ),
        "init-project-bootstrap" => Some(
            r#"Bootstrap initial project structure using selected stack.
- Keep setup minimal and reproducible.
- Run baseline validation and summarize generated artifacts."#,
        ),
        "init-project-context-file" => Some(
            r#"Create PROJECT_CONTEXT.md with architecture overview and development guidelines.
- Use project-type-appropriate content (web, API, mobile, etc.).
- Include key commands and workflows for future AI agents."#,
        ),
        "init-web-scaffold" => Some(
            r#"Web app scaffold: package.json, build tools (Vite/Next.js), TypeScript, README, .gitignore, .env.example, ESLint/Prettier, src/, public/, routing. Use Project Details for name/description."#,
        ),
        "init-api-scaffold" => Some(
            r#"API scaffold: init project (Cargo/package/requirements), web framework, README, .gitignore, Docker, routes, middleware, health check, /api/v1/, CRUD template. Use Project Details for name/description."#,
        ),
        "init-mobile-scaffold" => Some(
            r#"Mobile scaffold: React Native/Expo/Flutter, platform config, README, .gitignore, Info.plist, AndroidManifest, src/lib/, navigation, screens. Use Project Details for name/description."#,
        ),
        "init-extension-scaffold" => Some(
            r#"Extension scaffold: manifest.json V3, build tools, README, background/content/popup/options, permissions, multi-browser. Use Project Details for name/description."#,
        ),
        "init-desktop-scaffold" => Some(
            r#"Desktop scaffold: Electron/Tauri, main/renderer, README, .gitignore, IPC, packaging, code signing. Use Project Details for name/description."#,
        ),
        "init-microservice-scaffold" => Some(
            r#"Microservice scaffold: go.mod/Cargo.toml, Dockerfile, docker-compose, health/ready/live, metrics, logging, cmd/, api/, configs/. Use Project Details for name/description."#,
        ),
        "init-import-analyze" => Some(
            r#"Analyze imported repository: explore directory structure, identify services/components, evaluate tech stack.
- List key dirs (src/, app/, packages/, services/).
- Identify frontend, backend, database, auth, cache, queue, storage.
- Write .acpms/import-analysis.json with architecture (nodes, edges) and assessment (project_type, summary, services, tech_stack).
- Node types: client, frontend, api, database, cache, queue, storage, auth, gateway, service, mobile, worker.
- Read-only: do not modify source code."#,
        ),
        "build-artifact" => Some(
            r#"Produce build artifacts appropriate for project type.
- Ensure output path exists.
- Record artifact commands and output summary in report."#,
        ),
        "preview-artifact-desktop" => Some(
            r#"For desktop task previews, produce installable desktop artifacts.
- Keep build command/output dir aligned with project metadata.
- Prefer native installers or platform bundles in the desktop output folder.
- Report install notes for macOS and Windows when applicable."#,
        ),
        "preview-artifact-mobile" => Some(
            r#"For mobile task previews, produce downloadable test artifacts.
- Prefer APK/AAB for Android; note clearly when iOS requires signing or simulator-only output.
- Keep build command/output dir aligned with project metadata.
- Report install steps and platform limitations."#,
        ),
        "preview-artifact-extension" => Some(
            r#"For extension task previews, produce downloadable extension bundles.
- Prefer a ready-to-load .zip when the build already emits one; otherwise ensure the output directory can be zipped.
- Verify manifest/build output is complete.
- Report browser load/install steps in the final summary."#,
        ),
        "cloudflare-config-validate" => Some(
            r#"Validate Cloudflare account and API token before tunnel/deploy.
- If missing, report cloudflare_not_configured.
- Skip unsafe steps and continue completion flow."#,
        ),
        "cloudflare-tunnel-setup-guide" => Some(
            r#"Guide for Cloudflare tunnel preview. Required System Settings: Account ID, API Token, Zone ID, Base Domain.
- Output PREVIEW_TARGET: http://127.0.0.1:<port> when preview needed.
- When tunnel fails: tell user to ensure all 4 fields in System Settings (/settings)."#,
        ),
        "deploy-cloudflare-pages" => Some(
            r#"Deploy web build to Cloudflare Pages flow configured for this project.
- Validate deploy command/config.
- Capture resulting deployment URL."#,
        ),
        "deploy-cloudflare-workers" => Some(
            r#"Deploy API runtime to Cloudflare Workers flow configured for this project.
- Validate worker config.
- Capture resulting deployment URL/endpoint."#,
        ),
        "setup-cloudflare-tunnel" => Some(
            r#"Prepare preview tunnel details for web/api.
- Produce PREVIEW_TARGET for runtime endpoint.
- If public URL is available, output PREVIEW_URL."#,
        ),
        "deploy-precheck-cloudflare" => Some(
            r#"Before deploy/tunnel, verify Cloudflare settings are configured.
- If missing, report: cloudflare not configured.
- Skip deploy/tunnel safely and continue normal completion flow."#,
        ),
        "cloudflare-dns-route" => Some(
            r#"Ensure DNS route points to deployed target.
- Create/update record idempotently.
- Report hostname, record type, and status."#,
        ),
        "post-deploy-smoke-and-healthcheck" => Some(
            r#"Run health and smoke checks after deployment.
- Validate critical endpoints.
- Report validation status and rollback recommendation."#,
        ),
        "update-deployment-metadata" => Some(
            r#"Emit metadata-aligned deployment summary fields.
- Include deployment_status and production_deployment_status.
- Include errors/reasons when skipped or failed."#,
        ),
        "review-handoff" => Some(
            r#"Prepare reviewer handoff when require-review mode is enabled.
- Do not commit or push in review mode.
- Report changed files, risks, and reviewer actions."#,
        ),
        "gitlab-branch-and-commit" => Some(
            r#"Perform branch, stage, commit, and push workflow safely.
- Stage only task-related files.
- Report commit hash and push status."#,
        ),
        "gitlab-ci-verify" => Some(
            r#"Check CI pipeline status for pushed changes.
- Report pass/fail/pending with pipeline link or context."#,
        ),
        "gitlab-merge-request" => Some(
            r#"Create or update merge request with summary, verification, and deployment notes.
- Avoid duplicate MR creation.
- Report MR URL and action."#,
        ),
        "gitlab-issue-sync" => Some(
            r#"Sync completion status back to linked issue when available.
- Include MR/deploy links and blockers.
- Skip clearly if no issue reference exists."#,
        ),
        "retry-triage-and-recovery" => Some(
            r#"Classify failures and choose safe retry action.
- Retry only transient failures.
- Report recovery actions and retry decision."#,
        ),
        "gitlab-rebase-conflict-resolution" => Some(
            r#"Resolve branch divergence and rebase conflicts.
- Fetch origin, then ALWAYS rebase onto origin/main (do not skip; "Already up to date" from fetch ≠ branch integrated).
- Resolve conflicts using task intent as tie-breaker.
- Verify and push (--force-with-lease if rebased). Do NOT suggest "retry on GitLab"—you must run the commands and fix conflicts."#,
        ),
        "db-migration-safety" => Some(
            r#"Apply safe migration strategy with backward compatibility.
- Prefer additive changes.
- Document rollback plan and migration risk."#,
        ),
        "release-note-and-delivery-summary" => Some(
            r#"Produce release-ready delivery summary.
- Include code, validation, deploy status, and follow-ups.
- Keep summary concise and evidence-based."#,
        ),
        "final-report" => Some(
            r#"End with a final report section:
- Task summary
- Deployment status
- Commands executed
- URLs/endpoints
- Verification results
- Remaining risks/issues"#,
        ),
        "rollback-deploy" => Some(
            r#"If deployment is unsafe or broken, rollback to previous stable deployment reference.
- Report rollback target and reason."#,
        ),
        "deploy-ssh-remote" => Some(
            r#"Build artifact and deploy directly via SSH.
- Run project build. Verify artifact exists. Run tests if available.
- Use .acpms/deploy/ssh_key and .acpms/deploy/config.json to SSH to server.
- Copy artifact (rsync/scp) to deploy_path. Run deploy script if needed.
- Report build_status, artifact_paths, deployment_status in final report."#,
        ),
        "deploy-cancel-stop-cleanup" => Some(
            r#"Cancel deploy, stop containers/processes, clean resources.
- Cancel ACPMS run via UI (Deployments tab → Cancel) or API if token available.
- Use .acpms/deploy/ to SSH; run docker compose down, docker stop, pkill as needed.
- Remove resources only when task explicitly asks. Report cleanup_status."#,
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_skills_from_metadata_are_loaded() {
        let metadata = serde_json::json!({
            "skills": ["foo"],
            "execution": {
                "skills": ["bar"],
                "skill_chain": ["baz"]
            }
        });
        let skills = extract_explicit_skills(&metadata);
        assert_eq!(skills, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn unsafe_skill_id_is_rejected() {
        assert!(is_safe_skill_id("deploy-cloudflare-pages"));
        assert!(!is_safe_skill_id("../escape"));
        assert!(!is_safe_skill_id("UPPER"));
    }

    #[test]
    fn db_migration_skill_uses_metadata_hint() {
        let task = Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Add users index".to_string(),
            description: None,
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({
                "execution": {
                    "database_migration": true
                }
            }),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(task_mentions_database_changes(&task));
    }

    #[test]
    fn binary_preview_projects_add_project_specific_preview_skill() {
        let task = Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Ship desktop preview".to_string(),
            description: None,
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({}),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let settings = ProjectSettings {
            preview_enabled: true,
            ..ProjectSettings::default()
        };

        let skills = resolve_skill_chain(&task, &settings, ProjectType::Desktop);

        assert!(skills.iter().any(|skill| skill == "build-artifact"));
        assert!(skills
            .iter()
            .any(|skill| skill == "preview-artifact-desktop"));
    }

    #[test]
    fn project_preview_alias_enables_live_preview_skill_chain() {
        let task = Task {
            id: uuid::Uuid::new_v4(),
            project_id: uuid::Uuid::new_v4(),
            requirement_id: None,
            sprint_id: None,
            title: "Ship api preview".to_string(),
            description: None,
            task_type: TaskType::Feature,
            status: acpms_db::models::TaskStatus::Todo,
            assigned_to: None,
            parent_task_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({}),
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let settings = ProjectSettings {
            auto_deploy: false,
            preview_enabled: true,
            ..ProjectSettings::default()
        };

        let skills = resolve_skill_chain(&task, &settings, ProjectType::Api);

        assert!(skills
            .iter()
            .any(|skill| skill == "deploy-cloudflare-workers"));
    }
}
