use super::{AuthResponseDto, UserDto};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[aliases(
    ApiResponseUserDto = ApiResponse<UserDto>,
    ApiResponseVecUserDto = ApiResponse<Vec<UserDto>>,
    ApiResponseAuthResponseDto = ApiResponse<AuthResponseDto>,
)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub code: ResponseCode,
    pub message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub metadata: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorDetail>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            code: ResponseCode::Success,
            message: message.into(),
            data: Some(data),
            metadata: None,
            error: None,
        }
    }

    pub fn created(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            code: ResponseCode::Success, // Using 0000 for created as well for now
            message: message.into(),
            data: Some(data),
            metadata: None,
            error: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, ToSchema)]
pub enum ResponseCode {
    // -------------------
    // Success Codes
    // -------------------
    #[serde(rename = "0000")]
    Success,

    // -------------------
    // Client Errors (4xxx)
    // -------------------

    // General 400
    #[serde(rename = "4000")]
    BadRequest,
    #[serde(rename = "4001")]
    ValidationError,
    #[serde(rename = "4002")]
    MissingParameter,
    #[serde(rename = "4003")]
    InvalidFormat,

    // Auth 401
    #[serde(rename = "4010")]
    Unauthorized,
    #[serde(rename = "4011")]
    InvalidCredentials,
    #[serde(rename = "4012")]
    TokenExpired,
    #[serde(rename = "4013")]
    TokenInvalid,

    // Forbidden 403
    #[serde(rename = "4030")]
    Forbidden,
    #[serde(rename = "4031")]
    AccessDenied,
    #[serde(rename = "4032")]
    AccountSuspended,

    // Not Found 404
    #[serde(rename = "4040")]
    NotFound,
    #[serde(rename = "4041")]
    TaskNotFound,
    #[serde(rename = "4042")]
    ProjectNotFound,
    #[serde(rename = "4043")]
    UserNotFound,
    #[serde(rename = "4044")]
    RequirementNotFound,
    #[serde(rename = "4045")]
    SprintNotFound,
    #[serde(rename = "4046")]
    ResourceNotFound,

    // Conflict 409
    #[serde(rename = "4090")]
    Conflict,
    #[serde(rename = "4091")]
    ResourceAlreadyExists,
    #[serde(rename = "4092")]
    StateConflict, // e.g., transitioning to an invalid state

    // -------------------
    // Server Errors (5xxx)
    // -------------------
    #[serde(rename = "5000")]
    InternalError,

    #[serde(rename = "5001")]
    DatabaseError,

    #[serde(rename = "5002")]
    ExternalServiceError, // e.g. GitLab, MinIO

    #[serde(rename = "5003")]
    ServiceUnavailable,

    #[serde(rename = "5004")]
    NotImplemented,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorDetail {
    pub details: Option<String>,
    pub trace_id: Option<String>,
}
