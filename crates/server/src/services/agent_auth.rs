use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
use tokio::sync::{Mutex, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthFlowType {
    DeviceFlow,
    OobCode,
    LoopbackProxy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthSessionStatus {
    Initiated,
    WaitingUserAction,
    Verifying,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthSessionRecord {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub flow_type: AuthFlowType,
    pub status: AuthSessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub process_pid: Option<u32>,
    pub allowed_loopback_port: Option<u16>,
    pub last_seq: u64,
    pub last_error: Option<String>,
    pub result: Option<String>,
    pub action_url: Option<String>,
    pub action_code: Option<String>,
    pub action_hint: Option<String>,
}

#[derive(Clone, Default)]
pub struct AuthSessionStore {
    sessions: Arc<RwLock<HashMap<Uuid, AuthSessionRecord>>>,
    stdin_writers: Arc<RwLock<HashMap<Uuid, Arc<Mutex<ChildStdin>>>>>,
    submit_rate_windows: Arc<RwLock<HashMap<Uuid, SubmitRateWindow>>>,
}

#[derive(Debug, Clone)]
struct SubmitRateWindow {
    started_at: DateTime<Utc>,
    count: u32,
}

impl AuthSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create_session(
        &self,
        user_id: Uuid,
        provider: String,
        flow_type: AuthFlowType,
        ttl_seconds: i64,
    ) -> AuthSessionRecord {
        let now = Utc::now();
        let session = AuthSessionRecord {
            session_id: Uuid::new_v4(),
            user_id,
            provider,
            flow_type,
            status: AuthSessionStatus::Initiated,
            created_at: now,
            updated_at: now,
            expires_at: now + Duration::seconds(ttl_seconds.max(1)),
            process_pid: None,
            allowed_loopback_port: None,
            last_seq: 0,
            last_error: None,
            result: None,
            action_url: None,
            action_code: None,
            action_hint: None,
        };

        self.sessions
            .write()
            .await
            .insert(session.session_id, session.clone());

        session
    }

    pub async fn get_owned(&self, session_id: Uuid, user_id: Uuid) -> Option<AuthSessionRecord> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).and_then(|session| {
            if session.user_id == user_id {
                Some(session.clone())
            } else {
                None
            }
        })
    }

    pub async fn get(&self, session_id: Uuid) -> Option<AuthSessionRecord> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).cloned()
    }

    pub async fn update_owned_status(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        status: AuthSessionStatus,
        last_error: Option<String>,
        result: Option<String>,
    ) -> Option<AuthSessionRecord> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        if session.user_id != user_id {
            return None;
        }
        session.status = status;
        session.last_error = last_error;
        session.result = result;
        session.last_seq = session.last_seq.saturating_add(1);
        session.updated_at = Utc::now();
        Some(session.clone())
    }

    pub async fn update_status(
        &self,
        session_id: Uuid,
        status: AuthSessionStatus,
        last_error: Option<String>,
        result: Option<String>,
    ) -> Option<AuthSessionRecord> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.status = status;
        session.last_error = last_error;
        session.result = result;
        session.last_seq = session.last_seq.saturating_add(1);
        session.updated_at = Utc::now();
        Some(session.clone())
    }

    pub async fn set_process_info(
        &self,
        session_id: Uuid,
        process_pid: Option<u32>,
        allowed_loopback_port: Option<u16>,
    ) -> Option<AuthSessionRecord> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.process_pid = process_pid;
        session.allowed_loopback_port = allowed_loopback_port;
        session.last_seq = session.last_seq.saturating_add(1);
        session.updated_at = Utc::now();
        Some(session.clone())
    }

    pub async fn update_action(
        &self,
        session_id: Uuid,
        action_url: Option<String>,
        action_code: Option<String>,
        action_hint: Option<String>,
        allowed_loopback_port: Option<u16>,
    ) -> Option<AuthSessionRecord> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        if let Some(url) = action_url {
            session.action_url = Some(url);
        }
        if let Some(code) = action_code {
            session.action_code = Some(code);
        }
        if let Some(hint) = action_hint {
            session.action_hint = Some(hint);
        }
        if let Some(port) = allowed_loopback_port {
            session.allowed_loopback_port = Some(port);
        }
        session.last_seq = session.last_seq.saturating_add(1);
        session.updated_at = Utc::now();
        Some(session.clone())
    }

    pub async fn set_stdin_writer(
        &self,
        session_id: Uuid,
        writer: Option<ChildStdin>,
    ) -> Option<AuthSessionRecord> {
        if let Some(writer) = writer {
            self.stdin_writers
                .write()
                .await
                .insert(session_id, Arc::new(Mutex::new(writer)));
        } else {
            self.stdin_writers.write().await.remove(&session_id);
        }

        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.updated_at = Utc::now();
        Some(session.clone())
    }

    pub async fn write_to_stdin(&self, session_id: Uuid, input: &str) -> Result<(), String> {
        let writer = self
            .stdin_writers
            .read()
            .await
            .get(&session_id)
            .cloned()
            .ok_or_else(|| "Auth session cannot accept input right now".to_string())?;

        let mut writer = writer.lock().await;
        writer
            .write_all(input.as_bytes())
            .await
            .map_err(|e| format!("Failed writing auth input: {}", e))?;
        writer
            .flush()
            .await
            .map_err(|e| format!("Failed flushing auth input: {}", e))?;
        Ok(())
    }

    pub async fn check_and_record_submit_attempt(
        &self,
        session_id: Uuid,
        max_attempts: u32,
        window_seconds: i64,
    ) -> Result<(), String> {
        let now = Utc::now();
        let mut windows = self.submit_rate_windows.write().await;
        let window = windows.entry(session_id).or_insert(SubmitRateWindow {
            started_at: now,
            count: 0,
        });

        if now.signed_duration_since(window.started_at) > Duration::seconds(window_seconds.max(1)) {
            window.started_at = now;
            window.count = 0;
        }

        if window.count >= max_attempts {
            return Err(format!(
                "Too many auth submissions. Please wait up to {} seconds and try again.",
                window_seconds.max(1)
            ));
        }

        window.count = window.count.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthFlowType, AuthSessionStatus, AuthSessionStore};
    use uuid::Uuid;

    #[tokio::test]
    async fn submit_attempt_rate_limit_enforced() {
        let store = AuthSessionStore::new();
        let session = store
            .create_session(
                Uuid::new_v4(),
                "openai-codex".to_string(),
                AuthFlowType::DeviceFlow,
                300,
            )
            .await;

        for _ in 0..2 {
            store
                .check_and_record_submit_attempt(session.session_id, 2, 60)
                .await
                .expect("within rate limit");
        }

        let err = store
            .check_and_record_submit_attempt(session.session_id, 2, 60)
            .await
            .expect_err("expected rate limit error");
        assert!(err.to_lowercase().contains("too many auth submissions"));
    }

    #[tokio::test]
    async fn status_updates_increment_sequence() {
        let store = AuthSessionStore::new();
        let user_id = Uuid::new_v4();
        let session = store
            .create_session(
                user_id,
                "openai-codex".to_string(),
                AuthFlowType::DeviceFlow,
                300,
            )
            .await;
        assert_eq!(session.last_seq, 0);

        let updated = store
            .update_owned_status(
                session.session_id,
                user_id,
                AuthSessionStatus::WaitingUserAction,
                None,
                None,
            )
            .await
            .expect("session should update");
        assert_eq!(updated.last_seq, 1);
    }

    #[tokio::test]
    async fn get_owned_enforces_session_owner() {
        let store = AuthSessionStore::new();
        let owner_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();

        let session = store
            .create_session(
                owner_id,
                "claude-code".to_string(),
                AuthFlowType::LoopbackProxy,
                300,
            )
            .await;

        assert!(store
            .get_owned(session.session_id, owner_id)
            .await
            .is_some());
        assert!(store
            .get_owned(session.session_id, other_user_id)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn update_owned_status_rejects_non_owner() {
        let store = AuthSessionStore::new();
        let owner_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        let session = store
            .create_session(
                owner_id,
                "gemini-cli".to_string(),
                AuthFlowType::OobCode,
                300,
            )
            .await;

        let rejected = store
            .update_owned_status(
                session.session_id,
                other_user_id,
                AuthSessionStatus::Cancelled,
                Some("should fail".to_string()),
                None,
            )
            .await;
        assert!(rejected.is_none(), "non-owner must not update session");

        let current = store
            .get(session.session_id)
            .await
            .expect("session should exist");
        assert_eq!(current.status, AuthSessionStatus::Initiated);
        assert_eq!(current.last_seq, 0);
    }

    #[tokio::test]
    async fn owned_state_transitions_follow_expected_flow() {
        let store = AuthSessionStore::new();
        let user_id = Uuid::new_v4();
        let session = store
            .create_session(
                user_id,
                "openai-codex".to_string(),
                AuthFlowType::DeviceFlow,
                300,
            )
            .await;

        let waiting = store
            .update_owned_status(
                session.session_id,
                user_id,
                AuthSessionStatus::WaitingUserAction,
                None,
                Some("waiting".to_string()),
            )
            .await
            .expect("waiting update");
        assert_eq!(waiting.status, AuthSessionStatus::WaitingUserAction);

        let verifying = store
            .update_owned_status(
                session.session_id,
                user_id,
                AuthSessionStatus::Verifying,
                None,
                Some("verifying".to_string()),
            )
            .await
            .expect("verifying update");
        assert_eq!(verifying.status, AuthSessionStatus::Verifying);

        let succeeded = store
            .update_owned_status(
                session.session_id,
                user_id,
                AuthSessionStatus::Succeeded,
                None,
                Some("done".to_string()),
            )
            .await
            .expect("success update");
        assert_eq!(succeeded.status, AuthSessionStatus::Succeeded);
        assert_eq!(succeeded.result.as_deref(), Some("done"));
        assert!(succeeded.last_seq >= 3);
    }
}
