#[cfg(test)]
mod init_metadata_tests {
    use acpms_db::models::{InitSource, InitTaskMetadata};

    #[test]
    fn test_gitlab_import_metadata_creation() {
        let metadata =
            InitTaskMetadata::gitlab_import("https://gitlab.com/user/repo.git".to_string(), None);

        assert!(metadata.get("init").is_some());
        assert_eq!(metadata["init"]["source"], "gitlab_import");
        assert_eq!(
            metadata["init"]["repository_url"],
            "https://gitlab.com/user/repo.git"
        );
    }

    #[test]
    fn test_from_scratch_metadata_creation() {
        let metadata = InitTaskMetadata::from_scratch("private".to_string(), None, None, None);

        assert!(metadata.get("init").is_some());
        assert_eq!(metadata["init"]["source"], "from_scratch");
        assert_eq!(metadata["init"]["visibility"], "private");
    }

    #[test]
    fn test_metadata_parsing_gitlab_import() {
        let json = serde_json::json!({
            "init": {
                "source": "gitlab_import",
                "repository_url": "https://gitlab.com/user/repo.git"
            }
        });

        let parsed = InitTaskMetadata::parse(&json).unwrap();
        assert_eq!(parsed.source, InitSource::GitlabImport);
        assert_eq!(
            parsed.repository_url.unwrap(),
            "https://gitlab.com/user/repo.git"
        );
        assert!(parsed.visibility.is_none());
    }

    #[test]
    fn test_metadata_parsing_from_scratch() {
        let json = serde_json::json!({
            "init": {
                "source": "from_scratch",
                "visibility": "private"
            }
        });

        let parsed = InitTaskMetadata::parse(&json).unwrap();
        assert_eq!(parsed.source, InitSource::FromScratch);
        assert_eq!(parsed.visibility.unwrap(), "private");
        assert!(parsed.repository_url.is_none());
    }

    #[test]
    fn test_metadata_parsing_missing_init_key() {
        let json = serde_json::json!({
            "other": "data"
        });

        let result = InitTaskMetadata::parse(&json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing 'init' metadata"));
    }

    #[test]
    fn test_metadata_parsing_invalid_source() {
        let json = serde_json::json!({
            "init": {
                "source": "invalid_source"
            }
        });

        let result = InitTaskMetadata::parse(&json);
        assert!(result.is_err());
    }

    #[test]
    fn test_gitlab_import_with_empty_url() {
        let metadata = InitTaskMetadata::gitlab_import("".to_string(), None);

        assert_eq!(metadata["init"]["repository_url"], "");
    }

    #[test]
    fn test_from_scratch_with_public_visibility() {
        let metadata = InitTaskMetadata::from_scratch("public".to_string(), None, None, None);

        assert_eq!(metadata["init"]["visibility"], "public");
    }

    #[test]
    fn test_from_scratch_with_internal_visibility() {
        let metadata = InitTaskMetadata::from_scratch("internal".to_string(), None, None, None);

        assert_eq!(metadata["init"]["visibility"], "internal");
    }

    #[test]
    fn test_metadata_roundtrip_gitlab() {
        let original_url = "https://gitlab.com/test/project.git";
        let metadata = InitTaskMetadata::gitlab_import(original_url.to_string(), None);
        let parsed = InitTaskMetadata::parse(&metadata).unwrap();

        assert_eq!(parsed.source, InitSource::GitlabImport);
        assert_eq!(parsed.repository_url.unwrap(), original_url);
    }

    #[test]
    fn test_metadata_roundtrip_from_scratch() {
        let original_visibility = "private";
        let metadata =
            InitTaskMetadata::from_scratch(original_visibility.to_string(), None, None, None);
        let parsed = InitTaskMetadata::parse(&metadata).unwrap();

        assert_eq!(parsed.source, InitSource::FromScratch);
        assert_eq!(parsed.visibility.unwrap(), original_visibility);
    }
}

#[cfg(test)]
mod task_type_tests {
    use acpms_db::models::TaskType;

    #[test]
    fn test_is_init_for_init_task() {
        let task_type = TaskType::Init;
        assert!(task_type.is_init());
    }

    #[test]
    fn test_is_init_for_feature_task() {
        let task_type = TaskType::Feature;
        assert!(!task_type.is_init());
    }

    #[test]
    fn test_is_init_for_bug_task() {
        let task_type = TaskType::Bug;
        assert!(!task_type.is_init());
    }

    #[test]
    fn test_is_init_for_other_types() {
        assert!(!TaskType::Refactor.is_init());
        assert!(!TaskType::Docs.is_init());
        assert!(!TaskType::Test.is_init());
        assert!(!TaskType::Hotfix.is_init());
        assert!(!TaskType::Chore.is_init());
        assert!(!TaskType::Spike.is_init());
        assert!(!TaskType::SmallTask.is_init());
        assert!(!TaskType::Deploy.is_init());
    }
}

#[cfg(test)]
mod project_type_serde_tests {
    use acpms_db::models::ProjectType;

    #[test]
    fn test_project_type_deserializes_lowercase() {
        let parsed: ProjectType = serde_json::from_str(r#""desktop""#).unwrap();
        assert_eq!(parsed, ProjectType::Desktop);
    }

    #[test]
    fn test_project_type_serializes_lowercase() {
        let value = serde_json::to_string(&ProjectType::Microservice).unwrap();
        assert_eq!(value, r#""microservice""#);
    }
}
