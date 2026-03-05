use super::*;
use acpms_db::models::{Project, ProjectSettings, ProjectType, Task};
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio::sync::watch;
use uuid::Uuid;

const MAX_VALIDATION_OUTPUT_CHARS: usize = 12_000;
const MAX_VALIDATION_FIX_ROUNDS: usize = 2;
const MAX_DEPLOY_VALIDATION_COMMAND_TIMEOUT: Duration = Duration::from_secs(8 * 60);

fn architecture_is_empty(config: &serde_json::Value) -> bool {
    config
        .get("nodes")
        .and_then(|nodes| nodes.as_array())
        .map(|nodes| nodes.is_empty())
        .unwrap_or(true)
}

fn architecture_node_count(config: &serde_json::Value) -> usize {
    config
        .get("nodes")
        .and_then(|nodes| nodes.as_array())
        .map(|nodes| nodes.len())
        .unwrap_or(0)
}

fn architecture_is_legacy_frontend_only(config: &serde_json::Value) -> bool {
    let Some(nodes) = config.get("nodes").and_then(|value| value.as_array()) else {
        return false;
    };
    if nodes.len() != 2 {
        return false;
    }

    let has_browser = nodes
        .iter()
        .any(|node| node.get("id").and_then(|value| value.as_str()) == Some("browser"));
    let has_frontend = nodes
        .iter()
        .any(|node| node.get("id").and_then(|value| value.as_str()) == Some("frontend"));
    if !has_browser || !has_frontend {
        return false;
    }

    let Some(edges) = config.get("edges").and_then(|value| value.as_array()) else {
        return true;
    };

    if edges.is_empty() {
        return true;
    }

    edges.len() == 1
        && edges[0].get("source").and_then(|value| value.as_str()) == Some("browser")
        && edges[0].get("target").and_then(|value| value.as_str()) == Some("frontend")
}

fn architecture_node(id: &str, label: &str, node_type: &str) -> serde_json::Value {
    json!({
        "id": id,
        "label": label,
        "type": node_type,
        "status": "healthy"
    })
}

fn architecture_edge(source: &str, target: &str, label: &str) -> serde_json::Value {
    json!({
        "source": source,
        "target": target,
        "label": label
    })
}

fn push_architecture_node(
    nodes: &mut Vec<serde_json::Value>,
    id: &str,
    label: &str,
    node_type: &str,
) {
    let exists = nodes
        .iter()
        .any(|node| node.get("id").and_then(|value| value.as_str()) == Some(id));
    if !exists {
        nodes.push(architecture_node(id, label, node_type));
    }
}

fn push_architecture_edge(
    edges: &mut Vec<serde_json::Value>,
    source: &str,
    target: &str,
    label: &str,
) {
    let exists = edges.iter().any(|edge| {
        edge.get("source").and_then(|value| value.as_str()) == Some(source)
            && edge.get("target").and_then(|value| value.as_str()) == Some(target)
    });
    if !exists {
        edges.push(architecture_edge(source, target, label));
    }
}

fn truncate_for_requirement_summary(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= max_chars {
        return trimmed.to_string();
    }

    let mut cut = max_chars;
    while cut > 0 && !trimmed.is_char_boundary(cut) {
        cut -= 1;
    }

    let mut out = trimmed[..cut].to_string();
    out.push_str("...");
    out
}

fn parse_compact_stack_tokens(raw: &str) -> Vec<String> {
    raw.split(['|', ','])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            segment
                .rsplit_once(':')
                .map(|(_, stack)| stack)
                .unwrap_or(segment)
                .trim()
                .to_string()
        })
        .filter(|segment| !segment.is_empty())
        .collect()
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PackageJsonLite {
    scripts: Option<HashMap<String, String>>,
    dependencies: Option<HashMap<String, serde_json::Value>>,
    dev_dependencies: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone)]
struct ValidationOutcome {
    command: String,
    success: bool,
    exit_code: Option<i32>,
    output: String,
}

impl ExecutorOrchestrator {
    fn truncate_validation_output(text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            return text.to_string();
        }

