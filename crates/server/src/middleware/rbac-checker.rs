use acpms_db::models::ProjectRole as DbProjectRole;
use acpms_db::PgPool;
use uuid::Uuid;

use super::rbac_types::{Permission, ProjectRole};
use crate::error::ApiError;

/// Map middleware ProjectRole to DB ProjectRole for proper SQL binding
fn to_db_project_role(r: &ProjectRole) -> DbProjectRole {
    match r {
        ProjectRole::Owner => DbProjectRole::Owner,
        ProjectRole::Admin => DbProjectRole::Admin,
        ProjectRole::ProductOwner => DbProjectRole::ProductOwner,
        ProjectRole::Developer => DbProjectRole::Developer,
        ProjectRole::BusinessAnalyst => DbProjectRole::BusinessAnalyst,
        ProjectRole::QualityAssurance => DbProjectRole::QualityAssurance,
        ProjectRole::Viewer => DbProjectRole::Viewer,
    }
}

/// RBAC permission checker
pub struct RbacChecker;

impl RbacChecker {
    /// Check whether a user has system admin role
    pub async fn is_system_admin(user_id: Uuid, pool: &PgPool) -> Result<bool, ApiError> {
        let is_admin: Option<bool> = sqlx::query_scalar(
            r#"
            SELECT 'admin'::system_role = ANY(global_roles)
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(is_admin.unwrap_or(false))
    }

    /// Require system admin role
    pub async fn check_system_admin(user_id: Uuid, pool: &PgPool) -> Result<(), ApiError> {
        if Self::is_system_admin(user_id, pool).await? {
            Ok(())
        } else {
            Err(ApiError::Forbidden("Admin access required".to_string()))
        }
    }

    /// Check if user has required permission for a project
    pub async fn check_permission(
        user_id: Uuid,
        project_id: Uuid,
        permission: Permission,
        pool: &PgPool,
    ) -> Result<(), ApiError> {
        // Check if user is system admin (bypass project membership)
        let is_admin = Self::is_system_admin(user_id, pool).await?;

        // System admins have full access to all projects
        if is_admin {
            return Ok(());
        }

        // First check if user is a member of the project
        let is_member: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM project_members
                WHERE user_id = $1 AND project_id = $2
            )
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_one(pool)
        .await?;

        if !is_member {
            // Return 404 instead of 403 to hide project existence from non-members
            return Err(ApiError::NotFound("Project not found".to_string()));
        }

        // Check if user has any of the required roles (use DbProjectRole for proper PostgreSQL enum array binding)
        let required_roles: Vec<DbProjectRole> = permission
            .required_roles()
            .iter()
            .map(to_db_project_role)
            .collect();

        let has_permission: bool = sqlx::query_scalar(
            r#"
            SELECT user_has_any_role($1, $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .bind(&required_roles)
        .fetch_one(pool)
        .await?;

        if !has_permission {
            return Err(ApiError::Forbidden(
                "Insufficient permissions for this project".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if user is project owner (highest privilege)
    #[allow(dead_code)]
    pub async fn check_owner(
        user_id: Uuid,
        project_id: Uuid,
        pool: &PgPool,
    ) -> Result<(), ApiError> {
        let is_owner: bool = sqlx::query_scalar(
            r#"
            SELECT user_has_role($1, $2, 'owner'::project_role)
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_one(pool)
        .await?;

        if !is_owner {
            return Err(ApiError::Forbidden(
                "Only project owner can perform this action".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if user can modify a specific task
    /// Returns the project_id if user has permission
    #[allow(dead_code)]
    pub async fn check_task_permission(
        user_id: Uuid,
        task_id: Uuid,
        pool: &PgPool,
    ) -> Result<Uuid, ApiError> {
        #[derive(sqlx::FromRow)]
        struct TaskPermissionResult {
            project_id: Uuid,
            can_modify: Option<bool>,
        }

        // Get project_id and check permission in one query
        let result = sqlx::query_as::<_, TaskPermissionResult>(
            r#"
            SELECT t.project_id, user_can_modify_task($1, $2) as can_modify
            FROM tasks t
            WHERE t.id = $2
            "#,
        )
        .bind(user_id)
        .bind(task_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

        if !result.can_modify.unwrap_or(false) {
            return Err(ApiError::Forbidden(
                "You don't have permission to modify this task".to_string(),
            ));
        }

        Ok(result.project_id)
    }
}
