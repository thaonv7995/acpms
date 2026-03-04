use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use futures::{Stream, StreamExt};
use serde::Deserialize;
use std::convert::Infallible;
use uuid::Uuid;

use crate::middleware::{authenticate_bearer_token, Permission, RbacChecker};
use crate::{error::ApiError, AppState};

#[derive(Deserialize)]
pub struct StreamParams {
    pub since: Option<u64>, // Last seen sequence
}

/// SSE endpoint for streaming task attempt updates with JSON Patch
pub async fn stream_attempt_sse(
    Path(attempt_id): Path<Uuid>,
    Query(params): Query<StreamParams>,
    State(state): State<AppState>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let auth_user = authenticate_bearer_token(auth.token(), &state).await?;
    let user_id = auth_user.id;

    // Resolve project_id for this attempt, then apply RBAC.
    // This keeps behavior consistent with other attempt endpoints (system admin bypass, role checks).
    let project_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT t.project_id
        FROM task_attempts ta
        JOIN tasks t ON ta.task_id = t.id
        WHERE ta.id = $1
        "#,
    )
    .bind(attempt_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let project_id =
        project_id.ok_or_else(|| ApiError::NotFound("Task attempt not found".into()))?;

    // Require view permission to stream attempt logs/status updates.
    RbacChecker::check_permission(user_id, project_id, Permission::ViewTask, &state.db).await?;

    // Get stream from StreamService
    let stream = state
        .stream_service
        .stream_task_attempt_with_catchup(attempt_id, params.since)
        .await;

    // Convert to SSE events
    let sse_stream = stream.map(|msg_result| match msg_result {
        Ok(msg) => {
            let data = serde_json::to_string(&msg).unwrap_or_default();
            Ok(Event::default().data(data))
        }
        Err(_) => Ok(Event::default().data("error")),
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}
