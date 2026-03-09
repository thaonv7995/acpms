use serde_json::{Map, Value};
use utoipa::OpenApi;

use crate::{api, routes};

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::auth::register,
        routes::auth::login,
        routes::users::list_users,
        routes::users::get_user,
        routes::users::update_user,
        routes::users::delete_user,
        routes::users::get_avatar_upload_url,
        routes::projects::create_project,
        routes::projects::list_projects,
        routes::projects::get_project,
        routes::projects::update_project,
        routes::projects::delete_project,
        routes::projects::recheck_project_repository_access,
        routes::projects::link_existing_fork,
        routes::projects::create_project_fork,
        routes::projects::import_project_preflight,
        routes::projects::import_project_create_fork,
        routes::projects::import_project,
        routes::tasks::create_task,
        routes::tasks::list_tasks,
        routes::tasks::get_task,
        routes::tasks::update_task,
        routes::tasks::delete_task,
        routes::tasks::update_task_status,
        routes::tasks::get_task_children,
        routes::tasks::assign_task,
        routes::tasks::update_task_metadata,
        routes::sprints::list_project_sprints,
        routes::sprints::create_sprint,
        routes::sprints::generate_sprints,
        routes::sprints::get_sprint,
        routes::sprints::update_sprint,
        routes::sprints::delete_sprint,
        routes::sprints::get_active_sprint,
        routes::sprints::activate_sprint,
        routes::sprints::close_sprint,
        routes::sprints::get_sprint_overview,
        routes::requirements::create_requirement,
        routes::requirements::list_project_requirements,
        routes::requirements::get_requirement,
        routes::requirements::update_requirement,
        routes::requirements::delete_requirement,
        routes::requirement_breakdowns::start_requirement_breakdown,
        routes::requirement_breakdowns::get_requirement_breakdown_session,
        routes::requirement_breakdowns::confirm_requirement_breakdown,
        routes::requirement_breakdowns::confirm_requirement_breakdown_manual,
        routes::requirement_breakdowns::cancel_requirement_breakdown,
        routes::requirement_breakdowns::start_requirement_task_sequence,
        routes::dashboard::get_dashboard,
        routes::task_attempts::create_task_attempt,
        routes::task_attempts::get_task_attempts,
        routes::task_attempts::get_attempt,
        routes::task_attempts::get_attempt_logs,
        routes::task_attempts::patch_attempt_log,
        routes::execution_processes::list_execution_processes,
        routes::execution_processes::get_execution_process,
        routes::execution_processes::get_execution_process_raw_logs,
        routes::execution_processes::get_execution_process_normalized_logs,
        routes::execution_processes::follow_up_execution_process,
        routes::execution_processes::reset_execution_process,
        routes::task_attempts::send_attempt_input,
        routes::task_attempts::cancel_attempt,
        routes::gitlab::list_merge_requests,
        routes::gitlab::get_merge_request_stats,
        routes::gitlab::link_project,
        routes::gitlab::get_status,
        routes::gitlab::get_task_merge_requests,
        routes::gitlab::handle_webhook,
        routes::health::health_check,
        routes::health::readiness_check,
        routes::health::liveness_check,
    ),
    components(
        schemas(
            api::UserDto,
            api::AuthResponseDto,
            api::UserResponse,
            api::UserListResponse,
            api::AuthResponse,
            api::EmptyResponse,
            api::ResponseCode,
            api::ApiErrorDetail,
            routes::auth::RegisterRequest,
            routes::auth::LoginRequest,
            routes::users::UpdateUserRequest,
            routes::users::GetUploadUrlRequest,
            routes::users::UploadUrlResponse,
            api::ProjectDto,
            api::ProjectResponse,
            api::ProjectListResponse,
            api::ProjectStackSelectionDoc,
            api::CreateProjectRequestDoc,
            api::UpdateProjectRequestDoc,
            acpms_db::models::ProjectSettings,
            acpms_db::models::ProjectSettingsResponse,
            routes::projects::ImportProjectRequest,
            routes::projects::ImportProjectResponse,
            api::TaskDto,
            api::TaskResponse,
            api::TaskListResponse,
            api::CreateTaskRequestDoc,
            api::UpdateTaskRequestDoc,
            routes::tasks::UpdateStatusRequest,
            routes::tasks::AssignTaskRequest,
            routes::tasks::UpdateMetadataRequest,
            api::SprintDto,
            api::SprintResponse,
            api::SprintListResponse,
            api::CreateSprintRequestDoc,
            api::UpdateSprintRequestDoc,
            api::GenerateSprintsRequestDoc,
            api::CreateNextSprintRequestDoc,
            api::CloseSprintRequestDoc,
            api::CloseSprintResultResponse,
            api::SprintOverviewResponse,
            acpms_db::models::SprintCarryOverMode,
            acpms_db::models::CreateNextSprintRequest,
            acpms_db::models::CloseSprintRequest,
            acpms_db::models::CloseSprintResult,
            acpms_db::models::SprintOverview,
            api::RequirementDto,
            api::RequirementResponse,
            api::RequirementListResponse,
            api::CreateRequirementRequestDoc,
            api::UpdateRequirementRequestDoc,
            api::DashboardResponse,
            api::DashboardDataDoc,
            api::DashboardStatsDoc,
            api::StatsMetricDoc,
            api::AgentStatsDoc,
            api::SystemLoadDoc,
            api::PrStatsDoc,
            api::DashboardProjectDoc,
            api::AgentAvatarDoc,
            api::DashboardAgentLogDoc,
            api::DashboardHumanTaskDoc,
            api::UserAvatarDoc,
            api::TaskAttemptDto,
            api::TaskAttemptResponse,
            api::TaskAttemptListResponse,
            api::AgentLogDto,
            api::AgentLogListResponse,
            api::CreateTaskAttemptRequestDoc,
            api::SendInputRequestDoc,
            routes::task_attempts::CancelAttemptRequest,
            routes::task_attempts::ResumeAttemptRequest,
            routes::execution_processes::ExecutionProcessDto,
            api::GitLabConfigurationDto,
            api::MergeRequestDto,
            api::MergeRequestOverviewDto,
            api::MergeRequestStatsDto,
            api::GitLabConfigurationResponse,
            api::MergeRequestListResponse,
            api::MergeRequestOverviewListResponse,
            api::MergeRequestStatsResponse,
            api::LinkGitLabProjectRequestDoc,
            routes::health::HealthStatus,
            routes::health::ComponentHealth,
            routes::health::HealthResponse,
            routes::openclaw::OpenClawGuideRequest,
            routes::openclaw::OpenClawReportingRequest,
            routes::openclaw::OpenClawPrimaryUserRequest,
            routes::openclaw::OpenClawReportingChannel,
            routes::openclaw::OpenClawGuideResponse,
            routes::openclaw::OpenClawAcpmsProfile,
            routes::openclaw::OpenClawHandoffContract,
            routes::openclaw::OpenClawOperatingModel,
            routes::openclaw::OpenClawOperatingRules,
            routes::openclaw::OpenClawAuthRules,
            routes::openclaw::OpenClawReportingPolicy,
            routes::openclaw::OpenClawConnectionStatus,
            routes::openclaw::OpenClawNextCall,
            routes::openclaw::OpenClawEventStreamParams,
            routes::openclaw::OpenClawEventCursorExpiredData,
            routes::openclaw::OpenClawGuideApiResponseDoc,
            routes::openclaw::OpenClawCursorExpiredApiResponseDoc,
        )
    ),
    tags(
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Users", description = "User management endpoints"),
        (name = "Projects", description = "Project management endpoints"),
        (name = "Tasks", description = "Task management endpoints"),
        (name = "Sprints", description = "Sprint management endpoints"),
        (name = "Requirements", description = "Requirement management endpoints"),
        (name = "Dashboard", description = "Dashboard endpoints"),
        (name = "Task Attempts", description = "Task attempt endpoints"),
        (name = "GitLab", description = "GitLab integration endpoints"),
        (name = "Health", description = "Health check endpoints"),
        (name = "OpenClaw", description = "OpenClaw gateway REST endpoints"),
        (name = "OpenClaw WebSocket", description = "OpenClaw gateway WebSocket upgrade endpoints"),
    )
)]
pub struct ApiDoc;

