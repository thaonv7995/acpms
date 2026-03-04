use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::debug;

/// Represents a Claude Code project directory
#[derive(Debug, Clone)]
pub struct ClaudeProject {
    pub path: PathBuf,
    pub name: String,
    pub git_branch: Option<String>,
    pub first_prompt: Option<String>,
    pub session_count: usize,
    pub modified_at: SystemTime,
}

/// Represents a single session file within a project
#[derive(Debug, Clone)]
pub struct ClaudeSession {
    pub path: PathBuf,
    pub git_branch: Option<String>,
    pub first_prompt: Option<String>,
    pub modified_at: SystemTime,
}

/// A JSONL record for metadata extraction
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonlRecord {
    git_branch: Option<String>,
    message: Option<JsonlMessage>,
}

/// Message within a JSONL record
#[derive(Debug, Deserialize)]
struct JsonlMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

/// Manages Claude Code CLI session discovery and reading
#[derive(Clone)]
pub struct ClaudeSessionManager {
    claude_dir: PathBuf,
}

impl ClaudeSessionManager {
    /// Create a new session manager
    /// Reads from CLAUDE_SESSION_DIR env var or defaults to ~/.claude
    pub fn new() -> Result<Self> {
        let claude_dir = if let Ok(dir) = std::env::var("CLAUDE_SESSION_DIR") {
            PathBuf::from(dir)
        } else {
            dirs::home_dir()
                .context("Could not find home directory")?
                .join(".claude")
        };

        Ok(Self { claude_dir })
    }

    /// Get the Claude projects directory path
    fn get_projects_dir(&self) -> PathBuf {
        self.claude_dir.join("projects")
    }

    /// Discover all Claude projects, sorted by modification time (most recent first)
    /// Aggregates session metadata (git_branch, first_prompt, session_count) from each project's sessions
    pub fn discover_projects(&self) -> Result<Vec<ClaudeProject>> {
        let projects_dir = self.get_projects_dir();

        if !projects_dir.exists() {
            debug!(
                "Claude projects directory does not exist: {:?}",
                projects_dir
            );
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();

        let entries = fs::read_dir(&projects_dir)
            .with_context(|| format!("Failed to read directory: {:?}", projects_dir))?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let metadata = entry.metadata().context("Failed to read entry metadata")?;
            let modified_at = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

            // Extract a friendly name from the directory name
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let name = extract_project_name(dir_name);

            // Discover sessions to get aggregated metadata
            let sessions = discover_sessions_in_dir(&path)?;
            let session_count = sessions.len();

            // Skip projects with no sessions
            if session_count == 0 {
                continue;
            }

            // Get metadata from the most recent session
            let most_recent = &sessions[0]; // Already sorted by modification time
            let git_branch = most_recent.git_branch.clone();
            let first_prompt = most_recent.first_prompt.clone();

            projects.push(ClaudeProject {
                path,
                name,
                git_branch,
                first_prompt,
                session_count,
                modified_at,
            });
        }

        // Sort by modification time, most recent first
        projects.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

        Ok(projects)
    }

    /// Fast check if any Claude projects exist without parsing session JSONL files.
    /// Used for rapid verification of Claude Code login status.
    pub fn has_any_project(&self) -> Result<bool> {
        let projects_dir = self.get_projects_dir();

        if !projects_dir.exists() {
            return Ok(false);
        }

        let entries = fs::read_dir(&projects_dir)
            .with_context(|| format!("Failed to read directory: {:?}", projects_dir))?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            if entry.path().is_dir() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Find projects matching a specific git branch using fuzzy matching
    pub fn find_projects_by_branch(&self, target_branch: &str) -> Result<Vec<ClaudeProject>> {
        let projects = self.discover_projects()?;
        let mut matches = Vec::new();

        for project in projects {
            // Check if project's branch matches
            if let Some(ref project_branch) = project.git_branch {
                if branches_match(target_branch, project_branch) {
                    matches.push(project);
                }
            }
        }

        Ok(matches)
    }

    /// Discover sessions for a specific project
    pub fn discover_sessions(&self, project: &ClaudeProject) -> Result<Vec<ClaudeSession>> {
        discover_sessions_in_dir(&project.path)
    }

    /// Read session JSONL content as a single string
    pub fn read_session(&self, session_path: &Path) -> Result<String> {
        fs::read_to_string(session_path)
            .with_context(|| format!("Failed to read session file: {:?}", session_path))
    }
}

/// Extract a friendly project name from the Claude directory name
fn extract_project_name(dir_name: &str) -> String {
    // Directory names look like:
    // "-private-var-folders-m1-9q-ct1913z10v6wbnv54j25r0000gn-T-vibe-kanban-worktrees-a04a-store-payloads-i"
    // We want to extract the meaningful part after "worktrees-"
    if let Some(idx) = dir_name.find("worktrees-") {
        let after_worktrees = &dir_name[idx + "worktrees-".len()..];
        // Skip the short hash prefix (e.g., "a04a-")
        if let Some(dash_idx) = after_worktrees.find('-') {
            return after_worktrees[dash_idx + 1..].to_string();
        }
        return after_worktrees.to_string();
    }

    // Fallback: use last segment after the final dash
    dir_name.rsplit('-').next().unwrap_or(dir_name).to_string()
}

/// Discover sessions in a directory, excluding agent-* files
fn discover_sessions_in_dir(dir_path: &Path) -> Result<Vec<ClaudeSession>> {
    let mut sessions = Vec::new();

    let entries = fs::read_dir(dir_path)
        .with_context(|| format!("Failed to read directory: {:?}", dir_path))?;

    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        // Only process .jsonl files
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip agent-* files
        if file_name.starts_with("agent-") {
            continue;
        }

        let metadata = entry.metadata().context("Failed to read entry metadata")?;
        let modified_at = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        // Extract metadata from the JSONL file
        let (git_branch, first_prompt) = extract_session_metadata(&path);

        sessions.push(ClaudeSession {
            path,
            git_branch,
            first_prompt,
            modified_at,
        });
    }

