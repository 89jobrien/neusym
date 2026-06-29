use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A tracked mapping between an issue in two providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapping {
    pub id: String,
    pub source: IssueRef,
    pub target: IssueRef,
    pub direction: SyncDirection,
    pub created_at: DateTime<Utc>,
    pub last_synced: Option<DateTime<Utc>>,
}

/// Reference to an issue in a specific provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueRef {
    pub provider: Provider,
    pub project: String,
    pub issue_id: String,
    pub identifier: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Jira,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    JiraToLinear,
    LinearToJira,
    Bidirectional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEvent {
    pub mapping_id: String,
    pub timestamp: DateTime<Utc>,
    pub action: SyncAction,
    pub fields_changed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    Created,
    Updated,
    InSync,
    Conflict {
        field: String,
        source: String,
        target: String,
    },
    Failed {
        reason: String,
    },
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Credential {
    Linear {
        api_key: String,
    },
    Jira {
        base_url: String,
        email: String,
        api_token: String,
    },
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credential::Linear { .. } => f
                .debug_struct("Linear")
                .field("api_key", &"[REDACTED]")
                .finish(),
            Credential::Jira {
                base_url, email, ..
            } => f
                .debug_struct("Jira")
                .field("base_url", base_url)
                .field("email", email)
                .field("api_token", &"[REDACTED]")
                .finish(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    SourceWins,
    TargetWins,
    ReportOnly,
    FieldLevel(Vec<FieldResolution>),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FieldResolution {
    pub field: String,
    pub strategy: FieldStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldStrategy {
    SourceWins,
    TargetWins,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HealthReport {
    pub providers: Vec<ProviderHealth>,
    pub mappings_total: usize,
    pub mappings_stale: usize,
    pub conflicts_pending: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProviderHealth {
    pub provider: Provider,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// Normalized issue representation used for cross-provider comparison.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NormalizedIssue {
    pub provider: Provider,
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<String>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub parent_id: Option<String>,
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_debug_redacts_linear_api_key() {
        let cred = Credential::Linear {
            api_key: "lin_api_supersecret123".to_string(),
        };
        let debug = format!("{:?}", cred);
        assert!(
            !debug.contains("supersecret"),
            "Debug output must not contain the raw API key: {debug}"
        );
        assert!(
            debug.contains("REDACTED"),
            "Debug output should show REDACTED: {debug}"
        );
    }

    #[test]
    fn credential_debug_redacts_jira_token() {
        let cred = Credential::Jira {
            base_url: "https://example.atlassian.net".to_string(),
            email: "a@b.com".to_string(),
            api_token: "jira_secret_token_456".to_string(),
        };
        let debug = format!("{:?}", cred);
        assert!(
            !debug.contains("jira_secret_token"),
            "Debug output must not contain the raw API token: {debug}"
        );
        assert!(
            debug.contains("REDACTED"),
            "Debug output should show REDACTED: {debug}"
        );
    }

    #[test]
    fn credential_linear_round_trips() {
        let cred = Credential::Linear {
            api_key: "test-key".to_string(),
        };
        let json = serde_json::to_string(&cred).unwrap();
        let back: Credential = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Credential::Linear { api_key } if api_key == "test-key"));
    }

    #[test]
    fn credential_jira_round_trips() {
        let cred = Credential::Jira {
            base_url: "https://example.atlassian.net".to_string(),
            email: "a@b.com".to_string(),
            api_token: "tok".to_string(),
        };
        let json = serde_json::to_string(&cred).unwrap();
        let back: Credential = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Credential::Jira { .. }));
    }

    #[test]
    fn conflict_strategy_default_serializes() {
        let s = ConflictStrategy::SourceWins;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#""source_wins""#);
    }

    #[test]
    fn conflict_strategy_field_level_round_trips() {
        let s = ConflictStrategy::FieldLevel(vec![
            FieldResolution {
                field: "title".to_string(),
                strategy: FieldStrategy::SourceWins,
            },
            FieldResolution {
                field: "status".to_string(),
                strategy: FieldStrategy::Skip,
            },
        ]);
        let json = serde_json::to_string(&s).unwrap();
        let back: ConflictStrategy = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ConflictStrategy::FieldLevel(v) if v.len() == 2));
    }

    #[test]
    fn sync_action_in_sync_serializes() {
        let action = SyncAction::InSync;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, r#""in_sync""#);
    }

    #[test]
    fn health_report_serializes() {
        let report = HealthReport {
            providers: vec![ProviderHealth {
                provider: Provider::Linear,
                reachable: true,
                latency_ms: Some(42),
                error: None,
            }],
            mappings_total: 5,
            mappings_stale: 1,
            conflicts_pending: 0,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"reachable\":true"));
    }
}
