use acpms_utils::{
    embed_project_document_text, score_project_document_chunk, PROJECT_DOCUMENT_RUNTIME_TOP_K_LIMIT,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::SkillKnowledgeStatus;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProjectVaultSearchMatch {
    pub document_id: Uuid,
    pub document_title: String,
    pub filename: String,
    pub document_kind: String,
    pub chunk_index: i32,
    pub score: f32,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProjectVaultSearchResult {
    pub status: SkillKnowledgeStatus,
    pub detail: Option<String>,
    pub matches: Vec<RuntimeProjectVaultSearchMatch>,
}

#[derive(Debug, FromRow)]
struct ProjectVaultChunkRow {
    document_id: Uuid,
    document_title: String,
    filename: String,
    document_kind: String,
    chunk_index: i32,
    content: String,
    embedding: Vec<f32>,
}

pub async fn search_project_vault(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    query: &str,
    top_k: usize,
) -> Result<RuntimeProjectVaultSearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(RuntimeProjectVaultSearchResult {
            status: SkillKnowledgeStatus::NoMatches,
            detail: Some("Project vault search query was empty.".to_string()),
            matches: Vec::new(),
        });
    }

    let top_k = top_k.clamp(1, PROJECT_DOCUMENT_RUNTIME_TOP_K_LIMIT);
    let rows = sqlx::query_as::<_, ProjectVaultChunkRow>(
        r#"
        SELECT c.document_id,
               d.title AS document_title,
               d.filename,
               d.document_kind,
               c.chunk_index,
               c.content,
               c.embedding
        FROM project_document_chunks c
        INNER JOIN project_documents d
            ON d.id = c.document_id
        WHERE c.project_id = $1
          AND d.ingestion_status = 'indexed'
        ORDER BY d.updated_at DESC, c.chunk_index ASC
        LIMIT 512
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .context("Failed to search project vault chunks")?;

    if rows.is_empty() {
        return Ok(RuntimeProjectVaultSearchResult {
            status: SkillKnowledgeStatus::NoMatches,
            detail: Some(
                "No indexed project documents are available for this project.".to_string(),
            ),
            matches: Vec::new(),
        });
    }

    let query_embedding = embed_project_document_text(query);
    let mut scored = rows
        .into_iter()
        .map(|row| {
            let score =
                score_project_document_chunk(query, &query_embedding, &row.content, &row.embedding);
            RuntimeProjectVaultSearchMatch {
                document_id: row.document_id,
                document_title: row.document_title,
                filename: row.filename,
                document_kind: row.document_kind,
                chunk_index: row.chunk_index,
                score,
                content: row.content,
            }
        })
        .filter(|candidate| candidate.score > 0.05)
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_k);

    if scored.is_empty() {
        return Ok(RuntimeProjectVaultSearchResult {
            status: SkillKnowledgeStatus::NoMatches,
            detail: Some(
                "Indexed project documents exist, but no relevant project-vault matches were found."
                    .to_string(),
            ),
            matches: Vec::new(),
        });
    }

    Ok(RuntimeProjectVaultSearchResult {
        status: SkillKnowledgeStatus::Ready,
        detail: Some(format!("Found {} project vault match(es).", scored.len())),
        matches: scored,
    })
}

pub fn format_project_vault_search_summary(
    query: &str,
    result: &RuntimeProjectVaultSearchResult,
) -> String {
    match result.status {
        SkillKnowledgeStatus::Ready if !result.matches.is_empty() => {
            let items = result
                .matches
                .iter()
                .map(|item| {
                    format!(
                        "{}#{} ({}%)",
                        item.filename,
                        item.chunk_index,
                        (item.score * 100.0).round() as i32
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Project vault search: query=\"{}\" -> [{}]",
                query.trim(),
                items
            )
        }
        _ => format!(
            "Project vault search: query=\"{}\" -> {} ({})",
            query.trim(),
            runtime_status_label(&result.status),
            flatten_runtime_detail(result.detail.as_deref())
        ),
    }
}

pub fn format_project_vault_search_follow_up(
    query: &str,
    result: &RuntimeProjectVaultSearchResult,
) -> String {
    let mut lines = vec![format!(
        "ACPMS project vault search results for query: \"{}\"",
        query.trim()
    )];

    match result.status {
        SkillKnowledgeStatus::Ready if !result.matches.is_empty() => {
            lines.push(
                "Use the following project documents as grounded context. Search again with a narrower query if needed.".to_string(),
            );
            for (idx, item) in result.matches.iter().enumerate() {
                lines.push(format!(
                    "{}. {} [{}] chunk={} relevance={}%",
                    idx + 1,
                    item.document_title,
                    item.filename,
                    item.chunk_index,
                    (item.score * 100.0).round() as i32
                ));
                lines.push(format!("   kind: {}", item.document_kind));
                lines.push(format!("   {}", flatten_snippet(&item.content)));
            }
        }
        _ => {
            lines.push(flatten_runtime_detail(result.detail.as_deref()));
            lines.push(
                "Continue without vault context or retry with a different search query."
                    .to_string(),
            );
        }
    }

    lines.join("\n")
}

fn flatten_runtime_detail(detail: Option<&str>) -> String {
    detail
        .unwrap_or("No detail available.")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn runtime_status_label(status: &SkillKnowledgeStatus) -> &'static str {
    match status {
        SkillKnowledgeStatus::Disabled => "disabled",
        SkillKnowledgeStatus::Pending => "pending",
        SkillKnowledgeStatus::Ready => "ready",
        SkillKnowledgeStatus::Failed => "failed",
        SkillKnowledgeStatus::NoMatches => "no_matches",
    }
}

fn flatten_snippet(content: &str) -> String {
    const MAX_SNIPPET_CHARS: usize = 320;
    let single_line = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() <= MAX_SNIPPET_CHARS {
        single_line
    } else {
        let mut truncated = single_line
            .chars()
            .take(MAX_SNIPPET_CHARS)
            .collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_formats_matches() {
        let result = RuntimeProjectVaultSearchResult {
            status: SkillKnowledgeStatus::Ready,
            detail: Some("Found 1 project vault match.".to_string()),
            matches: vec![RuntimeProjectVaultSearchMatch {
                document_id: Uuid::nil(),
                document_title: "Login Spec".to_string(),
                filename: "login-spec.md".to_string(),
                document_kind: "api_spec".to_string(),
                chunk_index: 0,
                score: 0.82,
                content: "Use email + password.".to_string(),
            }],
        };

        let summary = format_project_vault_search_summary("login", &result);
        assert!(summary.contains("login-spec.md#0"));
    }
}