        let mut cut = max_chars;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }

        let mut out = text[..cut].to_string();
        out.push_str("\n... (truncated)");
        out
    }

    fn normalize_failure_output(output: &str) -> String {
        let lower = output.to_lowercase();
        let mut normalized = String::with_capacity(lower.len());
        let mut prev_space = false;

        for ch in lower.chars() {
            let mapped = if ch.is_ascii_digit() {
                '#'
            } else if ch.is_whitespace() {
                ' '
            } else {
                ch
            };

            if mapped == ' ' {
                if !prev_space {
                    normalized.push(' ');
                    prev_space = true;
                }
            } else {
                normalized.push(mapped);
                prev_space = false;
            }
        }

        normalized.trim().to_string()
    }

    fn build_validation_failure_signature(failure: &ValidationOutcome) -> (String, String) {
        (
            failure.command.clone(),
            Self::normalize_failure_output(&failure.output),
        )
    }

    fn parse_package_json_lite(path: &Path) -> Option<PackageJsonLite> {
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str::<PackageJsonLite>(&raw).ok()
    }

    fn package_has_script(package_json: Option<&PackageJsonLite>, script: &str) -> bool {
        package_json
            .and_then(|pkg| pkg.scripts.as_ref())
            .map(|scripts| scripts.contains_key(script))
            .unwrap_or(false)
    }

    fn package_has_dependency(package_json: Option<&PackageJsonLite>, dep_name: &str) -> bool {
        let has_dep_map = |deps: Option<&HashMap<String, serde_json::Value>>| {
            deps.map(|m| m.contains_key(dep_name)).unwrap_or(false)
        };

        package_json
            .map(|pkg| {
                has_dep_map(pkg.dependencies.as_ref()) || has_dep_map(pkg.dev_dependencies.as_ref())
            })
            .unwrap_or(false)
    }

    fn collect_post_init_validation_commands(
        &self,
        project: &Project,
        worktree_path: &Path,
    ) -> Vec<String> {
        let mut commands = Vec::new();

        let package_json_path = worktree_path.join("package.json");
        let package_json = if package_json_path.exists() {
            Self::parse_package_json_lite(&package_json_path)
        } else {
            None
        };

        if package_json_path.exists() {
            commands.push("npm install".to_string());

            if Self::package_has_script(package_json.as_ref(), "typecheck") {
                commands.push("npm run typecheck".to_string());
            }

            if Self::package_has_script(package_json.as_ref(), "build") {
                commands.push("npm run build".to_string());
            } else if Self::package_has_script(package_json.as_ref(), "check") {
                commands.push("npm run check".to_string());
            }

            let should_check_electron = matches!(project.project_type, ProjectType::Desktop)
                || Self::package_has_dependency(package_json.as_ref(), "electron");
            if should_check_electron {
                commands.push(
                    "node -e \"require('electron'); console.log('electron-ok')\"".to_string(),
                );
            }
        }

        let src_tauri_manifest = worktree_path.join("src-tauri").join("Cargo.toml");
        if src_tauri_manifest.exists() {
            commands.push("cargo check --manifest-path src-tauri/Cargo.toml".to_string());
        } else if commands.is_empty() && worktree_path.join("Cargo.toml").exists() {
            commands.push("cargo check".to_string());
        } else if commands.is_empty() && worktree_path.join("go.mod").exists() {
            commands.push("go test ./...".to_string());
        }

        // Preserve order while deduplicating.
        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        for command in commands {
            if seen.insert(command.clone()) {
                deduped.push(command);
            }
        }

        deduped
    }

    fn collect_deploy_validation_commands(&self, project: &Project) -> Vec<String> {
        let mut commands: Vec<String> = Vec::new();

        let push_metadata_commands = |commands: &mut Vec<String>, value: &serde_json::Value| {
            if let Some(items) = value.as_array() {
                for item in items {
                    if let Some(command) = item.as_str() {
                        let trimmed = command.trim();
                        if !trimmed.is_empty() {
                            commands.push(trimmed.to_string());
                        }
                    }
                }
            }
        };

        if let Some(value) = project.metadata.get("deploy_validation_commands") {
            push_metadata_commands(&mut commands, value);
        }
        if let Some(value) = project.metadata.get("deployValidationCommands") {
            push_metadata_commands(&mut commands, value);
        }

        if commands.is_empty() && matches!(project.project_type, ProjectType::Web) {
            commands.push(
                "if [ -f docker-compose.yml ]; then docker compose -f docker-compose.yml config -q; \
elif [ -f docker-compose.yaml ]; then docker compose -f docker-compose.yaml config -q; \
elif [ -f compose.yml ]; then docker compose -f compose.yml config -q; \
elif [ -f compose.yaml ]; then docker compose -f compose.yaml config -q; \
elif [ -f Dockerfile ]; then docker build -t acpms-preview-check .; \
else echo 'Missing Dockerfile or compose file (docker-compose.yml / compose.yml)'; exit 1; fi"
                    .to_string(),
            );
        }

        // Preserve order while deduplicating.
        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        for command in commands {
            if seen.insert(command.clone()) {
                deduped.push(command);
            }
        }

        deduped
    }

    async fn run_validation_command(
        &self,
        attempt_id: Uuid,
        worktree_path: &Path,
        command: &str,
    ) -> Result<ValidationOutcome> {
        self.log(
            attempt_id,
            "system",
            &format!("🧪 Running post-init validation command: {}", command),
        )
        .await?;

        let output = Command::new("sh")
            .arg("-lc")
            .arg(command)
            .current_dir(worktree_path)
            .output()
            .await
            .with_context(|| format!("Failed to execute validation command: {}", command))?;

        let stdout = sanitize_log(&String::from_utf8_lossy(&output.stdout));
        let stderr = sanitize_log(&String::from_utf8_lossy(&output.stderr));
        let mut combined = String::new();

        if !stdout.trim().is_empty() {
            combined.push_str("STDOUT:\n");
            combined.push_str(stdout.trim_end());
            combined.push('\n');
        }
        if !stderr.trim().is_empty() {
            combined.push_str("STDERR:\n");
            combined.push_str(stderr.trim_end());
            combined.push('\n');
        }

        let truncated = Self::truncate_validation_output(&combined, MAX_VALIDATION_OUTPUT_CHARS);
        let success = output.status.success();
        let exit_code = output.status.code();

        if success {
            self.log(
                attempt_id,
                "system",
                &format!("✅ Validation passed: {}", command),
            )
            .await?;
        } else {
            self.log(
                attempt_id,
                "stderr",
                &format!(
                    "❌ Validation failed: {} (exit code: {:?})",
                    command, exit_code
                ),
            )
            .await?;

            if !truncated.trim().is_empty() {
                self.log(
                    attempt_id,
                    "stderr",
                    &format!("Validation output for `{}`:\n{}", command, truncated),
                )
                .await?;
            }
        }

        Ok(ValidationOutcome {
            command: command.to_string(),
            success,
            exit_code,
            output: truncated,
        })
    }

    async fn run_deployment_validation_command(
        &self,
        attempt_id: Uuid,
        worktree_path: &Path,
        command: &str,
        command_timeout: Duration,
    ) -> Result<ValidationOutcome> {
        self.log(
            attempt_id,
            "system",
            &format!("🚀 Running deployment validation command: {}", command),
        )
        .await?;

        let output = match tokio::time::timeout(
            command_timeout,
            Command::new("sh")
                .arg("-lc")
                .arg(command)
                .current_dir(worktree_path)
                .output(),
        )
        .await
        {
            Ok(result) => result.with_context(|| {
                format!(
                    "Failed to execute deployment validation command: {}",
                    command
                )
            })?,
            Err(_) => {
                bail!(
                    "Deployment validation command timed out after {:?}: {}",
                    command_timeout,
                    command
                );
            }
        };

        let stdout = sanitize_log(&String::from_utf8_lossy(&output.stdout));
        let stderr = sanitize_log(&String::from_utf8_lossy(&output.stderr));
        let mut combined = String::new();

        if !stdout.trim().is_empty() {
            combined.push_str("STDOUT:\n");
            combined.push_str(stdout.trim_end());
            combined.push('\n');
        }
        if !stderr.trim().is_empty() {
            combined.push_str("STDERR:\n");
            combined.push_str(stderr.trim_end());
            combined.push('\n');
        }

        let truncated = Self::truncate_validation_output(&combined, MAX_VALIDATION_OUTPUT_CHARS);
        let success = output.status.success();
        let exit_code = output.status.code();

        if success {
            self.log(
                attempt_id,
                "system",
                &format!("✅ Deployment validation passed: {}", command),
            )
            .await?;
        } else {
            self.log(
                attempt_id,
                "stderr",
                &format!(
                    "❌ Deployment validation failed: {} (exit code: {:?})",
                    command, exit_code
                ),
            )
            .await?;

            if !truncated.trim().is_empty() {
                self.log(
                    attempt_id,
                    "stderr",
                    &format!(
                        "Deployment validation output for `{}`:\n{}",
                        command, truncated
                    ),
                )
                .await?;
            }
        }

        Ok(ValidationOutcome {
            command: command.to_string(),
            success,
            exit_code,
            output: truncated,
        })
    }

    fn build_post_init_fix_instruction(
        &self,
        project: &Project,
        commands: &[String],
        failed: &ValidationOutcome,
        current_round: usize,
        max_rounds: usize,
    ) -> String {
        let validation_list = commands
            .iter()
            .map(|cmd| format!("- `{}`", cmd))
            .collect::<Vec<_>>()
            .join("\n");
        let output = if failed.output.trim().is_empty() {
            "(no output captured)".to_string()
        } else {
            failed.output.clone()
        };

        format!(
            r#"## Post-init validation fix ({}/{})

The initialization is NOT complete yet. Fix the scaffold so it passes the post-init validation gate.

Project: {}
Project type: {}

### Validation commands
{}

### Current failing command
`{}` (exit code: {:?})

### Error output
```text
{}
```

### Required actions
1. Fix code/config/dependencies so all validation commands pass.
2. Re-run the failing command(s) yourself before finishing.
3. Commit and push fixes to the same repository.
4. Do not create a new repository.
5. Summarize root cause and fixes in final output.
"#,
            current_round,
            max_rounds,
            project.name,
            project.project_type.display_name(),
            validation_list,
            failed.command,
            failed.exit_code,
            output
        )
    }

    fn build_deployment_fix_instruction(
        &self,
        project: &Project,
        commands: &[String],
        failed: &ValidationOutcome,
        current_round: usize,
        max_rounds: usize,
        worktree_path: &Path,
    ) -> String {
        let validation_list = commands
            .iter()
            .map(|cmd| format!("- `{}`", cmd))
            .collect::<Vec<_>>()
            .join("\n");
        let output = if failed.output.trim().is_empty() {
            "(no output captured)".to_string()
        } else {
            failed.output.clone()
        };

        let docker_hints = if failed.command.to_lowercase().contains("docker") {
            let compose_files = [
                "docker-compose.yml",
                "docker-compose.yaml",
                "compose.yml",
                "compose.yaml",
            ];
            let existing: Vec<_> = compose_files
                .iter()
                .filter(|f| worktree_path.join(*f).exists())
                .map(|s| s.to_string())
                .collect();
            format!(
                r#"

### Docker-specific fixes (command failed: {})
1. Ensure Docker daemon is running: `docker info` or `docker ps`.
2. Check compose file syntax: `docker compose -f <file> config` shows validation errors.
3. Fix compose/Dockerfile: service names, image references, volumes, ports, env vars.
4. Common issues: missing `build:` context, invalid YAML, wrong file path.
5. Compose files present: {:?}. Fix the one used by the failing command.
"#,
                failed.command,
                if existing.is_empty() {
                    "none found".to_string()
                } else {
                    existing.join(", ")
                }
            )
        } else {
            String::new()
        };

        format!(
            r#"## Deployment validation fix ({}/{}).

Deployment validation is failing for this task attempt.

Project: {}
Project type: {}

### Deployment validation commands
{}

### Current failing command
`{}` (exit code: {:?})

### Error output
```text
{}
```
{}
### Required actions
1. Fix deployment-related files and configs (Dockerfile, compose files, startup scripts, env wiring).
2. Ensure the app can be deployed for preview/runtime.
3. Start preview runtime and output `PREVIEW_TARGET: http://127.0.0.1:<port>` in your final message.
4. Re-run the failing deployment command(s) before finishing.
5. Commit and push fixes to the same branch/repository.
6. Summarize root cause and deployment fixes in final output.
"#,
            current_round,
            max_rounds,
            project.name,
            project.project_type.display_name(),
            validation_list,
            failed.command,
            failed.exit_code,
            output,
            docker_hints
        )
    }

    async fn is_command_available(&self, worktree_path: &Path, command_name: &str) -> bool {
        Command::new("sh")
            .arg("-lc")
            .arg(format!("command -v {} >/dev/null 2>&1", command_name))
            .current_dir(worktree_path)
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn repo_has_pattern(repo_files: &[String], patterns: &[&str]) -> bool {
        repo_files
            .iter()
            .map(|path| path.to_lowercase())
            .any(|path| patterns.iter().any(|pattern| path.contains(pattern)))
    }

    fn collect_project_stack_tokens(metadata: &serde_json::Value) -> Vec<String> {
        let mut tokens: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut push_token = |raw: &str| {
            let normalized = raw.trim().to_lowercase();
            if normalized.is_empty() {
                return;
            }
            if seen.insert(normalized.clone()) {
                tokens.push(normalized);
            }
        };

        if let Some(compact) = metadata.get("tech_stack").and_then(|value| value.as_str()) {
            for token in parse_compact_stack_tokens(compact) {
                push_token(&token);
            }
        } else if let Some(compact) = metadata.get("techStack").and_then(|value| value.as_str()) {
            for token in parse_compact_stack_tokens(compact) {
                push_token(&token);
            }
        }

        let push_array_tokens = |value: &serde_json::Value, push_token: &mut dyn FnMut(&str)| {
            if let Some(items) = value.as_array() {
                for item in items {
                    if let Some(text) = item.as_str() {
                        push_token(text);
                    }
                }
            }
        };

        if let Some(value) = metadata.get("techStack") {
            push_array_tokens(value, &mut push_token);
        }
        if let Some(value) = metadata.get("tech_stack") {
            push_array_tokens(value, &mut push_token);
        }

        let push_stack_selections =
            |value: &serde_json::Value, push_token: &mut dyn FnMut(&str)| {
                if let Some(items) = value.as_array() {
                    for item in items {
                        if let Some(layer) = item.get("layer").and_then(|value| value.as_str()) {
                            push_token(layer);
                        }
                        if let Some(stack) = item.get("stack").and_then(|value| value.as_str()) {
                            push_token(stack);
                        }
                    }
                }
            };

        if let Some(value) = metadata.get("stack_selections") {
            push_stack_selections(value, &mut push_token);
        }
        if let Some(value) = metadata.get("stackSelections") {
            push_stack_selections(value, &mut push_token);
        }

        tokens
    }

    fn stack_tokens_match(tokens: &[String], patterns: &[&str]) -> bool {
        tokens.iter().any(|token| {
            patterns
                .iter()
                .any(|pattern| token.contains(&pattern.to_ascii_lowercase()))
        })
    }

    fn detect_stack_hints(repo_files: &[String], project_type: ProjectType) -> Vec<String> {
        let mut hints: Vec<String> = Vec::new();
        let push_hint = |hints: &mut Vec<String>, value: &str| {
            if !hints
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(value))
            {
                hints.push(value.to_string());
            }
        };

        if Self::repo_has_pattern(repo_files, &["src-tauri", "tauri.conf"]) {
            push_hint(&mut hints, "Tauri");
        }
        if Self::repo_has_pattern(repo_files, &["electron", "electron-builder", "main.js"]) {
            push_hint(&mut hints, "Electron");
        }
        if Self::repo_has_pattern(repo_files, &["next.config", "app/page.tsx", "pages/"]) {
            push_hint(&mut hints, "Next.js");
        }
        if Self::repo_has_pattern(repo_files, &["vite.config", "index.html"]) {
            push_hint(&mut hints, "Vite");
        }
        if Self::repo_has_pattern(repo_files, &["src/main.tsx", "src/app.tsx", ".tsx"]) {
            push_hint(&mut hints, "React");
        }
        if Self::repo_has_pattern(repo_files, &["src/main.vue", ".vue", "nuxt.config"]) {
            push_hint(&mut hints, "Vue");
        }
        if Self::repo_has_pattern(
            repo_files,
            &["fastapi", "pyproject.toml", "requirements.txt"],
        ) {
            push_hint(&mut hints, "FastAPI");
        }
        if Self::repo_has_pattern(repo_files, &["nestjs", "nest-cli", "main.ts"]) {
            push_hint(&mut hints, "NestJS");
        }
        if Self::repo_has_pattern(repo_files, &["prisma/schema.prisma", "migrations/", ".sql"]) {
            push_hint(&mut hints, "PostgreSQL");
        }
        if Self::repo_has_pattern(repo_files, &["sqlite", ".db"]) {
            push_hint(&mut hints, "SQLite");
        }
        if Self::repo_has_pattern(repo_files, &["redis"]) {
            push_hint(&mut hints, "Redis");
        }
        if Self::repo_has_pattern(repo_files, &["dockerfile", "docker-compose"]) {
            push_hint(&mut hints, "Docker");
        }

        if hints.is_empty() {
            match project_type {
                ProjectType::Web => push_hint(&mut hints, "React + Vite"),
                ProjectType::Mobile => push_hint(&mut hints, "React Native"),
                ProjectType::Desktop => push_hint(&mut hints, "Tauri"),
                ProjectType::Extension => push_hint(&mut hints, "Browser Extension"),
                ProjectType::Api => push_hint(&mut hints, "REST API"),
                ProjectType::Microservice => push_hint(&mut hints, "Docker"),
            }
        }

        hints
    }

    fn build_architecture_config_from_repo(
        &self,
        project_type: ProjectType,
        project_metadata: &serde_json::Value,
        repo_files: &[String],
    ) -> serde_json::Value {
        let stack_tokens = Self::collect_project_stack_tokens(project_metadata);

        let has_frontend = matches!(
            project_type,
            ProjectType::Web | ProjectType::Desktop | ProjectType::Extension
        ) || Self::repo_has_pattern(
            repo_files,
            &[
                "src/main.tsx",
                "src/app.tsx",
                "index.html",
                ".vue",
                ".svelte",
                "frontend/",
            ],
        ) || Self::stack_tokens_match(
            &stack_tokens,
            &[
                "frontend", "react", "vue", "svelte", "next", "nuxt", "angular", "vite",
            ],
        );
        let has_api = matches!(project_type, ProjectType::Api | ProjectType::Microservice)
            || Self::repo_has_pattern(
                repo_files,
                &[
                    "server/",
                    "api/",
                    "backend/",
                    "src/main.rs",
                    "main.py",
                    "controllers/",
                ],
            )
            || Self::stack_tokens_match(
                &stack_tokens,
                &[
                    "backend", "api", "server", "express", "nestjs", "fastapi", "django", "flask",
                    "spring", "laravel", "dotnet", "asp.net", "axum", "actix", "hono", "fiber",
                    "gin",
                ],
            );
        let has_database = Self::repo_has_pattern(
            repo_files,
            &[
                "prisma/schema.prisma",
                "migrations/",
                "database/",
                ".sql",
                "sqlite",
                "db/",
            ],
        ) || Self::stack_tokens_match(
            &stack_tokens,
            &[
                "database",
                "postgres",
                "postgresql",
                "mysql",
                "mariadb",
                "sqlite",
                "mongo",
                "dynamodb",
                "supabase",
                "prisma",
            ],
        );
        let has_cache = Self::repo_has_pattern(repo_files, &["redis", "cache/"])
            || Self::stack_tokens_match(&stack_tokens, &["cache", "redis", "memcached", "valkey"]);
        let has_queue = Self::repo_has_pattern(
            repo_files,
            &["queue/", "rabbit", "kafka", "bullmq", "sqs", "nats"],
        ) || Self::stack_tokens_match(
            &stack_tokens,
            &[
                "queue",
                "kafka",
                "rabbit",
                "bull",
                "sqs",
                "pubsub",
                "nats",
                "messaging",
            ],
        );
        let has_storage = Self::repo_has_pattern(
            repo_files,
            &["storage/", "uploads/", "s3", "bucket", "minio"],
        ) || Self::stack_tokens_match(
            &stack_tokens,
            &["storage", "s3", "bucket", "minio", "blob", "cloudinary"],
        );
        let has_auth = Self::repo_has_pattern(
            repo_files,
            &[
                "auth/", "oauth", "jwt", "nextauth", "clerk", "auth0", "keycloak",
            ],
        ) || Self::stack_tokens_match(
            &stack_tokens,
            &[
                "auth",
                "oauth",
                "jwt",
                "nextauth",
                "clerk",
                "auth0",
                "keycloak",
                "supabase auth",
                "firebase auth",
            ],
        );

        let mut nodes: Vec<serde_json::Value> = Vec::new();
        let mut edges: Vec<serde_json::Value> = Vec::new();

        match project_type {
            ProjectType::Web => {
                push_architecture_node(&mut nodes, "browser", "Browser Client", "client");
                push_architecture_node(&mut nodes, "frontend", "Web Frontend", "frontend");
                push_architecture_edge(&mut edges, "browser", "frontend", "HTTPS");

                let should_include_api =
                    has_api || has_database || has_cache || has_queue || has_auth || has_storage;
                if should_include_api {
                    push_architecture_node(&mut nodes, "api", "Application API", "api");
                    push_architecture_edge(&mut edges, "frontend", "api", "REST/GraphQL");

                    if has_auth {
                        push_architecture_node(&mut nodes, "auth", "Auth Provider", "auth");
                        push_architecture_edge(&mut edges, "frontend", "auth", "OIDC/OAuth");
                        push_architecture_edge(&mut edges, "api", "auth", "Token Verify");
                    }
                    if has_database {
                        push_architecture_node(
                            &mut nodes,
                            "database",
                            "Primary Database",
                            "database",
                        );
                        push_architecture_edge(&mut edges, "api", "database", "Read/Write");
                    }
                    if has_cache {
                        push_architecture_node(&mut nodes, "cache", "Cache Layer", "cache");
                        push_architecture_edge(&mut edges, "api", "cache", "Cache");
                    }
                    if has_queue {
                        push_architecture_node(&mut nodes, "queue", "Async Queue", "queue");
                        push_architecture_edge(&mut edges, "api", "queue", "Jobs");
                    }
                    if has_storage {
                        push_architecture_node(&mut nodes, "storage", "Object Storage", "storage");
                        push_architecture_edge(&mut edges, "api", "storage", "File Assets");
                    }
                } else if has_database {
                    push_architecture_node(&mut nodes, "database", "Primary Database", "database");
                    push_architecture_edge(&mut edges, "frontend", "database", "Data Access");
                }
            }
            ProjectType::Desktop => {
                push_architecture_node(&mut nodes, "desktop-ui", "Desktop Shell", "client");
                push_architecture_node(&mut nodes, "desktop-core", "Desktop Core", "service");
                push_architecture_edge(&mut edges, "desktop-ui", "desktop-core", "IPC");

                if has_database || Self::repo_has_pattern(repo_files, &["sqlite", "src-tauri"]) {
                    push_architecture_node(&mut nodes, "local-db", "Local Database", "database");
                    push_architecture_edge(&mut edges, "desktop-core", "local-db", "Local Data");
                }
                if has_api {
                    push_architecture_node(&mut nodes, "remote-api", "Remote API", "api");
                    push_architecture_edge(&mut edges, "desktop-core", "remote-api", "Sync");
                    if has_auth {
                        push_architecture_node(&mut nodes, "auth", "Auth Provider", "auth");
                        push_architecture_edge(&mut edges, "remote-api", "auth", "Token Verify");
                    }
                    if has_storage {
                        push_architecture_node(&mut nodes, "storage", "Object Storage", "storage");
                        push_architecture_edge(&mut edges, "remote-api", "storage", "Asset I/O");
                    }
                }
            }
            ProjectType::Mobile => {
                push_architecture_node(&mut nodes, "mobile-app", "Mobile App", "mobile");
                push_architecture_node(&mut nodes, "api", "Backend API", "api");
                push_architecture_edge(&mut edges, "mobile-app", "api", "HTTPS");

                if has_auth {
                    push_architecture_node(&mut nodes, "auth", "Auth Provider", "auth");
                    push_architecture_edge(&mut edges, "mobile-app", "auth", "OIDC/OAuth");
                    push_architecture_edge(&mut edges, "api", "auth", "Token Verify");
                }

                if has_database {
                    push_architecture_node(&mut nodes, "database", "Primary Database", "database");
                    push_architecture_edge(&mut edges, "api", "database", "Read/Write");
                }
                if has_cache {
                    push_architecture_node(&mut nodes, "cache", "Cache Layer", "cache");
                    push_architecture_edge(&mut edges, "api", "cache", "Cache");
                }
                if has_queue {
                    push_architecture_node(&mut nodes, "queue", "Async Queue", "queue");
                    push_architecture_edge(&mut edges, "api", "queue", "Jobs");
                }
                if has_storage {
                    push_architecture_node(&mut nodes, "storage", "Object Storage", "storage");
                    push_architecture_edge(&mut edges, "api", "storage", "Media Upload");
                }
            }
            ProjectType::Extension => {
                push_architecture_node(&mut nodes, "browser-ext", "Browser Extension", "frontend");
                push_architecture_node(&mut nodes, "bg-worker", "Background Worker", "worker");
                push_architecture_edge(&mut edges, "browser-ext", "bg-worker", "Events");

                if has_storage {
                    push_architecture_node(&mut nodes, "storage", "Persistent Storage", "storage");
                    push_architecture_edge(&mut edges, "bg-worker", "storage", "Read/Write");
                }
                if has_api {
                    push_architecture_node(&mut nodes, "api", "Remote API", "api");
                    push_architecture_edge(&mut edges, "bg-worker", "api", "HTTPS");
                    if has_auth {
                        push_architecture_node(&mut nodes, "auth", "Auth Provider", "auth");
                        push_architecture_edge(&mut edges, "api", "auth", "Token Verify");
                    }
                }
            }
            ProjectType::Api => {
                push_architecture_node(&mut nodes, "api-gateway", "API Gateway", "gateway");
                push_architecture_node(&mut nodes, "app-service", "Application Service", "service");
                push_architecture_edge(&mut edges, "api-gateway", "app-service", "Request");

                if has_database || !has_frontend {
                    push_architecture_node(&mut nodes, "database", "Primary Database", "database");
                    push_architecture_edge(&mut edges, "app-service", "database", "Read/Write");
                }
                if has_cache {
                    push_architecture_node(&mut nodes, "cache", "Cache Layer", "cache");
                    push_architecture_edge(&mut edges, "app-service", "cache", "Cache");
                }
                if has_queue {
                    push_architecture_node(&mut nodes, "queue", "Async Queue", "queue");
                    push_architecture_edge(&mut edges, "app-service", "queue", "Jobs");
                }
                if has_auth {
                    push_architecture_node(&mut nodes, "auth", "Auth Provider", "auth");
                    push_architecture_edge(&mut edges, "api-gateway", "auth", "OIDC/OAuth");
                    push_architecture_edge(&mut edges, "app-service", "auth", "Token Verify");
                }
                if has_storage {
                    push_architecture_node(&mut nodes, "storage", "Object Storage", "storage");
                    push_architecture_edge(&mut edges, "app-service", "storage", "File Assets");
                }
            }
            ProjectType::Microservice => {
                push_architecture_node(&mut nodes, "gateway", "Ingress Gateway", "gateway");
                push_architecture_node(&mut nodes, "service-a", "Core Service", "service");
                push_architecture_edge(&mut edges, "gateway", "service-a", "HTTP/gRPC");

                push_architecture_node(&mut nodes, "database", "Service Database", "database");
                push_architecture_edge(&mut edges, "service-a", "database", "Read/Write");

                if has_queue || !has_frontend {
                    push_architecture_node(&mut nodes, "queue", "Event Bus", "queue");
                    push_architecture_edge(&mut edges, "service-a", "queue", "Publish");
                }
                if has_cache {
                    push_architecture_node(&mut nodes, "cache", "Cache Layer", "cache");
                    push_architecture_edge(&mut edges, "service-a", "cache", "Cache");
                }
                if has_auth {
                    push_architecture_node(&mut nodes, "auth", "Identity Service", "auth");
                    push_architecture_edge(&mut edges, "gateway", "auth", "OIDC/OAuth");
                }
                if has_storage {
                    push_architecture_node(&mut nodes, "storage", "Object Storage", "storage");
                    push_architecture_edge(&mut edges, "service-a", "storage", "Blob I/O");
                }
            }
        }

        if nodes.is_empty() {
            push_architecture_node(&mut nodes, "app", "Application Core", "service");
        }

        json!({
            "nodes": nodes,
            "edges": edges
        })
    }

    pub(super) async fn bootstrap_project_context_after_init(
        &self,
        task: &Task,
        project: &Project,
        attempt_id: Uuid,
        repo_path: &Path,
        project_type: ProjectType,
        emit_attempt_logs: bool,
    ) -> Result<()> {
        let repo_files = self.list_repo_files(repo_path);

        let generated_architecture =
            self.build_architecture_config_from_repo(project_type, &project.metadata, &repo_files);
        let should_upgrade_legacy_architecture =
            architecture_is_legacy_frontend_only(&project.architecture_config)
                && architecture_node_count(&generated_architecture)
                    > architecture_node_count(&project.architecture_config);

        if architecture_is_empty(&project.architecture_config) || should_upgrade_legacy_architecture
        {
            sqlx::query(
                "UPDATE projects SET architecture_config = $2, updated_at = NOW() WHERE id = $1",
            )
            .bind(project.id)
            .bind(&generated_architecture)
            .execute(&self.db_pool)
            .await
            .context("Failed to persist generated system architecture")?;

            if emit_attempt_logs {
                self.log(
                    attempt_id,
                    "system",
                    if should_upgrade_legacy_architecture {
                        "Upgraded System Architecture with detected backend/data components."
                    } else {
                        "Generated initial System Architecture from init scaffold."
                    },
                )
                .await?;
            }
        } else if emit_attempt_logs {
            self.log(
                attempt_id,
                "system",
                "System Architecture already exists, skipping auto-generation.",
            )
            .await?;
        }

        Ok(())
    }

    /// Ensure system architecture + PRD draft requirements are seeded for a project.
    ///
    /// This is a safe idempotent recovery path:
    /// - Finds the latest successful init attempt for the project
    /// - Resolves a repository path from attempt metadata/worktree root
    /// - Re-runs bootstrap generation only when data is still missing
    pub async fn ensure_project_context_seeded(&self, project_id: Uuid) -> Result<bool> {
        #[derive(sqlx::FromRow)]
        struct InitAttemptRow {
            attempt_id: Uuid,
            task_id: Uuid,
            metadata: serde_json::Value,
        }

        let Some(init_attempt) = sqlx::query_as::<_, InitAttemptRow>(
            r#"
            SELECT ta.id AS attempt_id, ta.task_id, ta.metadata
            FROM task_attempts ta
            JOIN tasks t ON t.id = ta.task_id
            WHERE t.project_id = $1
              AND t.task_type = 'init'
              AND ta.status = 'success'
            ORDER BY ta.completed_at DESC NULLS LAST, ta.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.db_pool)
        .await
        .context("Failed to lookup successful init attempt for project context seeding")?
        else {
            return Ok(false);
        };

        let task = self.fetch_task(init_attempt.task_id).await?;
        let project = self.fetch_project(project_id).await?;

        let repo_from_attempt = init_attempt
            .metadata
            .get("worktree_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .filter(|p| p.exists());

        let repo_from_slug =
            self.worktree_manager
                .base_path()
                .await
                .join(project_repo_relative_path(
                    project.id,
                    &project.metadata,
                    &project.name,
                ));
        let repo_from_slug = if repo_from_slug.exists() {
            Some(repo_from_slug)
        } else {
            None
        };

        let repo_path = match repo_from_attempt.or(repo_from_slug) {
            Some(path) => path,
            None => {
                return Ok(false);
            }
        };

        self.bootstrap_project_context_after_init(
            &task,
            &project,
            init_attempt.attempt_id,
            &repo_path,
            project.project_type,
            false,
        )
        .await?;

        Ok(true)
    }

    pub(super) async fn run_post_init_validation_with_auto_fix(
        &self,
        attempt_id: Uuid,
        project: &Project,
        worktree_path: &Path,
        provider: AgentCliProvider,
        agent_env: &HashMap<String, String>,
        task_timeout: Duration,
        project_settings: &ProjectSettings,
    ) -> Result<()> {
        let commands = self.collect_post_init_validation_commands(project, worktree_path);
        if commands.is_empty() {
            self.log(
                attempt_id,
                "system",
                "No post-init validation commands detected; skipping validation gate.",
            )
            .await?;
            return Ok(());
        }

        self.log(
            attempt_id,
            "system",
            &format!("Post-init validation plan: {}", commands.join("  ->  ")),
        )
        .await?;

        let retry_budget = project_settings.max_retries.max(0) as usize;
        let fix_rounds = retry_budget.clamp(1, MAX_VALIDATION_FIX_ROUNDS);
        let worktree_path_buf = worktree_path.to_path_buf();
        let mut last_failure_signature: Option<(String, String)> = None;

        for pass in 0..=fix_rounds {
            self.log(
                attempt_id,
                "system",
                &format!(
                    "Running post-init validation pass {}/{}.",
                    pass + 1,
                    fix_rounds + 1
                ),
            )
            .await?;

            let mut failed: Option<ValidationOutcome> = None;
            for command in &commands {
                let outcome = self
                    .run_validation_command(attempt_id, worktree_path, command)
                    .await?;
                if !outcome.success {
                    failed = Some(outcome);
                    break;
                }
            }

            if let Some(failure) = failed {
                let failure_signature = Self::build_validation_failure_signature(&failure);
                if last_failure_signature
                    .as_ref()
                    .map(|prev| prev == &failure_signature)
                    .unwrap_or(false)
                {
                    bail!(
                        "Post-init validation appears stuck in a loop on `{}` with unchanged error signature",
                        failure.command
                    );
                }
                last_failure_signature = Some(failure_signature);

                if pass == fix_rounds {
                    bail!(
                        "Validation command `{}` still failing after {} passes (exit code: {:?})",
                        failure.command,
                        fix_rounds + 1,
                        failure.exit_code
                    );
                }

                self.log(
                    attempt_id,
                    "system",
                    &format!(
                        "Validation failed on `{}`. Starting auto-fix round {}/{}.",
                        failure.command,
                        pass + 1,
                        fix_rounds
                    ),
                )
                .await?;

                let fix_instruction = self.build_post_init_fix_instruction(
                    project,
                    &commands,
                    &failure,
                    pass + 1,
                    fix_rounds,
                );
                let (_cancel_tx, cancel_rx) = watch::channel(false);
                let execute_fix = self.execute_agent(
                    attempt_id,
                    &worktree_path_buf,
                    &fix_instruction,
                    cancel_rx,
                    provider,
                    Some(agent_env.clone()),
                );

                match tokio::time::timeout(task_timeout, execute_fix).await {
                    Ok(Ok(())) => {
                        self.log(
                            attempt_id,
                            "system",
                            "Auto-fix round completed. Re-running validation...",
                        )
                        .await?;
                    }
                    Ok(Err(e)) => {
                        bail!("Auto-fix round failed: {}", e);
                    }
                    Err(_) => {
                        bail!(
                            "Auto-fix round timed out after {} minutes",
                            project_settings.timeout_mins
                        );
                    }
                }
            } else {
                self.log(
                    attempt_id,
                    "system",
                    "✅ Post-init validation passed. Scaffold is ready.",
                )
                .await?;
                return Ok(());
            }
        }

        bail!("Post-init validation failed")
    }

    pub(super) async fn run_deployment_validation_with_auto_fix(
        &self,
        attempt_id: Uuid,
        project: &Project,
        worktree_path: &Path,
        provider: AgentCliProvider,
        agent_env: &HashMap<String, String>,
        task_timeout: Duration,
        project_settings: &ProjectSettings,
    ) -> Result<()> {
        if !matches!(project.project_type, ProjectType::Web) {
            self.log(
                attempt_id,
                "system",
                &format!(
                    "Skipping deployment validation for project type {}.",
                    project.project_type.display_name()
                ),
            )
            .await?;
            return Ok(());
        }

        let commands = self.collect_deploy_validation_commands(project);
        if commands.is_empty() {
            self.log(
                attempt_id,
                "system",
                "No deployment validation commands configured; skipping deployment gate.",
            )
            .await?;
            return Ok(());
        }

        let requires_docker = commands.iter().any(|cmd| cmd.contains("docker "));
        if requires_docker && !self.is_command_available(worktree_path, "docker").await {
            self.log(
                attempt_id,
                "stderr",
                "Deployment validation requires Docker, but Docker is not available on this runner. Skipping deployment gate.",
            )
            .await?;
            return Ok(());
        }

        self.log(
            attempt_id,
            "system",
            &format!("Deployment validation plan: {}", commands.join("  ->  ")),
        )
        .await?;

        let retry_budget = project_settings.max_retries.max(0) as usize;
        let fix_rounds = retry_budget.clamp(1, MAX_VALIDATION_FIX_ROUNDS);
        let worktree_path_buf = worktree_path.to_path_buf();
        let mut last_failure_signature: Option<(String, String)> = None;
        let command_timeout = task_timeout.min(MAX_DEPLOY_VALIDATION_COMMAND_TIMEOUT);

        for pass in 0..=fix_rounds {
            self.log(
                attempt_id,
                "system",
                &format!(
                    "Running deployment validation pass {}/{}.",
                    pass + 1,
                    fix_rounds + 1
                ),
            )
            .await?;

            let mut failed: Option<ValidationOutcome> = None;
            for command in &commands {
                let outcome = self
                    .run_deployment_validation_command(
                        attempt_id,
                        worktree_path,
                        command,
                        command_timeout,
                    )
                    .await?;
                if !outcome.success {
                    failed = Some(outcome);
                    break;
                }
            }

            if let Some(failure) = failed {
                let failure_signature = Self::build_validation_failure_signature(&failure);
                if last_failure_signature
                    .as_ref()
                    .map(|prev| prev == &failure_signature)
                    .unwrap_or(false)
                {
                    bail!(
                        "Deployment validation appears stuck in a loop on `{}` with unchanged error signature",
                        failure.command
                    );
                }
                last_failure_signature = Some(failure_signature);

                if pass == fix_rounds {
                    bail!(
                        "Deployment validation command `{}` still failing after {} passes (exit code: {:?})",
                        failure.command,
                        fix_rounds + 1,
                        failure.exit_code
                    );
                }

                self.log(
                    attempt_id,
                    "system",
                    &format!(
                        "Deployment validation failed on `{}`. Starting auto-fix round {}/{}.",
                        failure.command,
                        pass + 1,
                        fix_rounds
                    ),
                )
                .await?;

                let fix_instruction = self.build_deployment_fix_instruction(
                    project,
                    &commands,
                    &failure,
                    pass + 1,
                    fix_rounds,
                    worktree_path,
                );
                let (_cancel_tx, cancel_rx) = watch::channel(false);
                let execute_fix = self.execute_agent(
                    attempt_id,
                    &worktree_path_buf,
                    &fix_instruction,
                    cancel_rx,
                    provider,
                    Some(agent_env.clone()),
                );

                match tokio::time::timeout(task_timeout, execute_fix).await {
                    Ok(Ok(())) => {
                        self.log(
                            attempt_id,
                            "system",
                            "Deployment auto-fix round completed. Re-running deployment validation...",
                        )
                        .await?;
                    }
                    Ok(Err(e)) => {
                        bail!("Deployment auto-fix round failed: {}", e);
                    }
                    Err(_) => {
                        bail!(
                            "Deployment auto-fix round timed out after {} minutes",
                            project_settings.timeout_mins
                        );
                    }
                }
            } else {
                self.log(
                    attempt_id,
                    "system",
                    "✅ Deployment validation passed. Ready for deployment flow.",
                )
                .await?;
                return Ok(());
            }
        }

        bail!("Deployment validation failed")
    }

    pub(super) async fn maybe_run_agent_driven_deploy_validation(
        &self,
        attempt_id: Uuid,
        task_id: Uuid,
        require_review: bool,
        worktree_path: &Path,
        provider: AgentCliProvider,
        agent_env: &HashMap<String, String>,
    ) -> Result<Option<String>> {
        if require_review {
            return Ok(None);
        }

        let task_snapshot = match self.fetch_task(task_id).await {
            Ok(task) => task,
            Err(err) => {
                self.log(
                    attempt_id,
                    "stderr",
                    &format!(
                        "Warning: failed to fetch task context for deploy validation: {}",
                        err
                    ),
                )
                .await?;
                return Ok(None);
            }
        };

        let project_settings = self
            .fetch_project_settings(task_snapshot.project_id)
            .await
            .unwrap_or_default();
        let auto_deploy_enabled = task_snapshot
            .metadata
            .get("execution")
            .and_then(|v| v.get("auto_deploy"))
            .and_then(|v| v.as_bool())
            .or_else(|| {
                task_snapshot
                    .metadata
                    .get("auto_deploy")
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(project_settings.auto_deploy);

        if !auto_deploy_enabled {
            return Ok(None);
        }

        let project = match self.fetch_project(task_snapshot.project_id).await {
            Ok(project) => project,
            Err(err) => {
                self.log(
                    attempt_id,
                    "stderr",
                    &format!(
                        "Auto-deploy enabled but project context unavailable; skipping deploy validation: {}",
                        err
                    ),
                )
                .await?;
                return Ok(None);
            }
        };

        let deploy_task_timeout =
            Duration::from_secs((project_settings.timeout_mins.max(1) as u64).saturating_mul(60));

        self.log(
            attempt_id,
            "system",
            "Auto-deploy is enabled. Running agent-driven deployment validation...",
        )
        .await?;

        self.log(
            attempt_id,
            "system",
            "Expected output from agent after deployment prep format: PREVIEW_TARGET: <local-preview-url>",
        )
        .await?;

        self.run_deployment_validation_with_auto_fix(
            attempt_id,
            &project,
            worktree_path,
            provider,
            agent_env,
            deploy_task_timeout,
            &project_settings,
        )
        .await?;

        if matches!(project.project_type, ProjectType::Web) {
            let preview_target = self
                .persist_preview_target_from_attempt_logs(attempt_id, Some(worktree_path))
                .await?;
            if let Some(pt) = &preview_target {
                return Ok(Some(pt.clone()));
            }
            // Extract agent-reported reason when PREVIEW_TARGET is missing
            let lines: Vec<String> = self
                .fetch_attempt_log_lines(attempt_id, "deployment failure reason")
                .await
                .unwrap_or_default();
            let agent_reason = super::extract_labeled_value(&lines, "DEPLOYMENT_FAILURE_REASON")
                .or_else(|| super::extract_labeled_value(&lines, "deployment_failure_reason"));
            let msg = if let Some(reason) = &agent_reason {
                format!(
                    "Auto-deploy failed: Agent reported — {}. \
                    (Agent must output DEPLOYMENT_FAILURE_REASON when PREVIEW_TARGET cannot be provided.)",
                    reason.trim()
                )
            } else {
                "Auto-deploy is enabled but agent did not output PREVIEW_TARGET. \
                Deploy preview requires: start the app (e.g. docker compose up -d or npm run dev) \
                and output PREVIEW_TARGET: http://127.0.0.1:<port> or create .acpms/preview-output.json. \
                If you cannot provide PREVIEW_TARGET, you MUST output DEPLOYMENT_FAILURE_REASON: <root cause> \
                (e.g. app failed to start, port conflict, docker compose error, Cloudflare not configured)."
                    .to_string()
            };
            self.log(attempt_id, "stderr", &msg).await?;
            bail!("{}", msg);
        }

        Ok(None)
    }
}
