use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
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
        routes::task_contexts::list_task_contexts,
        routes::task_contexts::create_task_context,
        routes::task_contexts::update_task_context,
        routes::task_contexts::delete_task_context,
        routes::task_contexts::get_task_context_attachment_upload_url,
        routes::task_contexts::create_task_context_attachment,
        routes::task_contexts::delete_task_context_attachment,
        routes::task_contexts::get_task_context_attachment_download_url,
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
            api::ProjectSummaryDto,
            api::ProjectResponse,
            api::ProjectListResponse,
            api::ProjectStackSelectionDoc,
            api::CreateProjectRequestDoc,
            api::UpdateProjectRequestDoc,
            acpms_db::models::ProjectSettings,
            acpms_db::models::ProjectSettingsResponse,
            acpms_db::models::RepositoryProvider,
            acpms_db::models::RepositoryAccessMode,
            acpms_db::models::RepositoryVerificationStatus,
            acpms_db::models::RepositoryContext,
            routes::projects::ImportProjectRequest,
            routes::projects::ImportProjectResponse,
            routes::projects::RecheckRepositoryAccessResponse,
            routes::projects::LinkExistingForkRequest,
            routes::projects::LinkExistingForkResponse,
            routes::projects::CreateForkResponse,
            routes::projects::ImportProjectPreflightRequest,
            routes::projects::ImportProjectPreflightResponse,
            routes::projects::ImportProjectCreateForkRequest,
            routes::projects::ImportProjectCreateForkResponse,
            api::TaskDto,
            api::TaskResponse,
            api::TaskListResponse,
            api::CreateTaskRequestDoc,
            api::UpdateTaskRequestDoc,
            api::TaskContextDto,
            api::TaskContextAttachmentDto,
            api::TaskContextResponse,
            api::TaskContextListResponse,
            api::TaskContextAttachmentResponse,
            api::CreateTaskContextRequestDoc,
            api::UpdateTaskContextRequestDoc,
            api::CreateTaskContextAttachmentRequestDoc,
            routes::tasks::UpdateStatusRequest,
            routes::tasks::AssignTaskRequest,
            routes::tasks::UpdateMetadataRequest,
            routes::task_contexts::TaskContextAttachmentUploadUrlRequest,
            routes::task_contexts::TaskContextAttachmentUploadUrlResponse,
            routes::task_contexts::TaskContextAttachmentDownloadUrlRequest,
            routes::task_contexts::TaskContextAttachmentDownloadUrlResponse,
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
            routes::requirement_breakdowns::RequirementBreakdownSessionDto,
            routes::requirement_breakdowns::ConfirmRequirementBreakdownResponse,
            routes::requirement_breakdowns::ConfirmManualRequirementBreakdownResponse,
            routes::requirement_breakdowns::StartRequirementTaskSequenceRequest,
            routes::requirement_breakdowns::StartRequirementTaskSequenceResponse,
            routes::requirement_breakdowns::BreakdownSprintAssignmentMode,
            routes::requirement_breakdowns::ConfirmRequirementBreakdownRequest,
            routes::requirement_breakdowns::ManualBreakdownTaskDraftRequest,
            routes::requirement_breakdowns::ConfirmManualRequirementBreakdownRequest,
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
            api::AgentLogResponse,
            api::AgentLogListResponse,
            api::CreateTaskAttemptRequestDoc,
            api::SendInputRequestDoc,
            routes::task_attempts::CancelAttemptRequest,
            routes::task_attempts::ResumeAttemptRequest,
            routes::task_attempts::UpdateLogRequest,
            routes::execution_processes::ExecutionProcessDto,
            routes::execution_processes::ResetExecutionProcessRequest,
            routes::execution_processes::ResetExecutionProcessResponse,
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpenClawOpenApiQuery {
    pub path: Option<String>,
    pub operation_id: Option<String>,
    pub tag: Option<String>,
    pub method: Option<String>,
}

impl OpenClawOpenApiQuery {
    fn has_filters(&self) -> bool {
        self.path.is_some()
            || self.operation_id.is_some()
            || self.tag.is_some()
            || self.method.is_some()
    }
}

fn openclaw_bearer_security() -> Value {
    serde_json::json!([{ "bearer_auth": [] }])
}

