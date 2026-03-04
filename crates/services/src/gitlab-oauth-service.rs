use crate::encryption_service::EncryptionService;
use crate::gitlab_oauth_types::*;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// GitLab OAuth2 service for handling authorization flow
#[derive(Clone)]
pub struct GitLabOAuthService {
    db: PgPool,
    encryption: EncryptionService,
    client: Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    gitlab_base_url: String,
}

impl GitLabOAuthService {
    /// Create new GitLabOAuthService from environment variables.
    /// If GITLAB_CLIENT_ID / GITLAB_CLIENT_SECRET / GITLAB_REDIRECT_URI are not set,
    /// service starts in "not configured" mode; OAuth flows will return a clear error.
    ///
    /// - GITLAB_CLIENT_ID, GITLAB_CLIENT_SECRET, GITLAB_REDIRECT_URI (optional; set for GitLab integration)
    /// - GITLAB_BASE_URL (optional, defaults to https://gitlab.com)
    /// - ENCRYPTION_KEY (required for token encryption when OAuth is used)
    pub fn from_env(db: PgPool) -> Result<Self> {
        let encryption = EncryptionService::from_env()
            .context("Failed to initialize encryption for OAuth tokens")?;

        let client_id = std::env::var("GITLAB_CLIENT_ID").unwrap_or_default();
        let client_secret = std::env::var("GITLAB_CLIENT_SECRET").unwrap_or_default();
        let redirect_uri = std::env::var("GITLAB_REDIRECT_URI").unwrap_or_default();
        let gitlab_base_url =
            std::env::var("GITLAB_BASE_URL").unwrap_or_else(|_| "https://gitlab.com".to_string());

        Ok(Self {
            db,
            encryption,
            client: Client::new(),
            client_id,
            client_secret,
            redirect_uri,
            gitlab_base_url,
        })
    }

    /// True if GitLab OAuth is configured (client_id set).
    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
    }

    /// Generate OAuth authorization URL. Returns empty string if OAuth not configured.
    pub fn get_authorization_url(&self, state: &str, scope: Option<&str>) -> String {
        if !self.is_configured() {
            return String::new();
        }
        let scope_str = scope.unwrap_or("api read_user");
        format!(
            "{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&scope={}",
            self.gitlab_base_url,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(state),
            urlencoding::encode(scope_str)
        )
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code(
        &self,
        code: &str,
        user_id: Uuid,
        project_id: Option<Uuid>,
    ) -> Result<GitLabOAuthToken> {
        if !self.is_configured() {
            bail!("GitLab OAuth is not configured. Set GITLAB_CLIENT_ID, GITLAB_CLIENT_SECRET, GITLAB_REDIRECT_URI in environment or Settings.");
        }
        // Exchange code for token
        let token_response = self.request_token(code).await?;

        // Get GitLab user info
        let gitlab_user = self.get_gitlab_user(&token_response.access_token).await?;

        // Encrypt and store
        self.store_token(user_id, project_id, &token_response, &gitlab_user)
            .await
    }

    /// Store encrypted OAuth token in database
    async fn store_token(
        &self,
        user_id: Uuid,
        project_id: Option<Uuid>,
        token_resp: &TokenResponse,
        gitlab_user: &GitLabUser,
    ) -> Result<GitLabOAuthToken> {
        let access_token_encrypted = self.encryption.encrypt(&token_resp.access_token)?;
        let refresh_token_encrypted = token_resp
            .refresh_token
            .as_ref()
            .map(|t| self.encryption.encrypt(t))
            .transpose()?;

        let expires_at = token_resp
            .expires_in
            .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs));

        let token = sqlx::query_as::<_, GitLabOAuthTokenDb>(
            r#"
            INSERT INTO gitlab_oauth_tokens
            (user_id, project_id, access_token_encrypted, refresh_token_encrypted,
             token_type, expires_at, scope, gitlab_user_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (user_id, project_id)
            DO UPDATE SET
                access_token_encrypted = EXCLUDED.access_token_encrypted,
                refresh_token_encrypted = EXCLUDED.refresh_token_encrypted,
                expires_at = EXCLUDED.expires_at,
                updated_at = NOW()
            RETURNING id, user_id, gitlab_user_id, expires_at
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .bind(access_token_encrypted.as_str())
        .bind(refresh_token_encrypted.as_deref())
        .bind(&token_resp.token_type)
        .bind(expires_at)
        .bind(token_resp.scope.as_str())
        .bind(gitlab_user.id as i64)
        .fetch_one(&self.db)
        .await
        .context("Failed to store OAuth token")?;

        Ok(GitLabOAuthToken {
            id: token.id,
            user_id: token.user_id,
            gitlab_user_id: token.gitlab_user_id,
            gitlab_username: gitlab_user.username.clone(),
            expires_at: token.expires_at,
        })
    }

    /// Request access token from GitLab
    async fn request_token(&self, code: &str) -> Result<TokenResponse> {
        let url = format!("{}/oauth/token", self.gitlab_base_url);

        let params = TokenRequest {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            code: code.to_string(),
            grant_type: "authorization_code".to_string(),
            redirect_uri: self.redirect_uri.clone(),
        };

        let resp = self
            .client
            .post(&url)
            .json(&params)
            .send()
            .await
            .context("Failed to request OAuth token")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Token exchange failed ({}): {}", status, body);
        }

        resp.json().await.context("Failed to parse token response")
    }

    /// Get GitLab user information
    async fn get_gitlab_user(&self, access_token: &str) -> Result<GitLabUser> {
        let url = format!("{}/api/v4/user", self.gitlab_base_url);

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("Failed to get GitLab user")?;

        if !resp.status().is_success() {
            bail!("Failed to get GitLab user: {}", resp.status());
        }

        resp.json()
            .await
            .context("Failed to parse GitLab user response")
    }

    /// Get decrypted access token for a user
    pub async fn get_access_token(
        &self,
        user_id: Uuid,
        project_id: Option<Uuid>,
    ) -> Result<String> {
        #[derive(FromRow)]
        struct TokenData {
            access_token_encrypted: String,
        }

        let token = sqlx::query_as::<_, TokenData>(
            "SELECT access_token_encrypted FROM gitlab_oauth_tokens WHERE user_id = $1 AND project_id IS NOT DISTINCT FROM $2"
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&self.db)
        .await
        .context("Failed to fetch OAuth token")?
        .context("No OAuth token found for user")?;

        self.encryption
            .decrypt(&token.access_token_encrypted)
            .context("Failed to decrypt access token")
    }
}