fn openclaw_bearer_security() -> Value {
    serde_json::json!([{ "bearer_auth": [] }])
}

fn openclaw_ws_operation(summary: &str, description: &str, parameters: Vec<Value>) -> Value {
    serde_json::json!({
        "get": {
            "tags": ["OpenClaw WebSocket"],
            "summary": summary,
            "description": description,
            "parameters": parameters,
            "security": openclaw_bearer_security(),
            "responses": {
                "101": {
                    "description": "WebSocket upgrade accepted"
                },
                "401": {
                    "description": "Missing or invalid OpenClaw bearer token"
                },
                "403": {
                    "description": "Gateway disabled or access forbidden"
                },
                "404": {
                    "description": "Requested resource was not found"
                }
            }
        }
    })
}

pub fn build_openclaw_openapi_json() -> Value {
    let mut document =
        serde_json::to_value(ApiDoc::openapi()).expect("ApiDoc OpenAPI should serialize");

    if let Some(components) = document
        .get_mut("components")
        .and_then(Value::as_object_mut)
    {
        let security_schemes = components
            .entry("securitySchemes".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(security_schemes) = security_schemes.as_object_mut() {
            security_schemes
                .entry("bearer_auth".to_string())
                .or_insert_with(|| {
                    serde_json::json!({
                        "type": "http",
                        "scheme": "bearer",
                        "bearerFormat": "API Key"
                    })
                });
        }
    }

    let Some(paths) = document.get_mut("paths").and_then(Value::as_object_mut) else {
        return document;
    };

    let mut rewritten = Map::new();
    for (path, item) in std::mem::take(paths) {
        if path.starts_with("/api/v1/") || path == "/api/v1" {
            let new_path = path.replacen("/api/v1", "/api/openclaw/v1", 1);
            rewritten.insert(new_path, item);
        }
    }

    rewritten.insert(
        "/api/openclaw/guide-for-openclaw".to_string(),
        serde_json::json!({
            "post": {
                "tags": ["OpenClaw"],
                "summary": "Bootstrap the OpenClaw integration",
                "description": "Returns the authoritative runtime guide, operating rules, reporting policy, and ACPMS connection profile that OpenClaw should load before controlling ACPMS.",
                "security": openclaw_bearer_security(),
                "requestBody": {
                    "required": false,
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/OpenClawGuideRequest"
                            }
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Bootstrap guide generated successfully",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/OpenClawGuideApiResponseDoc"
                                }
                            }
                        }
                    },
                    "401": {
                        "description": "Missing or invalid OpenClaw bearer token"
                    },
                    "403": {
                        "description": "Gateway disabled or not configured"
                    }
                }
            }
        }),
    );
    rewritten.insert(
        "/api/openclaw/v1/events/stream".to_string(),
        serde_json::json!({
            "get": {
                "tags": ["OpenClaw"],
                "summary": "Subscribe to OpenClaw lifecycle events",
                "description": "Server-sent event stream for OpenClaw lifecycle updates. Supports replay via `Last-Event-ID` or `after` query parameter and resumes into live mode once retained backlog is drained.",
                "security": openclaw_bearer_security(),
                "parameters": [
                    {
                        "name": "after",
                        "in": "query",
                        "required": false,
                        "schema": {
                            "type": "string"
                        },
                        "description": "Replay events strictly after this cursor."
                    },
                    {
                        "name": "Last-Event-ID",
                        "in": "header",
                        "required": false,
                        "schema": {
                            "type": "string"
                        },
                        "description": "Replay events strictly after this cursor using the standard SSE resume header."
                    }
                ],
                "responses": {
                    "200": {
                        "description": "SSE event stream",
                        "content": {
                            "text/event-stream": {
                                "schema": {
                                    "type": "string"
                                }
                            }
                        }
                    },
                    "400": {
                        "description": "Invalid cursor or conflicting resume parameters"
                    },
                    "409": {
                        "description": "Requested cursor has expired from the retained replay window",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/OpenClawCursorExpiredApiResponseDoc"
                                }
                            }
                        }
                    },
                    "401": {
                        "description": "Missing or invalid OpenClaw bearer token"
                    }
                }
            }
        }),
    );
    rewritten.insert(
        "/api/openclaw/ws/attempts/{id}/logs".to_string(),
        openclaw_ws_operation(
            "Stream raw attempt logs",
            "WebSocket stream of agent logs and status updates for a specific task attempt.",
            vec![serde_json::json!({
                "name": "id",
                "in": "path",
                "required": true,
                "schema": { "type": "string", "format": "uuid" }
            })],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/attempts/{id}/diffs".to_string(),
        openclaw_ws_operation(
            "Stream attempt diffs",
            "WebSocket stream of file diff updates for a specific task attempt.",
            vec![serde_json::json!({
                "name": "id",
                "in": "path",
                "required": true,
                "schema": { "type": "string", "format": "uuid" }
            })],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/attempts/{id}/stream".to_string(),
        openclaw_ws_operation(
            "Stream normalized attempt events",
            "WebSocket mirror of `/api/openclaw/v1/attempts/{id}/stream` for normalized attempt stream messages.",
            vec![
                serde_json::json!({
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "since",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "integer", "format": "uint64", "minimum": 0 }
                })
            ],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/projects/{project_id}/agents".to_string(),
        openclaw_ws_operation(
            "Stream project agent activity",
            "WebSocket stream of live agent events across attempts belonging to a single project.",
            vec![serde_json::json!({
                "name": "project_id",
                "in": "path",
                "required": true,
                "schema": { "type": "string", "format": "uuid" }
            })],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/projects/{project_id}/assistant/sessions/{session_id}/logs".to_string(),
        openclaw_ws_operation(
            "Stream project assistant session logs",
            "WebSocket stream of assistant log events for a project assistant session.",
            vec![
                serde_json::json!({
                    "name": "project_id",
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "session_id",
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
            ],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/agent-activity/status".to_string(),
        openclaw_ws_operation(
            "Stream global agent activity",
            "WebSocket stream of ACPMS agent activity status for dashboard-style supervision.",
            vec![],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/execution-processes/{id}/raw-logs".to_string(),
        openclaw_ws_operation(
            "Stream raw execution-process logs",
            "WebSocket stream of raw logs for a single execution process.",
            vec![
                serde_json::json!({
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "since_seq",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "integer", "format": "uint64", "minimum": 0 }
                }),
            ],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/execution-processes/{id}/normalized-logs".to_string(),
        openclaw_ws_operation(
            "Stream normalized execution-process logs",
            "WebSocket stream of normalized logs for a single execution process.",
            vec![
                serde_json::json!({
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "since_seq",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "integer", "format": "uint64", "minimum": 0 }
                }),
            ],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/execution-processes/stream/attempt".to_string(),
        openclaw_ws_operation(
            "Stream execution processes by attempt",
            "WebSocket stream of execution-process collection updates for a task attempt.",
            vec![
                serde_json::json!({
                    "name": "attempt_id",
                    "in": "query",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "since_seq",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "integer", "format": "uint64", "minimum": 0 }
                }),
            ],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/execution-processes/stream/session".to_string(),
        openclaw_ws_operation(
            "Stream execution processes by session",
            "WebSocket stream of execution-process collection updates using session-scoped query semantics.",
            vec![
                serde_json::json!({
                    "name": "session_id",
                    "in": "query",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "since_seq",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "integer", "format": "uint64", "minimum": 0 }
                })
            ],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/approvals/stream".to_string(),
        openclaw_ws_operation(
            "Stream approval requests",
            "WebSocket stream of approval request updates. Requires either `attempt_id` or `execution_process_id`.",
            vec![
                serde_json::json!({
                    "name": "attempt_id",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "execution_process_id",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "projection",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "string", "enum": ["legacy", "patch"] }
                }),
                serde_json::json!({
                    "name": "since_seq",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "integer", "format": "uint64", "minimum": 0 }
                })
            ],
        ),
    );
    rewritten.insert(
        "/api/openclaw/ws/agent/auth/sessions/{id}".to_string(),
        openclaw_ws_operation(
            "Stream agent auth-session status",
            "WebSocket stream of agent authentication session snapshots and updates.",
            vec![
                serde_json::json!({
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }),
                serde_json::json!({
                    "name": "since_seq",
                    "in": "query",
                    "required": false,
                    "schema": { "type": "integer", "format": "uint64", "minimum": 0 }
                }),
            ],
        ),
    );
    *paths = rewritten;

    if let Some(info) = document.get_mut("info").and_then(Value::as_object_mut) {
        info.insert(
            "title".to_string(),
            Value::String("ACPMS OpenClaw Gateway API".to_string()),
        );
    }

    document
}
