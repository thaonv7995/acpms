use std::sync::Arc;

use acpms_db::{models::*, PgPool};
use acpms_executors::{publish_docs_task_to_vault, DiffStorageUploader};

pub const TASK_DOCUMENT_KINDS: &[&str] = &[
    "brainstorm",
    "idea_note",
    "prd",
    "srs",
    "design",
    "technical_spec",
    "research_note",
    "meeting_note",
    "architecture",
    "api_spec",
    "database_schema",
    "business_rules",
    "runbook",
    "notes",
    "other",
];

pub const TASK_DOCUMENT_FORMATS: &[&str] = &["markdown", "pdf", "image", "figma_link", "binary"];

pub fn task_has_vault_document(task: &Task) -> bool {
    task.metadata
        .get("document")
        .and_then(|value| value.get("vault_document_id"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

pub fn normalize_docs_task_metadata(
    task_type: TaskType,
    task_title: &str,
    metadata: serde_json::Value,
) -> serde_json::Value {
    let mut metadata = match metadata {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };

    if task_type != TaskType::Docs {
        return serde_json::Value::Object(metadata);
    }

    let mut execution = metadata
        .remove("execution")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    execution.insert("no_code_changes".into(), serde_json::Value::Bool(true));
    execution.insert("run_build_and_tests".into(), serde_json::Value::Bool(false));
    execution.insert("auto_deploy".into(), serde_json::Value::Bool(false));
    metadata.insert("execution".into(), serde_json::Value::Object(execution));

    let mut document = metadata
        .remove("document")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();

    let existing_kind = document
        .get("kind")
        .and_then(|value| value.as_str())
        .filter(|value| TASK_DOCUMENT_KINDS.contains(value))
        .unwrap_or("other");
    document.insert(
        "kind".into(),
        serde_json::Value::String(existing_kind.to_string()),
    );

    let inferred_format = if document
        .get("figma_url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        "figma_link"
    } else {
        "markdown"
    };
    let existing_format = document
        .get("format")
        .and_then(|value| value.as_str())
        .filter(|value| TASK_DOCUMENT_FORMATS.contains(value))
        .unwrap_or(inferred_format);
    document.insert(
        "format".into(),
        serde_json::Value::String(existing_format.to_string()),
    );
    document.insert(
        "preview_mode".into(),
        serde_json::Value::String("document".to_string()),
    );
    document.insert(
        "publish_policy".into(),
        serde_json::Value::String("final_on_done".to_string()),
    );

    let title = document
        .get("title")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(task_title);
    document.insert("title".into(), serde_json::Value::String(title.to_string()));

    metadata.insert("document".into(), serde_json::Value::Object(document));
    serde_json::Value::Object(metadata)
}

pub struct TaskDocumentWorkflowService {
    pool: PgPool,
    storage: Arc<dyn DiffStorageUploader>,
}

impl TaskDocumentWorkflowService {
    pub fn new(pool: PgPool, storage: Arc<dyn DiffStorageUploader>) -> Self {
        Self { pool, storage }
    }

    pub async fn publish_final_document(
        &self,
        task_id: uuid::Uuid,
    ) -> anyhow::Result<Option<ProjectDocument>> {
        publish_docs_task_to_vault(&self.pool, &self.storage, task_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_has_vault_document_detects_published_docs() {
        let task = Task {
            id: uuid::Uuid::nil(),
            project_id: uuid::Uuid::nil(),
            title: "Doc".to_string(),
            description: None,
            task_type: TaskType::Docs,
            status: TaskStatus::Done,
            assigned_to: None,
            parent_task_id: None,
            requirement_id: None,
            sprint_id: None,
            gitlab_issue_id: None,
            metadata: serde_json::json!({
                "document": {
                    "vault_document_id": "abc123"
                }
            }),
            created_by: uuid::Uuid::nil(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(task_has_vault_document(&task));
    }

    #[test]
    fn docs_metadata_is_normalized_with_document_defaults() {
        let metadata = normalize_docs_task_metadata(
            TaskType::Docs,
            "Product Requirements",
            serde_json::json!({
                "priority": "high",
                "document": {
                    "kind": "prd"
                }
            }),
        );

        assert_eq!(metadata["document"]["kind"], "prd");
        assert_eq!(metadata["document"]["format"], "markdown");
        assert_eq!(metadata["document"]["preview_mode"], "document");
        assert_eq!(metadata["document"]["publish_policy"], "final_on_done");
        assert_eq!(metadata["document"]["title"], "Product Requirements");
        assert_eq!(metadata["execution"]["no_code_changes"], true);
        assert_eq!(metadata["execution"]["run_build_and_tests"], false);
        assert_eq!(metadata["execution"]["auto_deploy"], false);
    }

    #[test]
    fn docs_metadata_prefers_figma_link_when_figma_url_is_present() {
        let metadata = normalize_docs_task_metadata(
            TaskType::Docs,
            "Checkout Design",
            serde_json::json!({
                "document": {
                    "kind": "design",
                    "figma_url": "https://www.figma.com/design/demo"
                }
            }),
        );

        assert_eq!(metadata["document"]["format"], "figma_link");
        assert_eq!(metadata["document"]["kind"], "design");
    }

    #[test]
    fn non_docs_metadata_is_left_unchanged() {
        let original = serde_json::json!({
            "execution": {
                "run_build_and_tests": true
            }
        });

        let normalized =
            normalize_docs_task_metadata(TaskType::Feature, "Feature", original.clone());

        assert_eq!(normalized, original);
    }
}
