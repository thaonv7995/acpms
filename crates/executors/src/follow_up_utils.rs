//! Utilities for follow-up user input to avoid full context re-run and token waste.
//!
//! When user sends brief messages like "Hi" or "ok", the agent may re-process the entire
//! attempt context. Wrapping trivial messages with a directive prevents this loop.

/// Trivial follow-up patterns that should not trigger full task re-execution.
const TRIVIAL_PATTERNS: &[&str] = &[
    "hi",
    "hello",
    "hey",
    "ok",
    "okay",
    "thanks",
    "thank you",
    "got it",
    "cool",
    "nice",
    "good",
    "yes",
    "no",
    "sure",
    "alright",
    "done",
    "xong",
    "được",
];

/// Wraps short or trivial follow-up messages with a directive to avoid full context re-run.
/// Prevents token waste when user sends brief acknowledgments like "Hi" or "ok".
pub fn wrap_trivial_follow_up(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return content.to_string();
    }
    let lower = trimmed.to_lowercase();
    let is_trivial = trimmed.len() < 25
        || TRIVIAL_PATTERNS
            .iter()
            .any(|p| lower == *p || lower.starts_with(&format!("{} ", p)));
    if !is_trivial {
        return content.to_string();
    }
    format!(
        r#"[User sent: "{}"]

If this does not require code changes or task updates, respond briefly (e.g. acknowledge). Do not re-analyze or re-execute the full task."#,
        trimmed
    )
}
