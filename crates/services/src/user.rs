use acpms_db::models::{SystemRole, User};
use sqlx::PgPool;
use uuid::Uuid;

pub const OPENCLAW_SERVICE_USER_EMAIL: &str = "openclaw-gateway@acpms.local";

pub fn is_hidden_user_email(email: &str) -> bool {
    email.eq_ignore_ascii_case(OPENCLAW_SERVICE_USER_EMAIL)
}

#[derive(Clone)]
pub struct UserService {
    pool: PgPool,
}

impl UserService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_all_users(&self) -> Result<Vec<User>, sqlx::Error> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT
                id,
                email,
                name,
                avatar_url,
                gitlab_id,
                gitlab_username,
                password_hash,
                global_roles,
                created_at,
                updated_at
            FROM users
            WHERE LOWER(email) <> LOWER($1)
            ORDER BY name ASC
            "#,
        )
        .bind(OPENCLAW_SERVICE_USER_EMAIL)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    pub async fn get_user_by_id(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT
                id,
                email,
                name,
                avatar_url,
                gitlab_id,
                gitlab_username,
                password_hash,
                global_roles,
                created_at,
                updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Get user by email (for admin bootstrap / ensure single admin).
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, sqlx::Error> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT
                id,
                email,
                name,
                avatar_url,
                gitlab_id,
                gitlab_username,
                password_hash,
                global_roles,
                created_at,
                updated_at
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Check if any user has the Admin system role (for bootstrap idempotency).
    pub async fn has_any_admin(&self) -> Result<bool, sqlx::Error> {
        let exists = sqlx::query_scalar::<_, i32>(
            "SELECT 1 FROM users WHERE 'admin'::system_role = ANY(global_roles) LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .is_some();
        Ok(exists)
    }

    pub async fn update_user(
        &self,
        id: Uuid,
        name: Option<String>,
        avatar_url: Option<String>,
        gitlab_username: Option<String>,
        global_roles: Option<Vec<SystemRole>>,
    ) -> Result<Option<User>, sqlx::Error> {
        // We use COALESCE to keep existing values if None is passed,
        // OR we can construct the query dynamically.
        // For simplicity with sqlx constants, we often fetch then update or use COALESCE if we want to support partial updates easily in one query.
        // However, standard COALESCE($2, name) updates to null if we pass explicit null? No, param is Option.
        // If param is None, it binds as NULL. COALESCE(NULL, name) = name. Correct.
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET
                name = COALESCE($2, name),
                avatar_url = COALESCE($3, avatar_url),
                gitlab_username = COALESCE($4, gitlab_username),
                global_roles = COALESCE($5, global_roles),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, name, avatar_url, gitlab_id, gitlab_username, password_hash, global_roles, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(name)
        .bind(avatar_url)
        .bind(gitlab_username)
        .bind(global_roles)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn delete_user(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Change user's password. Returns the user if successful, None if user not found.
    /// Does NOT verify the old password - that should be done by the caller.
    pub async fn change_password(
        &self,
        id: Uuid,
        new_password_hash: String,
    ) -> Result<Option<User>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET password_hash = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, name, avatar_url, gitlab_id, gitlab_username, password_hash, global_roles, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(new_password_hash)
        .fetch_optional(&mut *tx)
        .await?;

        // Invalidate all refresh sessions as part of password rotation.
        if user.is_some() {
            sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        Ok(user)
    }

    /// Get user by ID with password hash for verification
    pub async fn get_user_with_password(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        self.get_user_by_id(id).await
    }

    /// Create a new user. Used by admin invite flow.
    pub async fn create_user(
        &self,
        email: &str,
        name: &str,
        password_hash: &str,
        global_roles: &[SystemRole],
    ) -> Result<User, sqlx::Error> {
        let user_id = Uuid::new_v4();
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, email, name, password_hash, global_roles)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, email, name, avatar_url, gitlab_id, gitlab_username, password_hash, global_roles, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(email)
        .bind(name)
        .bind(password_hash)
        .bind(global_roles)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }
}
