//! PA-203: Instruction builder for Project Assistant context.

use acpms_db::models::{Project, Requirement};
const MAX_REQUIREMENTS: usize = 20;
const MAX_TASKS: usize = 30;
const MAX_HISTORY_MESSAGES: usize = 20;
const MAX_INSTRUCTION_CHARS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct AssistantMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct AttachmentContent {
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct TaskSummary {
    pub title: String,
    pub description: Option<String>,
    pub status: String,
}

/// Language hint line for agent (when preferred language is set).
fn language_instruction_line(preferred_language: Option<&str>) -> Option<&'static str> {
    match normalize_preferred_agent_language(preferred_language) {
        Some("vi") => Some("Always respond in Vietnamese.\n\n"),
        Some("en") => Some("Always respond in English.\n\n"),
        _ => None,
    }
}

/// Normalize preferred language from settings/user input to canonical language code.
/// Supported canonical values: `vi` and `en`.
pub fn normalize_preferred_agent_language(
    preferred_language: Option<&str>,
) -> Option<&'static str> {
    let raw = preferred_language
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())?;
    let lower = raw.to_ascii_lowercase();
    if lower == "vi"
        || lower.starts_with("vi-")
        || lower == "vietnamese"
        || lower == "tiếng việt"
        || lower == "tieng viet"
    {
        return Some("vi");
    }
    if lower == "en" || lower.starts_with("en-") || lower == "english" {
        return Some("en");
    }
    None
}

/// Apply language preference to a follow-up input turn for long-running provider sessions.
/// This helps non-restarted chat sessions consistently follow language preference.
pub fn apply_preferred_language_to_follow_up_input(
    content: &str,
    preferred_language: Option<&str>,
) -> String {
    match normalize_preferred_agent_language(preferred_language) {
        Some("vi") => format!(
            "[Language preference: Reply in Vietnamese unless user explicitly requests another language.]\n\n{}",
            content
        ),
        Some("en") => format!(
            "[Language preference: Reply in English unless user explicitly requests another language.]\n\n{}",
            content
        ),
        _ => content.to_string(),
    }
}

/// Build instruction for Project Assistant CLI.
pub fn build_instruction(
    project: &Project,
    requirements: &[Requirement],
    tasks: &[TaskSummary],
    history: &[AssistantMessage],
    user_message: &str,
    attachments: Option<&[AttachmentContent]>,
    preferred_language: Option<&str>,
) -> String {
    let mut out = String::with_capacity(4096);

    if let Some(line) = language_instruction_line(preferred_language) {
        out.push_str(line);
    }

    out.push_str("You are a Project Assistant. Help the user with questions about this project, requirements, and tasks. You CAN create requirements and tasks: when the user asks to create one, output the JSON line immediately (you may add a brief intro line before it). Do not just explain—propose the tool call.\n\n");

    out.push_str("## Output Rules (CRITICAL)\n");
    out.push_str("- Only output SHORT, concise responses that the user should see in chat.\n");
    out.push_str("- Do NOT output internal steps: command runs, file reads, tool executions, or intermediate reasoning.\n");
    out.push_str("- Do NOT output status lines like \"Preparing...\", \"Confirming...\", \"Summarizing...\", \"Inspecting...\", \"Exploring...\", \"Extending...\", \"Preparing brief greeting\", \"Preparing initial codebase inspection\", etc.\n");
    out.push_str("- Do NOT stream logs, debug output, or verbose progress to the user.\n");
    out.push_str("- Reply like a chat assistant: brief, direct, user-facing answers only.\n\n");

    out.push_str("## Tool Contract (REQUIRED when user asks to create)\n");
    out.push_str("When the user asks to create a requirement or task, you MUST output exactly one JSON line (optionally after a short intro):\n");
    out.push_str("- create_requirement: {\"tool\":\"create_requirement\",\"args\":{\"title\":\"...\",\"content\":\"...\",\"priority\":\"low|medium|high|critical\"}}\n");
    out.push_str("- create_task: {\"tool\":\"create_task\",\"args\":{\"title\":\"...\",\"description\":\"...\",\"task_type\":\"feature|bug|refactor|docs|test|chore|hotfix|spike|small_task\",\"requirement_id\":\"uuid|null\",\"sprint_id\":\"uuid|null\"}}\n");
    out.push_str("task_type must be one of: feature, bug, refactor, docs, test, chore, hotfix, spike, small_task. Default: feature.\n\n");

    // Project context
    out.push_str("## Project\n");
    out.push_str(&format!("- **Name**: {}\n", escape(&project.name)));
    if let Some(ref desc) = project.description {
        let truncated = truncate_str(desc, 2000);
        out.push_str(&format!("- **Description**: {}\n", escape(&truncated)));
    }
    if let Some(ref url) = project.repository_url {
        out.push_str(&format!("- **Repository**: {}\n", escape(url)));
    }
    out.push_str(&format!(
        "- **Branch**: {}\n",
        escape(&project.settings.deploy_branch)
    ));
    out.push_str("\n");

    // Requirements
    out.push_str("## Requirements (recent)\n");
    for req in requirements.iter().take(MAX_REQUIREMENTS) {
        out.push_str(&format!(
            "- [{}] {}: {}\n",
            format!("{:?}", req.status),
            escape(&req.title),
            truncate_str(&req.content, 500)
        ));
    }
    if requirements.is_empty() {
        out.push_str("(none)\n");
    }
    out.push_str("\n");

    // Tasks
    out.push_str("## Tasks (recent)\n");
    for task in tasks.iter().take(MAX_TASKS) {
        out.push_str(&format!(
            "- [{}] {}: {}\n",
            escape(&task.status),
            escape(&task.title),
            truncate_str(task.description.as_deref().unwrap_or(""), 300)
        ));
    }
    if tasks.is_empty() {
        out.push_str("(none)\n");
    }
    out.push_str("\n");

    // Conversation history
    if !history.is_empty() {
        out.push_str("## Conversation History\n");
        for msg in history.iter().rev().take(MAX_HISTORY_MESSAGES).rev() {
            out.push_str(&format!("**{}**: {}\n", msg.role, escape(&msg.content)));
        }
        out.push_str("\n");
    }

    // User message
    out.push_str("## User Message\n");
    out.push_str(&escape(user_message));
    out.push_str("\n");

    // Attachments
    if let Some(atts) = attachments {
        if !atts.is_empty() {
            out.push_str("\n## Attached Files\n");
            for att in atts {
                out.push_str(&format!("### {}\n", escape(&att.filename)));
                out.push_str(&truncate_str(&att.content, 5000));
                out.push_str("\n\n");
            }
        }
    }

    truncate_str(&out, MAX_INSTRUCTION_CHARS).to_string()
}

