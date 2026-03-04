use super::PatchOperation;
use serde_json::Value;
use uuid::Uuid;

/// Helper for building JSON Patch operations
pub struct PatchBuilder;

impl PatchBuilder {
    /// Add a log entry to the logs array
    pub fn add_log_entry(attempt_id: Uuid, log: &Value) -> PatchOperation {
        PatchOperation::Add {
            path: format!("/attempts/{}/logs/-", attempt_id),
            value: log.clone(),
        }
    }

    /// Update attempt status
    pub fn update_attempt_status(attempt_id: Uuid, status: &str) -> PatchOperation {
        PatchOperation::Replace {
            path: format!("/attempts/{}/status", attempt_id),
            value: Value::String(status.to_string()),
        }
    }

    /// Add normalized entry
    pub fn add_normalized_entry(attempt_id: Uuid, entry: &Value) -> PatchOperation {
        PatchOperation::Add {
            path: format!("/attempts/{}/normalized/-", attempt_id),
            value: entry.clone(),
        }
    }

    /// Replace field in attempt
    pub fn replace_field(attempt_id: Uuid, field: &str, value: Value) -> PatchOperation {
        PatchOperation::Replace {
            path: format!("/attempts/{}/{}", attempt_id, field),
            value,
        }
    }

    /// Remove field from attempt
    pub fn remove_field(attempt_id: Uuid, field: &str) -> PatchOperation {
        PatchOperation::Remove {
            path: format!("/attempts/{}/{}", attempt_id, field),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_add_log_entry() {
        let attempt_id = Uuid::new_v4();
        let log = json!({"message": "test"});

        let patch = PatchBuilder::add_log_entry(attempt_id, &log);

        match patch {
            PatchOperation::Add { path, value } => {
                assert!(path.contains(&attempt_id.to_string()));
                assert!(path.ends_with("/logs/-"));
                assert_eq!(value, log);
            }
            _ => panic!("Expected Add operation"),
        }
    }

    #[test]
    fn test_update_status() {
        let attempt_id = Uuid::new_v4();
        let patch = PatchBuilder::update_attempt_status(attempt_id, "running");

        match patch {
            PatchOperation::Replace { path, value } => {
                assert!(path.contains(&attempt_id.to_string()));
                assert!(path.ends_with("/status"));
                assert_eq!(value, json!("running"));
            }
            _ => panic!("Expected Replace operation"),
        }
    }
}