fn openclaw_search_examples() -> Value {
    serde_json::json!([
        "/api/openclaw/openapi.json?operation_id=list_tasks",
        "/api/openclaw/openapi.json?path=/api/openclaw/v1/tasks/{id}&method=get",
        "/api/openclaw/openapi.json?tag=Tasks&method=post"
    ])
}

fn openclaw_search_metadata(filters: &OpenClawOpenApiQuery, matches: &[Value]) -> Value {
    let matched_path_count = matches
        .iter()
        .filter_map(|entry| entry.get("path").and_then(Value::as_str))
        .collect::<BTreeSet<_>>()
        .len();

    serde_json::json!({
        "supported_filters": ["path", "operation_id", "tag", "method"],
        "examples": openclaw_search_examples(),
        "filters_applied": {
            "path": filters.path.clone(),
            "operation_id": filters.operation_id.clone(),
            "tag": filters.tag.clone(),
            "method": filters.method.clone()
        },
        "matched_path_count": matched_path_count,
        "matched_operation_count": matches.len(),
        "matches": matches
    })
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

fn openclaw_operation_keys() -> &'static [&'static str] {
    &[
        "get", "post", "put", "patch", "delete", "options", "head", "trace",
    ]
}

fn ensure_openclaw_path_security(item: &mut Value) {
    let Some(item) = item.as_object_mut() else {
        return;
    };

    for key in openclaw_operation_keys() {
        let Some(operation) = item.get_mut(*key).and_then(Value::as_object_mut) else {
            continue;
        };
        operation
            .entry("security".to_string())
            .or_insert_with(openclaw_bearer_security);
    }
}

fn matches_filter(candidate: &str, query: &Option<String>) -> bool {
    match query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(filter) => candidate
            .to_ascii_lowercase()
            .contains(&filter.to_ascii_lowercase()),
        None => true,
    }
}

fn operation_matches(
    path: &str,
    method: &str,
    operation: &Map<String, Value>,
    filters: &OpenClawOpenApiQuery,
) -> bool {
    if !matches_filter(path, &filters.path) {
        return false;
    }

    if !matches_filter(method, &filters.method) {
        return false;
    }

    if let Some(operation_id_filter) = filters
        .operation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !operation_id
            .to_ascii_lowercase()
            .contains(&operation_id_filter.to_ascii_lowercase())
        {
            return false;
        }
    }

    if let Some(tag_filter) = filters
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let tag_filter = tag_filter.to_ascii_lowercase();
        let has_matching_tag = operation
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(Value::as_str)
                    .any(|tag| tag.to_ascii_lowercase().contains(&tag_filter))
            })
            .unwrap_or(false);
        if !has_matching_tag {
            return false;
        }
    }

    true
}

fn collect_schema_refs(value: &Value, needed: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                    needed.insert(name.to_string());
                }
            }
            for child in map.values() {
                collect_schema_refs(child, needed);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_schema_refs(item, needed);
            }
        }
        _ => {}
    }
}

fn prune_unreferenced_schemas(document: &mut Value) {
    let mut needed = BTreeSet::new();
    if let Some(paths) = document.get("paths") {
        collect_schema_refs(paths, &mut needed);
    }

    loop {
        let current: Vec<String> = needed.iter().cloned().collect();
        let mut changed = false;

        for schema_name in current {
            let Some(schema) = document.pointer(&format!("/components/schemas/{schema_name}"))
            else {
                continue;
            };
            let before = needed.len();
            collect_schema_refs(schema, &mut needed);
            changed |= needed.len() != before;
        }

        if !changed {
            break;
        }
    }

    if let Some(schemas) = document
        .pointer_mut("/components/schemas")
        .and_then(Value::as_object_mut)
    {
        schemas.retain(|name, _| needed.contains(name));
    }
}

fn retain_used_tags(document: &mut Value, used_tags: &BTreeSet<String>) {
    let Some(tags) = document.get_mut("tags").and_then(Value::as_array_mut) else {
        return;
    };

    tags.retain(|tag| {
        tag.get("name")
            .and_then(Value::as_str)
            .map(|name| used_tags.contains(name))
            .unwrap_or(false)
    });
}

