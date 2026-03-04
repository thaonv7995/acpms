#[cfg(test)]
mod sanitization_tests {
    use acpms_executors::sanitize_log;

    #[test]
    fn test_sanitize_gitlab_pat() {
        let log = "curl -H 'PRIVATE-TOKEN: glpat-xxxxxxxxxxxxxxxxxxxx' https://gitlab.com/api/v4/projects";
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_multiple_pats() {
        let log = "Token1: glpat-aaaaaaaaaaaaaaaaaaaa and Token2: glpat-bbbbbbbbbbbbbbbbbbbb";
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert_eq!(sanitized.matches("***GITLAB_PAT_REDACTED***").count(), 2);
    }

    #[test]
    fn test_sanitize_preserves_non_sensitive_data() {
        let log = "Successfully cloned repository to /tmp/test";
        let sanitized = sanitize_log(log);

        assert_eq!(sanitized, log);
    }

    #[test]
    fn test_sanitize_pat_in_env_var() {
        let log = "export GITLAB_PAT=glpat-12345678901234567890";
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_pat_in_git_clone_url() {
        let log = "git clone https://oauth2:glpat-xxxxxxxxxxxxxxxxxxxx@gitlab.com/user/repo.git";
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_pat_with_underscores_and_dashes() {
        let log = "TOKEN: glpat-aB3_dEf-GhI_jKl-MnO_pQr";
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_empty_string() {
        let log = "";
        let sanitized = sanitize_log(log);

        assert_eq!(sanitized, "");
    }

    #[test]
    fn test_sanitize_no_tokens() {
        let log = "This is a normal log message without any tokens";
        let sanitized = sanitize_log(log);

        assert_eq!(sanitized, log);
    }

    #[test]
    fn test_sanitize_short_glpat_not_matched() {
        // Pattern requires at least 20 characters after glpat-
        let log = "glpat-short";
        let sanitized = sanitize_log(log);

        // Should not be redacted as it's too short
        assert_eq!(sanitized, log);
    }

    #[test]
    fn test_sanitize_exact_20_chars() {
        let log = "glpat-12345678901234567890"; // Exactly 20 chars after glpat-
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_very_long_token() {
        let log = "TOKEN: glpat-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_multiline_with_tokens() {
        let log = "Line 1: glpat-aaaaaaaaaaaaaaaaaaaa\nLine 2: Some text\nLine 3: glpat-bbbbbbbbbbbbbbbbbbbb";
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert_eq!(sanitized.matches("***GITLAB_PAT_REDACTED***").count(), 2);
        assert!(sanitized.contains("Line 2: Some text"));
    }

    #[test]
    fn test_sanitize_token_at_start() {
        let log = "glpat-xxxxxxxxxxxxxxxxxxxx is the token";
        let sanitized = sanitize_log(log);

        assert!(sanitized.starts_with("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_token_at_end() {
        let log = "The token is glpat-xxxxxxxxxxxxxxxxxxxx";
        let sanitized = sanitize_log(log);

        assert!(sanitized.ends_with("***GITLAB_PAT_REDACTED***"));
    }

    #[test]
    fn test_sanitize_json_with_token() {
        let log = r#"{"private_token": "glpat-xxxxxxxxxxxxxxxxxxxx", "url": "https://gitlab.com"}"#;
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
        assert!(sanitized.contains(r#""url": "https://gitlab.com""#));
    }

    #[test]
    fn test_sanitize_curl_command() {
        let log =
            r#"curl -H "PRIVATE-TOKEN: glpat-yyyyyyyyyyyyyyyyyyyy" https://gitlab.com/api/v4/user"#;
        let sanitized = sanitize_log(log);

        assert!(!sanitized.contains("glpat-"));
        assert!(sanitized.contains("***GITLAB_PAT_REDACTED***"));
        assert!(sanitized.contains("https://gitlab.com/api/v4/user"));
    }

    #[test]
    fn test_sanitize_preserves_structure() {
        let log = "Before glpat-xxxxxxxxxxxxxxxxxxxx after";
        let sanitized = sanitize_log(log);

        assert_eq!(sanitized, "Before ***GITLAB_PAT_REDACTED*** after");
    }
}
