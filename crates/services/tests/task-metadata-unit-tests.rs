#[cfg(test)]
mod task_metadata_unit_tests {
    use acpms_db::models::{InitSource, InitTaskMetadata};

    #[test]
    fn test_metadata_serialization_gitlab() {
        let metadata =
            InitTaskMetadata::gitlab_import("https://gitlab.com/test/repo.git".to_string(), None);

        // Should be valid JSON
        let json_str = serde_json::to_string(&metadata).expect("Failed to serialize");
        assert!(json_str.contains("gitlab_import"));
        assert!(json_str.contains("https://gitlab.com/test/repo.git"));
    }

    #[test]
    fn test_metadata_serialization_from_scratch() {
        let metadata = InitTaskMetadata::from_scratch("private".to_string(), None, None, None);

        // Should be valid JSON
        let json_str = serde_json::to_string(&metadata).expect("Failed to serialize");
        assert!(json_str.contains("from_scratch"));
        assert!(json_str.contains("private"));
    }

    #[test]
    fn test_init_source_variants() {
        let gitlab_source = InitSource::GitlabImport;
        let scratch_source = InitSource::FromScratch;

        assert_ne!(gitlab_source, scratch_source);
    }

    #[test]
    fn test_init_source_clone() {
        let source = InitSource::GitlabImport;
        let cloned = source.clone();
        assert_eq!(source, cloned);
    }
}