    // Sort by modification time, most recent first
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(sessions)
}

/// Extract session metadata from a JSONL file
/// Returns: (git_branch, first_prompt)
fn extract_session_metadata(path: &Path) -> (Option<String>, Option<String>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return (None, None),
    };
    let reader = BufReader::new(file);

    let mut git_branch: Option<String> = None;
    let mut first_prompt: Option<String> = None;

    // Check first 50 lines for metadata
    for line in reader.lines().take(50) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(record) = serde_json::from_str::<JsonlRecord>(&line) {
            // Extract git branch if not already found
            if git_branch.is_none() && record.git_branch.is_some() {
                git_branch = record.git_branch;
            }

            // Extract first user prompt if not already found
            if first_prompt.is_none() {
                if let Some(ref message) = record.message {
                    if message.role.as_deref() == Some("user") {
                        if let Some(ref content) = message.content {
                            // Content can be a string or an array
                            if let Some(text) = content.as_str() {
                                first_prompt = Some(truncate_string(text, 60));
                            }
                        }
                    }
                }
            }

            // Stop early if we have both
            if git_branch.is_some() && first_prompt.is_some() {
                break;
            }
        }
    }

    (git_branch, first_prompt)
}

/// Truncate a string to max length, adding "..." if truncated
fn truncate_string(s: &str, max_len: usize) -> String {
    // Replace newlines with spaces for display
    let s = s.replace('\n', " ");
    // Use char_indices for safe UTF-8 truncation
    let char_count = s.chars().count();
    if char_count <= max_len {
        s
    } else {
        // Find the byte position of the (max_len - 3)th character
        let truncate_at = max_len.saturating_sub(3);
        let byte_pos = s
            .char_indices()
            .nth(truncate_at)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        format!("{}...", &s[..byte_pos])
    }
}

/// Check if two branch names match using fuzzy matching
fn branches_match(target: &str, session_branch: &str) -> bool {
    let target_normalized = normalize_branch(target);
    let session_normalized = normalize_branch(session_branch);

    // Exact match after normalization
    if target_normalized == session_normalized {
        return true;
    }

    // Check if the slug portions match (e.g., "feature-auth" matches "vk/feature-auth")
    let target_slug = extract_branch_slug(&target_normalized);
    let session_slug = extract_branch_slug(&session_normalized);

    target_slug == session_slug && !target_slug.is_empty()
}

/// Normalize a branch name by stripping common prefixes
fn normalize_branch(branch: &str) -> String {
    let branch = branch.strip_prefix("refs/heads/").unwrap_or(branch);
    branch.to_lowercase()
}

/// Extract the "slug" portion of a branch name
/// e.g., "vk/a04a-store-payloads-i" -> "a04a-store-payloads-i"
fn extract_branch_slug(branch: &str) -> String {
    // Split by '/' and take the last part
    branch.rsplit('/').next().unwrap_or(branch).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_project_name() {
        assert_eq!(
            extract_project_name(
                "-private-var-folders-m1-9q-ct1913z10v6wbnv54j25r0000gn-T-vibe-kanban-worktrees-a04a-store-payloads-i"
            ),
            "store-payloads-i"
        );

        assert_eq!(
            extract_project_name(
                "-private-var-folders-m1-9q-ct1913z10v6wbnv54j25r0000gn-T-vibe-kanban-worktrees-1ff1-new-rust-binary"
            ),
            "new-rust-binary"
        );
    }

    #[test]
    fn test_branches_match() {
        // Exact match
        assert!(branches_match("feature-auth", "feature-auth"));

        // With prefix
        assert!(branches_match("feature-auth", "vk/feature-auth"));
        assert!(branches_match("vk/feature-auth", "feature-auth"));

        // Slug matching
        assert!(branches_match(
            "a04a-store-payloads-i",
            "vk/a04a-store-payloads-i"
        ));

        // Case insensitive
        assert!(branches_match("Feature-Auth", "feature-auth"));

        // Non-matches
        assert!(!branches_match("feature-auth", "feature-other"));
        assert!(!branches_match("main", "feature-auth"));

        // Regression tests: substring matches should NOT match
        assert!(!branches_match("vk/d13f-remove-compare-c", "c"));
        assert!(!branches_match("vk/d13f-remove-compare-c", "compare"));
        assert!(!branches_match("feature-auth", "auth"));
        assert!(!branches_match("feature-auth", "feature"));
    }

    #[test]
    fn test_normalize_branch() {
        assert_eq!(normalize_branch("refs/heads/main"), "main");
        assert_eq!(normalize_branch("Feature-Auth"), "feature-auth");
        assert_eq!(normalize_branch("vk/feature-auth"), "vk/feature-auth");
    }

    #[test]
    fn test_extract_branch_slug() {
        assert_eq!(extract_branch_slug("vk/feature-auth"), "feature-auth");
        assert_eq!(extract_branch_slug("feature-auth"), "feature-auth");
        assert_eq!(
            extract_branch_slug("user/prefix/feature-auth"),
            "feature-auth"
        );
    }
}