fn filter_paths(document: &mut Value, filters: &OpenClawOpenApiQuery) -> Vec<Value> {
    let Some(paths) = document.get_mut("paths").and_then(Value::as_object_mut) else {
        return Vec::new();
    };

    let original_paths = std::mem::take(paths);
    let mut filtered_paths = Map::new();
    let mut matches = Vec::new();
    let mut used_tags = BTreeSet::new();

    for (path, item) in original_paths {
        let Some(item_obj) = item.as_object() else {
            continue;
        };

        let mut filtered_item = Map::new();
        let mut has_operation = false;

        for (key, value) in item_obj {
            if openclaw_operation_keys().contains(&key.as_str()) {
                let Some(operation) = value.as_object() else {
                    continue;
                };
                if operation_matches(&path, key, operation, filters) {
                    has_operation = true;
                    filtered_item.insert(key.clone(), value.clone());
                    if let Some(tags) = operation.get("tags").and_then(Value::as_array) {
                        for tag in tags.iter().filter_map(Value::as_str) {
                            used_tags.insert(tag.to_string());
                        }
                    }
                    matches.push(serde_json::json!({
                        "path": path,
                        "method": key.to_ascii_uppercase(),
                        "operation_id": operation.get("operationId").and_then(Value::as_str),
                        "summary": operation.get("summary").and_then(Value::as_str),
                        "tags": operation.get("tags").cloned().unwrap_or(Value::Array(Vec::new()))
                    }));
                }
            } else if key == "parameters" {
                filtered_item.insert(key.clone(), value.clone());
            }
        }

        if has_operation {
            filtered_paths.insert(path, Value::Object(filtered_item));
        }
    }

    *paths = filtered_paths;
    retain_used_tags(document, &used_tags);
    matches
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
    for (path, mut item) in std::mem::take(paths) {
        if path.starts_with("/api/v1/") || path == "/api/v1" {
            ensure_openclaw_path_security(&mut item);
            let new_path = path.replacen("/api/v1", "/api/openclaw/v1", 1);
            rewritten.insert(new_path, item);
        }
    }

    rewritten.insert(
        "/api/openclaw/openapi.json".to_string(),
        serde_json::json!({
            "get": {
                "tags": ["OpenClaw"],
                "summary": "Load or search the OpenClaw OpenAPI contract",
                "description": "Returns the authenticated OpenAPI contract for OpenClaw. Use the optional filters to narrow the response to one endpoint or a small subset of operations.",
                "security": openclaw_bearer_security(),
                "parameters": [
                    {
                        "name": "path",
                        "in": "query",
                        "required": false,
                        "schema": { "type": "string" },
                        "description": "Case-insensitive path substring filter. Combine with `method` to isolate a single endpoint."
                    },
                    {
                        "name": "operation_id",
                        "in": "query",
                        "required": false,
                        "schema": { "type": "string" },
                        "description": "Case-insensitive filter for the OpenAPI `operationId`."
                    },
                    {
                        "name": "tag",
                        "in": "query",
                        "required": false,
                        "schema": { "type": "string" },
                        "description": "Case-insensitive filter for endpoint tags such as `Tasks` or `Projects`."
                    },
                    {
                        "name": "method",
                        "in": "query",
                        "required": false,
                        "schema": {
                            "type": "string",
                            "enum": ["get", "post", "put", "patch", "delete", "options", "head", "trace"]
                        },
                        "description": "Optional HTTP method filter, case-insensitive."
                    }
                ],
                "responses": {
                    "200": {
                        "description": "OpenClaw OpenAPI contract (full or filtered)",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object"
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
        "/api/openclaw/guide-for-openclaw".to_string(),
        serde_json::json!({
            "get": {
                "tags": ["OpenClaw"],
                "summary": "Bootstrap the OpenClaw integration",
                "description": "Returns the authoritative runtime guide, operating rules, reporting policy, and ACPMS connection profile that OpenClaw should load before controlling ACPMS. GET is supported for simple retrieval when no custom reporting payload is needed.",
                "security": openclaw_bearer_security(),
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
                    }
                }
            },
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
        info.insert(
            "description".to_string(),
            Value::String(
                "Authenticated ACPMS gateway contract for OpenClaw. All `/api/openclaw/*` routes require `Authorization: Bearer <OPENCLAW_API_KEY>`. To retrieve a smaller contract, call `/api/openclaw/openapi.json` with `path`, `operation_id`, `tag`, and optional `method` query filters."
                    .to_string(),
            ),
        );
    }

    if let Some(root) = document.as_object_mut() {
        root.insert(
            "x-openclaw-search".to_string(),
            openclaw_search_metadata(&OpenClawOpenApiQuery::default(), &[]),
        );
    }

    document
}

pub fn build_filtered_openclaw_openapi_json(filters: &OpenClawOpenApiQuery) -> Value {
    let mut document = build_openclaw_openapi_json();
    let matches = if filters.has_filters() {
        let matches = filter_paths(&mut document, filters);
        prune_unreferenced_schemas(&mut document);
        matches
    } else {
        Vec::new()
    };

    if let Some(root) = document.as_object_mut() {
        root.insert(
            "x-openclaw-search".to_string(),
            openclaw_search_metadata(filters, &matches),
        );
    }

    document
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::collections::BTreeSet;
    use utoipa::OpenApi;

    use super::{
        build_filtered_openclaw_openapi_json, build_openclaw_openapi_json, ApiDoc,
        OpenClawOpenApiQuery,
    };

    fn collect_missing_schema_refs(
        value: &Value,
        document: &Value,
        missing: &mut BTreeSet<String>,
    ) {
        match value {
            Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                    if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                        if document
                            .pointer(&format!("/components/schemas/{name}"))
                            .is_none()
                        {
                            missing.insert(name.to_string());
                        }
                    }
                }
                for child in map.values() {
                    collect_missing_schema_refs(child, document, missing);
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect_missing_schema_refs(item, document, missing);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn rewritten_v1_routes_require_bearer_security() {
        let document = build_openclaw_openapi_json();
        let security = document
            .pointer("/paths/~1api~1openclaw~1v1~1tasks/get/security")
            .and_then(|value| value.as_array())
            .expect("security should be present on rewritten OpenClaw routes");

        assert!(!security.is_empty());
    }

    #[test]
    fn includes_searchable_openapi_endpoint() {
        let document = build_openclaw_openapi_json();
        assert!(document
            .pointer("/paths/~1api~1openclaw~1openapi.json/get")
            .is_some());
    }

    #[test]
    fn generated_contract_has_no_dangling_schema_refs() {
        let document = build_openclaw_openapi_json();
        let mut missing = BTreeSet::new();
        collect_missing_schema_refs(&document, &document, &mut missing);
        assert!(missing.is_empty(), "missing schema refs: {missing:?}");
    }

    #[test]
    fn filter_can_reduce_contract_to_single_operation() {
        let document = build_filtered_openclaw_openapi_json(&OpenClawOpenApiQuery {
            operation_id: Some("list_tasks".to_string()),
            ..OpenClawOpenApiQuery::default()
        });

        let tasks_get = document
            .pointer("/paths/~1api~1openclaw~1v1~1tasks/get")
            .expect("filtered document should keep the matching operation");
        assert_eq!(
            tasks_get
                .get("operationId")
                .and_then(|value| value.as_str()),
            Some("list_tasks")
        );
        assert!(document
            .pointer("/paths/~1api~1openclaw~1v1~1projects")
            .is_none());
        assert!(document
            .pointer("/components/schemas/TaskListResponse")
            .is_some());
        assert!(document
            .pointer("/components/schemas/ProjectResponse")
            .is_none());
    }

    #[test]
    fn main_api_contract_exposes_project_summary_and_repository_context() {
        let document =
            serde_json::to_value(ApiDoc::openapi()).expect("ApiDoc OpenAPI should serialize");

        assert!(document
            .pointer("/components/schemas/ProjectDto/properties/summary")
            .is_some());
        assert!(document
            .pointer("/components/schemas/ProjectDto/properties/repository_context")
            .is_some());
        assert!(document
            .pointer("/paths/~1api~1v1~1projects~1{id}~1repository-context~1recheck/post")
            .is_some());
        assert!(document
            .pointer("/paths/~1api~1v1~1projects~1{id}~1repository-context~1link-fork/post")
            .is_some());
        assert!(document
            .pointer("/paths/~1api~1v1~1projects~1{id}~1repository-context~1create-fork/post")
            .is_some());
    }
}
