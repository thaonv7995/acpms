use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct TokenRequest {
    pub client_id: String,
    pub client_secret: String,
    pub code: String,
    pub grant_type: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub scope: String,
}

#[derive(Debug, Deserialize)]
pub struct GitLabUser {
    pub id: u64,
    pub username: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct GitLabOAuthTokenDb {
    pub id: Uuid,
    pub user_id: Uuid,
    pub gitlab_user_id: i64,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitLabOAuthToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub gitlab_user_id: i64,
    pub gitlab_username: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}
