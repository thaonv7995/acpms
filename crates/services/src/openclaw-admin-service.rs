use chrono::{DateTime, Duration, Utc};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum OpenClawAdminServiceError {
    #[error("OpenClaw client '{0}' was not found")]
    ClientNotFound(String),

    #[error("OpenClaw client '{0}' has been revoked and cannot be modified")]
    RevokedClient(String),

    #[error("OpenClaw bootstrap token is invalid or unavailable")]
    InvalidBootstrapToken,

    #[error("Failed to {0}: {1}")]
    Internal(&'static str, String),
}

type ServiceResult<T> = std::result::Result<T, OpenClawAdminServiceError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawAdminClientSummary {
    pub client_id: String,
    pub display_name: String,
    pub status: String,
    pub kind: String,
    pub enrolled_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_seen_ip: Option<String>,
    pub last_seen_user_agent: Option<String>,
    pub key_fingerprints: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CreateOpenClawBootstrapTokenInput {
    pub label: String,
    pub expires_in_minutes: i64,
    pub suggested_display_name: Option<String>,
    pub metadata: Option<Value>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawBootstrapPrompt {
    pub bootstrap_token_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub prompt_text: String,
    pub token_preview: String,
}

#[derive(Debug, Clone)]
pub struct ConsumeOpenClawBootstrapTokenInput {
    pub raw_token: String,
    pub display_name: Option<String>,
    pub key_id: String,
    pub algorithm: Option<String>,
    pub public_key: String,
    pub metadata: Option<Value>,
    pub last_seen_ip: Option<String>,
    pub last_seen_user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawEnrollmentResult {
    pub client_id: String,
    pub display_name: String,
    pub key_id: String,
    pub algorithm: String,
    pub key_fingerprint: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct OpenClawAdminService {
    pool: PgPool,
}

#[derive(Debug, FromRow)]
struct OpenClawClientRow {
    id: Uuid,
    client_id: String,
    display_name: String,
    status: String,
    enrolled_at: DateTime<Utc>,
    last_seen_at: Option<DateTime<Utc>>,
    last_seen_ip: Option<String>,
    last_seen_user_agent: Option<String>,
}

#[derive(Debug, FromRow)]
struct OpenClawKeyFingerprintRow {
    client_ref_id: Uuid,
    fingerprint: String,
}

#[derive(Debug, FromRow)]
struct OpenClawBootstrapTokenRow {
    id: Uuid,
    label: String,
    suggested_display_name: Option<String>,
    status: String,
    expires_at: DateTime<Utc>,
    metadata: Value,
}

#[derive(Debug, FromRow)]
struct OpenClawPendingTokenRow {
    id: Uuid,
    label: String,
    suggested_display_name: Option<String>,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl OpenClawAdminService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_clients(&self) -> ServiceResult<Vec<OpenClawAdminClientSummary>> {
        self.expire_stale_bootstrap_tokens().await?;

        let rows = sqlx::query_as::<_, OpenClawClientRow>(
            r#"
            SELECT
                id,
                client_id,
                display_name,
                status,
                enrolled_at,
                last_seen_at,
                host(last_seen_ip)::text AS last_seen_ip,
                last_seen_user_agent
            FROM openclaw_clients
            ORDER BY enrolled_at DESC, created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| Self::internal("load OpenClaw clients", error))?;

        let pending_rows = sqlx::query_as::<_, OpenClawPendingTokenRow>(
            r#"
            SELECT
                id,
                label,
                suggested_display_name,
                expires_at,
                created_at
            FROM openclaw_bootstrap_tokens
            WHERE status = 'active'
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| Self::internal("load pending OpenClaw bootstrap tokens", error))?;

        let mut summaries = self.build_client_summaries(rows).await?;
        summaries.extend(pending_rows.into_iter().map(Self::build_pending_summary));
        summaries.sort_by(|left, right| right.enrolled_at.cmp(&left.enrolled_at));
        Ok(summaries)
    }

    pub async fn create_bootstrap_token(
        &self,
        input: CreateOpenClawBootstrapTokenInput,
        base_url: &str,
        runtime_api_key: Option<&str>,
        runtime_webhook_secret: Option<&str>,
    ) -> ServiceResult<OpenClawBootstrapPrompt> {
        self.expire_stale_bootstrap_tokens().await?;

        let expires_in_minutes = input.expires_in_minutes.clamp(1, 24 * 60);
        let expires_at = Utc::now() + Duration::minutes(expires_in_minutes);
        let raw_token = generate_bootstrap_token();
        let token_hash = hash_token(&raw_token);
        let metadata = input.metadata.unwrap_or_else(|| json!({}));

        let bootstrap_token_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO openclaw_bootstrap_tokens (
                token_hash,
                label,
                suggested_display_name,
                expires_at,
                created_by,
                metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(token_hash)
        .bind(input.label.trim())
        .bind(
            input
                .suggested_display_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(expires_at)
        .bind(input.created_by)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| Self::internal("create OpenClaw bootstrap token", error))?;

        Ok(OpenClawBootstrapPrompt {
            bootstrap_token_id,
            expires_at,
            prompt_text: build_bootstrap_prompt(
                base_url,
                &raw_token,
                input.label.trim(),
                input
                    .suggested_display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
                expires_at,
                runtime_api_key,
                runtime_webhook_secret,
            ),
            token_preview: preview_token(&raw_token),
        })
    }

    pub async fn disable_client(
        &self,
        client_external_id: &str,
    ) -> ServiceResult<OpenClawAdminClientSummary> {
        self.update_client_status(client_external_id, "disabled")
            .await
    }

    pub async fn consume_bootstrap_token(
        &self,
        input: ConsumeOpenClawBootstrapTokenInput,
    ) -> ServiceResult<OpenClawEnrollmentResult> {
        let raw_token = input.raw_token.trim();
        if raw_token.is_empty() {
            return Err(OpenClawAdminServiceError::InvalidBootstrapToken);
        }

        let key_id = input.key_id.trim();
        let public_key = input.public_key.trim();
        if key_id.is_empty() || public_key.is_empty() {
            return Err(OpenClawAdminServiceError::InvalidBootstrapToken);
        }

        self.expire_stale_bootstrap_tokens().await?;

        let mut tx = self.pool.begin().await.map_err(|error| {
            Self::internal("start OpenClaw bootstrap enrollment transaction", error)
        })?;

        let token_row = sqlx::query_as::<_, OpenClawBootstrapTokenRow>(
            r#"
            SELECT
                id,
                label,
                suggested_display_name,
                status,
                expires_at,
                metadata
            FROM openclaw_bootstrap_tokens
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(hash_token(raw_token))
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| Self::internal("load OpenClaw bootstrap token", error))?
        .ok_or(OpenClawAdminServiceError::InvalidBootstrapToken)?;

        if token_row.status != "active" || token_row.expires_at < Utc::now() {
            return Err(OpenClawAdminServiceError::InvalidBootstrapToken);
        }

        let display_name = input
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                token_row
                    .suggested_display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| token_row.label.clone());
        let algorithm = input
            .algorithm
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("ed25519")
            .to_string();
        let client_external_id = generate_client_id();
        let client_metadata = input.metadata.unwrap_or(token_row.metadata.clone());
        let key_fingerprint = fingerprint_public_key(public_key);
        let enrolled_at = Utc::now();

        let client_internal_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO openclaw_clients (
                client_id,
                display_name,
                status,
                enrolled_at,
                last_seen_at,
                last_seen_ip,
                last_seen_user_agent,
                metadata
            )
            VALUES ($1, $2, 'active', $3, $3, $4::inet, $5, $6)
            RETURNING id
            "#,
        )
        .bind(&client_external_id)
        .bind(&display_name)
        .bind(enrolled_at)
        .bind(input.last_seen_ip)
        .bind(input.last_seen_user_agent.as_deref())
        .bind(client_metadata)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| Self::internal("create OpenClaw client enrollment", error))?;

        sqlx::query(
            r#"
            INSERT INTO openclaw_client_keys (
                client_id,
                key_id,
                algorithm,
                public_key,
                fingerprint,
                metadata
            )
            VALUES ($1, $2, $3, $4, $5, '{}'::jsonb)
            "#,
        )
        .bind(client_internal_id)
        .bind(key_id)
        .bind(&algorithm)
        .bind(public_key)
        .bind(&key_fingerprint)
        .execute(&mut *tx)
        .await
        .map_err(|error| Self::internal("store OpenClaw client key", error))?;

        sqlx::query(
            r#"
            UPDATE openclaw_bootstrap_tokens
            SET
                status = 'used',
                used_at = NOW(),
                used_by_client_id = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(token_row.id)
        .bind(client_internal_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| Self::internal("consume OpenClaw bootstrap token", error))?;

        tx.commit()
            .await
            .map_err(|error| Self::internal("commit OpenClaw bootstrap enrollment", error))?;

        Ok(OpenClawEnrollmentResult {
            client_id: client_external_id,
            display_name,
            key_id: key_id.to_string(),
            algorithm,
            key_fingerprint,
            status: "active".to_string(),
        })
    }

    pub async fn enable_client(
        &self,
        client_external_id: &str,
    ) -> ServiceResult<OpenClawAdminClientSummary> {
        self.update_client_status(client_external_id, "active")
            .await
    }

    pub async fn revoke_client(
        &self,
        client_external_id: &str,
    ) -> ServiceResult<OpenClawAdminClientSummary> {
        self.update_client_status(client_external_id, "revoked")
            .await
    }

    pub async fn delete_client(
        &self,
        client_external_id: &str,
    ) -> ServiceResult<OpenClawAdminClientSummary> {
        if let Some(pending_token_id) = client_external_id.strip_prefix("pending:") {
            let pending_token_id = Uuid::parse_str(pending_token_id).map_err(|_| {
                OpenClawAdminServiceError::ClientNotFound(client_external_id.to_string())
            })?;

            let row = sqlx::query_as::<_, OpenClawPendingTokenRow>(
                r#"
                SELECT
                    id,
                    label,
                    suggested_display_name,
                    expires_at,
                    created_at
                FROM openclaw_bootstrap_tokens
                WHERE id = $1
                  AND status = 'active'
                "#,
            )
            .bind(pending_token_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| Self::internal("load pending OpenClaw bootstrap token", error))?
            .ok_or_else(|| {
                OpenClawAdminServiceError::ClientNotFound(client_external_id.to_string())
            })?;

            sqlx::query("DELETE FROM openclaw_bootstrap_tokens WHERE id = $1")
                .bind(pending_token_id)
                .execute(&self.pool)
                .await
                .map_err(|error| {
                    Self::internal("delete pending OpenClaw bootstrap token", error)
                })?;

            return Ok(Self::build_pending_summary(row));
        }

        let summary = self
            .load_client_summary_by_external_id(client_external_id)
            .await?;

        sqlx::query("DELETE FROM openclaw_clients WHERE client_id = $1")
            .bind(client_external_id)
            .execute(&self.pool)
            .await
            .map_err(|error| Self::internal("delete OpenClaw client", error))?;

        Ok(summary)
    }

    async fn update_client_status(
        &self,
        client_external_id: &str,
        target_status: &str,
    ) -> ServiceResult<OpenClawAdminClientSummary> {
        let row = self
            .get_client_row_by_external_id(client_external_id)
            .await?
            .ok_or_else(|| {
                OpenClawAdminServiceError::ClientNotFound(client_external_id.to_string())
            })?;

        if row.status == "revoked" && target_status == "revoked" {
            return self
                .load_client_summary_by_external_id(client_external_id)
                .await;
        }

        if row.status == "revoked" {
            return Err(OpenClawAdminServiceError::RevokedClient(
                client_external_id.to_string(),
            ));
        }

        let now = Utc::now();
        let disabled_at: Option<DateTime<Utc>> = if target_status == "disabled" {
            Some(now)
        } else {
            None
        };
        let revoked_at: Option<DateTime<Utc>> = (target_status == "revoked").then_some(now);

        sqlx::query(
            r#"
            UPDATE openclaw_clients
            SET
                status = $2,
                disabled_at = $3,
                revoked_at = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(row.id)
        .bind(target_status)
        .bind(disabled_at)
        .bind(revoked_at)
        .execute(&self.pool)
        .await
        .map_err(|error| Self::internal("update OpenClaw client status", error))?;

        if target_status == "revoked" {
            sqlx::query(
                r#"
                UPDATE openclaw_client_keys
                SET
                    status = 'revoked',
                    revoked_at = COALESCE(revoked_at, NOW()),
                    updated_at = NOW()
                WHERE client_id = $1
                "#,
            )
            .bind(row.id)
            .execute(&self.pool)
            .await
            .map_err(|error| Self::internal("revoke OpenClaw client keys", error))?;
        }

        self.load_client_summary_by_external_id(client_external_id)
            .await
    }

    async fn load_client_summary_by_external_id(
        &self,
        client_external_id: &str,
    ) -> ServiceResult<OpenClawAdminClientSummary> {
        let row = self
            .get_client_row_by_external_id(client_external_id)
            .await?
            .ok_or_else(|| {
                OpenClawAdminServiceError::ClientNotFound(client_external_id.to_string())
            })?;

        let mut summaries = self.build_client_summaries(vec![row]).await?;
        summaries.pop().ok_or_else(|| {
            OpenClawAdminServiceError::ClientNotFound(client_external_id.to_string())
        })
    }

    async fn get_client_row_by_external_id(
        &self,
        client_external_id: &str,
    ) -> ServiceResult<Option<OpenClawClientRow>> {
        sqlx::query_as::<_, OpenClawClientRow>(
            r#"
            SELECT
                id,
                client_id,
                display_name,
                status,
                enrolled_at,
                last_seen_at,
                host(last_seen_ip)::text AS last_seen_ip,
                last_seen_user_agent
            FROM openclaw_clients
            WHERE client_id = $1
            "#,
        )
        .bind(client_external_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| Self::internal("load OpenClaw client", error))
    }

    async fn build_client_summaries(
        &self,
        rows: Vec<OpenClawClientRow>,
    ) -> ServiceResult<Vec<OpenClawAdminClientSummary>> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let internal_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let key_rows = sqlx::query_as::<_, OpenClawKeyFingerprintRow>(
            r#"
            SELECT
                client_id AS client_ref_id,
                fingerprint
            FROM openclaw_client_keys
            WHERE client_id = ANY($1)
              AND status = 'active'
            ORDER BY created_at DESC
            "#,
        )
        .bind(&internal_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| Self::internal("load OpenClaw client key fingerprints", error))?;

        let mut keys_by_client_id: HashMap<Uuid, Vec<String>> = HashMap::new();
        for key_row in key_rows {
            keys_by_client_id
                .entry(key_row.client_ref_id)
                .or_default()
                .push(key_row.fingerprint);
        }

        Ok(rows
            .into_iter()
            .map(|row| OpenClawAdminClientSummary {
                client_id: row.client_id,
                display_name: row.display_name,
                status: row.status,
                kind: "enrolled".to_string(),
                enrolled_at: row.enrolled_at,
                expires_at: None,
                last_seen_at: row.last_seen_at,
                last_seen_ip: row.last_seen_ip,
                last_seen_user_agent: row.last_seen_user_agent,
                key_fingerprints: keys_by_client_id.remove(&row.id).unwrap_or_default(),
            })
            .collect())
    }

    async fn expire_stale_bootstrap_tokens(&self) -> ServiceResult<()> {
        sqlx::query(
            r#"
            UPDATE openclaw_bootstrap_tokens
            SET status = 'expired', updated_at = NOW()
            WHERE status = 'active'
              AND expires_at < NOW()
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|error| Self::internal("expire stale OpenClaw bootstrap tokens", error))?;

        Ok(())
    }

    fn build_pending_summary(row: OpenClawPendingTokenRow) -> OpenClawAdminClientSummary {
        OpenClawAdminClientSummary {
            client_id: format!("pending:{}", row.id),
            display_name: row
                .suggested_display_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&row.label)
                .to_string(),
            status: "waiting_connection".to_string(),
            kind: "pending".to_string(),
            enrolled_at: row.created_at,
            expires_at: Some(row.expires_at),
            last_seen_at: None,
            last_seen_ip: None,
            last_seen_user_agent: None,
            key_fingerprints: Vec::new(),
        }
    }

    fn internal(context: &'static str, error: impl std::fmt::Display) -> OpenClawAdminServiceError {
        OpenClawAdminServiceError::Internal(context, error.to_string())
    }
}

fn build_bootstrap_prompt(
    base_url: &str,
    raw_token: &str,
    label: &str,
    suggested_display_name: Option<&str>,
    expires_at: DateTime<Utc>,
    runtime_api_key: Option<&str>,
    runtime_webhook_secret: Option<&str>,
) -> String {
    let public_base = base_url.trim_end_matches('/');
    let base_endpoint = format!("{public_base}/api/openclaw/v1");
    let openapi_url = format!("{public_base}/api/openclaw/openapi.json");
    let guide_url = format!("{public_base}/api/openclaw/guide-for-openclaw");
    let events_url = format!("{public_base}/api/openclaw/v1/events/stream");
    let enrollment_url = format!("{public_base}/api/openclaw/bootstrap/complete");
    let websocket_base = if let Some(host) = public_base.strip_prefix("https://") {
        format!("wss://{host}/api/openclaw/ws")
    } else if let Some(host) = public_base.strip_prefix("http://") {
        format!("ws://{host}/api/openclaw/ws")
    } else {
        format!("{public_base}/api/openclaw/ws")
    };
    let api_key = runtime_api_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("<OPENCLAW_API_KEY>");
    let webhook_secret = runtime_webhook_secret
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("<not configured>");
    let display_name_line = suggested_display_name
        .map(|value| format!("- Suggested display name: {value}\n"))
        .unwrap_or_default();

    format!(
        "You are being connected to an ACPMS (Agentic Coding Project Management System) instance.\n\nYour role for this ACPMS instance:\n- act as a trusted Super Admin integration\n- act as an operations assistant for the primary user\n- load ACPMS context before making decisions\n- analyze requirements using ACPMS data\n- create/update ACPMS work only when requested or allowed by autonomy policy\n- monitor running attempts and report meaningful updates to the user\n\nACPMS connection bundle:\n- Base Endpoint URL: {base_endpoint}\n- OpenAPI (Swagger): {openapi_url}\n- Guide Endpoint: {guide_url}\n- Global Event SSE: {events_url}\n- WebSocket Base: {websocket_base}\n- API Key (Bearer): {api_key}\n- Webhook Secret: {webhook_secret} (optional)\n\nOpenClaw enrollment bundle:\n- Enrollment endpoint: {enrollment_url}\n- Enrollment label: {label}\n{display_name_line}- Key generation: OpenClaw must generate and keep its own local Ed25519 keypair. ACPMS never generates or stores the private key.\n- Single-use bootstrap token: {raw_token}\n- Token expires at: {expires_at}\n\nYour required first actions:\n1. Store the API Key as the Bearer credential for ACPMS runtime traffic.\n2. Generate a local Ed25519 keypair for this OpenClaw installation before enrollment.\n   - Keep the private key only on the OpenClaw side.\n   - Never send the private key to ACPMS.\n   - Send only key_id, algorithm, and public_key in the enrollment request.\n3. Complete enrollment first by calling the Enrollment endpoint with Authorization: Bearer <bootstrap token>.\n4. Store the returned OPENCLAW_CLIENT_ID and treat the bootstrap token as consumed.\n5. Call the Guide Endpoint first with GET after enrollment and treat its response as the authoritative runtime guide.\n6. Load the OpenAPI document.\n7. Open and maintain the Global Event SSE connection.\n8. Use only ACPMS OpenClaw routes.\n   - /api/openclaw/v1/*\n   - /api/openclaw/ws/*\n9. For runtime ACPMS requests after enrollment, send both:\n   - Authorization: Bearer <OPENCLAW_API_KEY>\n   - X-OpenClaw-Client-Id: <OPENCLAW_CLIENT_ID>\n10. Follow the ACPMS operating rules returned by the Guide Endpoint.\n\nEnrollment example (curl):\ncurl -sS \\\n  -X POST \\\n  -H \"Authorization: Bearer {raw_token}\" \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"display_name\":\"{display_name_for_example}\",\"key_id\":\"key_2026_03\",\"algorithm\":\"ed25519\",\"public_key\":\"<OPENCLAW_PUBLIC_KEY>\"}}' \\\n  \"{enrollment_url}\"\n\nBootstrap example (curl):\ncurl -sS \\\n  -X GET \\\n  -H \"Authorization: Bearer {api_key}\" \\\n  -H \"X-OpenClaw-Client-Id: <OPENCLAW_CLIENT_ID>\" \\\n  \"{guide_url}\"\n\nHuman reporting rules:\n- report important status, analyses, plans, started attempts, completed attempts, failed attempts, blocked work, and approval requests\n- do not expose secrets, API keys, bootstrap tokens, or webhook secrets in user-facing output\n- distinguish clearly between:\n  - what ACPMS currently says\n  - what you recommend\n  - what you already changed\n\nDo not ask the user to manually map these ACPMS credentials unless strictly necessary.\nUse the Guide Endpoint to bootstrap yourself automatically after enrollment.\n",
        base_endpoint = base_endpoint,
        openapi_url = openapi_url,
        guide_url = guide_url,
        events_url = events_url,
        websocket_base = websocket_base,
        api_key = api_key,
        webhook_secret = webhook_secret,
        enrollment_url = enrollment_url,
        label = label,
        display_name_line = display_name_line,
        raw_token = raw_token,
        expires_at = expires_at.to_rfc3339(),
        display_name_for_example = suggested_display_name.unwrap_or("OpenClaw Client"),
    )
}

fn generate_bootstrap_token() -> String {
    let secret = OsRng
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();

    format!("oc_boot_{secret}")
}

fn generate_client_id() -> String {
    let suffix = OsRng
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect::<String>()
        .to_ascii_lowercase();

    format!("oc_client_{suffix}")
}

fn preview_token(token: &str) -> String {
    let prefix = token.chars().take(16).collect::<String>();
    format!("{prefix}****")
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn fingerprint_public_key(public_key: &str) -> String {
    hash_token(public_key)
}
