use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Agent settings for router service and execution behavior
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentSettings {
    pub enable_router_service: bool,
    #[serde(default = "default_router_version")]
    pub router_version: String,
    #[serde(default)]
    pub router_filters: Vec<MessageFilter>,
    #[serde(default = "default_router_timeout")]
    pub router_timeout_ms: u64,
}

fn default_router_version() -> String {
    "1.0.66".to_string()
}

fn default_router_timeout() -> u64 {
    5000
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enable_router_service: false,
            router_version: default_router_version(),
            router_filters: Vec::new(),
            router_timeout_ms: default_router_timeout(),
        }
    }
}

/// Message filter for router configuration
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MessageFilter {
    pub path_pattern: String,
    pub action: FilterAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum FilterAction {
    Allow,
    Deny,
    Transform { rule: String },
}

/// Serialize filters to JSON for router env var
pub fn serialize_filters(filters: &[MessageFilter]) -> Option<String> {
    serde_json::to_string(filters).ok()
}

/// Default filters for common use cases
pub fn default_filters() -> Vec<MessageFilter> {
    vec![
        MessageFilter {
            path_pattern: "/tasks/*".to_string(),
            action: FilterAction::Allow,
        },
        MessageFilter {
            path_pattern: "/approvals/*".to_string(),
            action: FilterAction::Allow,
        },
        MessageFilter {
            path_pattern: "/internal/*".to_string(),
            action: FilterAction::Deny,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_agent_settings() {
        let settings = AgentSettings::default();
        assert!(!settings.enable_router_service);
        assert_eq!(settings.router_version, "1.0.66");
        assert_eq!(settings.router_timeout_ms, 5000);
    }

    #[test]
    fn test_serialize_filters() {
        let filters = vec![MessageFilter {
            path_pattern: "/tasks/*".to_string(),
            action: FilterAction::Allow,
        }];

        let json = serialize_filters(&filters).unwrap();
        assert!(json.contains("tasks"));
        assert!(json.contains("allow"));
    }

    #[test]
    fn test_filter_action_variants() {
        let allow = FilterAction::Allow;
        let deny = FilterAction::Deny;
        let transform = FilterAction::Transform {
            rule: "uppercase".to_string(),
        };

        let allow_json = serde_json::to_string(&allow).unwrap();
        assert_eq!(allow_json, r#""allow""#);

        let deny_json = serde_json::to_string(&deny).unwrap();
        assert_eq!(deny_json, r#""deny""#);

        let transform_json = serde_json::to_string(&transform).unwrap();
        assert_eq!(transform_json, r#"{"transform":{"rule":"uppercase"}}"#);
    }
}
