use axum::{
    async_trait,
    extract::{FromRequest, Request},
    Json,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::ApiError;

/// Validated JSON extractor that automatically validates incoming request bodies
#[allow(dead_code)]
pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?;

        data.validate().map_err(|e| {
            let error_messages: Vec<String> = e
                .field_errors()
                .iter()
                .map(|(field, errors)| {
                    let messages: Vec<String> = errors
                        .iter()
                        .filter_map(|err| err.message.as_ref().map(|m| m.to_string()))
                        .collect();
                    format!("{}: {}", field, messages.join(", "))
                })
                .collect();

            ApiError::Validation(error_messages.join("; "))
        })?;

        Ok(ValidatedJson(data))
    }
}