/// Build minimal instruction for session start (agent greets and confirms ready).
pub fn build_start_instruction(project: &Project, preferred_language: Option<&str>) -> String {
    let mut out = String::with_capacity(2048);
    if let Some(line) = language_instruction_line(preferred_language) {
        out.push_str(line);
    }
    out.push_str("You are a Project Assistant. The user has just started a chat session.\n\n");
    out.push_str("## Task\n");
    out.push_str(
        "Reply with ONE brief greeting (1-2 sentences max) confirming you are ready to help.\n",
    );
    let example = match preferred_language
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some("vi") => "Example: \"Xin chào! Tôi đã sẵn sàng hỗ trợ bạn với dự án này.\"",
        _ => "Example: \"Hello! I'm ready to help with this project.\"",
    };
    out.push_str(example);
    out.push_str("\n");
    out.push_str(
        "Do NOT output any internal steps, commands, or logs—only this short greeting.\n\n",
    );
    out.push_str("## Project\n");
    out.push_str(&format!("- **Name**: {}\n", escape(&project.name)));
    if let Some(ref desc) = project.description {
        out.push_str(&format!(
            "- **Description**: {}\n",
            escape(&truncate_str(desc, 500))
        ));
    }
    out.push_str("\n");
    out
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut cut = max;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}... (truncated)", &s[..cut])
}

#[cfg(test)]
mod tests {
    use super::{apply_preferred_language_to_follow_up_input, normalize_preferred_agent_language};

    #[test]
    fn normalize_preferred_language_accepts_vi_variants() {
        assert_eq!(normalize_preferred_agent_language(Some("vi")), Some("vi"));
        assert_eq!(
            normalize_preferred_agent_language(Some("vi-VN")),
            Some("vi")
        );
        assert_eq!(
            normalize_preferred_agent_language(Some("Vietnamese")),
            Some("vi")
        );
    }

    #[test]
    fn normalize_preferred_language_accepts_en_variants() {
        assert_eq!(normalize_preferred_agent_language(Some("en")), Some("en"));
        assert_eq!(
            normalize_preferred_agent_language(Some("en-US")),
            Some("en")
        );
        assert_eq!(
            normalize_preferred_agent_language(Some("English")),
            Some("en")
        );
    }

    #[test]
    fn follow_up_input_includes_language_pref_for_vi() {
        let wrapped = apply_preferred_language_to_follow_up_input("Xin chao", Some("vi"));
        assert!(wrapped.contains("Reply in Vietnamese"));
        assert!(wrapped.ends_with("Xin chao"));
    }

    #[test]
    fn follow_up_input_keeps_original_when_language_unknown() {
        let original = "hello";
        assert_eq!(
            apply_preferred_language_to_follow_up_input(original, Some("jp")),
            original
        );
    }
}
