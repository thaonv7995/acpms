#[cfg(test)]
mod gitlab_import_task_tests {
    use acpms_db::models::{InitSource, InitTaskMetadata, TaskStatus, TaskType};
    use acpms_services::TaskService;
    use sqlx::PgPool;
    use uuid::Uuid;

    /// Helper to get test database connection from environment
    async fn setup_test_db() -> PgPool {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/acpms_test".to_string()
        });

        PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }

    /// Helper to create a test user
    async fn create_test_user(pool: &PgPool) -> Uuid {
        let user_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, email, name, password_hash, global_roles)
            VALUES ($1, $2, $3, $4, ARRAY['viewer']::system_role[])
            "#,
        )
        .bind(user_id)
        .bind(format!("test-{}@example.com", user_id))
        .bind("Test User")
        .bind("test_hash")
        .execute(pool)
        .await
        .expect("Failed to create test user");

        user_id
    }

    /// Helper to create a test project
    async fn create_test_project(pool: &PgPool, created_by: Uuid) -> Uuid {
        let project_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO projects (id, name, description, created_by, metadata)
            VALUES ($1, $2, $3, $4, '{}'::jsonb)
            "#,
        )
        .bind(project_id)
        .bind("Test Project")
        .bind("Test Description")
        .bind(created_by)
        .execute(pool)
        .await
        .expect("Failed to create test project");

        project_id
    }

    /// Helper to cleanup test data
    async fn cleanup_test_data(pool: &PgPool, user_id: Uuid, project_id: Uuid) {
        let _ = sqlx::query("DELETE FROM tasks WHERE project_id = $1")
            .bind(project_id)
            .execute(pool)
            .await;

        let _ = sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(project_id)
            .execute(pool)
            .await;

        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_create_gitlab_import_task() {
        let pool = setup_test_db().await;
        let service = TaskService::new(pool.clone());

        let user_id = create_test_user(&pool).await;
        let project_id = create_test_project(&pool, user_id).await;
        let repo_url = "https://gitlab.com/test/repo.git";

        let task = service
            .create_gitlab_import_task(project_id, user_id, repo_url, None)
            .await
            .expect("Failed to create GitLab import task");

        // Verify basic task properties
        assert_eq!(task.task_type, TaskType::Init);
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.title, "Initialize Local Repository");
        assert!(task.description.unwrap().contains(repo_url));
        assert_eq!(task.project_id, project_id);
        assert_eq!(task.created_by, user_id);

        // Verify metadata structure
        let metadata = InitTaskMetadata::parse(&task.metadata).expect("Failed to parse metadata");
        assert_eq!(metadata.source, InitSource::GitlabImport);
        assert_eq!(metadata.repository_url.unwrap(), repo_url);
        assert!(metadata.visibility.is_none());

        cleanup_test_data(&pool, user_id, project_id).await;
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_gitlab_import_with_different_urls() {
        let pool = setup_test_db().await;
        let service = TaskService::new(pool.clone());

        let user_id = create_test_user(&pool).await;

        let test_urls = vec![
            "https://gitlab.com/user/repo.git",
            "git@gitlab.com:user/repo.git",
            "https://gitlab.example.com/team/project.git",
        ];

        for url in test_urls {
            let project_id = create_test_project(&pool, user_id).await;

            let task = service
                .create_gitlab_import_task(project_id, user_id, url, None)
                .await
                .expect("Failed to create task");

            let metadata =
                InitTaskMetadata::parse(&task.metadata).expect("Failed to parse metadata");
            assert_eq!(metadata.repository_url.unwrap(), url);

            cleanup_test_data(&pool, user_id, project_id).await;
        }

        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_task_is_init_type() {
        let pool = setup_test_db().await;
        let service = TaskService::new(pool.clone());

        let user_id = create_test_user(&pool).await;
        let project_id = create_test_project(&pool, user_id).await;

        let task = service
            .create_gitlab_import_task(
                project_id,
                user_id,
                "https://gitlab.com/test/repo.git",
                None,
            )
            .await
            .expect("Failed to create task");

        assert!(task.task_type.is_init());

        cleanup_test_data(&pool, user_id, project_id).await;
    }
}
