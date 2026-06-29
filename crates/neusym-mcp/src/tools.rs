use std::sync::Arc;

use rmcp::model::*;
use rmcp::{ServerHandler, tool};
use schemars::JsonSchema;
use serde::Deserialize;

use neusym_core::ports::{HealthCheck, ProviderQuery, SyncOperations};
use neusym_core::{ConflictStrategy, Credential, Provider, SyncDirection};
use neusym_sync::NeusymService;

#[derive(Clone)]
pub struct NeusymMcp {
    service: Arc<NeusymService>,
}

impl NeusymMcp {
    pub fn new(service: Arc<NeusymService>) -> Self {
        Self { service }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub provider: Provider,
    pub query: String,
    pub linear_api_key: Option<String>,
    pub jira_base_url: Option<String>,
    pub jira_email: Option<String>,
    pub jira_api_token: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    pub provider: Provider,
    pub identifier: String,
    pub linear_api_key: Option<String>,
    pub jira_base_url: Option<String>,
    pub jira_email: Option<String>,
    pub jira_api_token: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkArgs {
    pub source: String,
    pub target: String,
    #[serde(default = "default_direction")]
    pub direction: SyncDirection,
}

fn default_direction() -> SyncDirection {
    SyncDirection::Bidirectional
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PushArgs {
    pub mapping_id: String,
    #[serde(default = "default_strategy")]
    pub strategy: ConflictStrategy,
}

fn default_strategy() -> ConflictStrategy {
    ConflictStrategy::SourceWins
}

fn to_credential(provider: Provider, args: &SearchArgs) -> Option<Credential> {
    match provider {
        Provider::Linear => args
            .linear_api_key
            .as_ref()
            .map(|k| Credential::Linear { api_key: k.clone() }),
        Provider::Jira => match (&args.jira_base_url, &args.jira_email, &args.jira_api_token) {
            (Some(u), Some(e), Some(t)) => Some(Credential::Jira {
                base_url: u.clone(),
                email: e.clone(),
                api_token: t.clone(),
            }),
            _ => None,
        },
    }
}

fn get_to_credential(provider: Provider, args: &GetArgs) -> Option<Credential> {
    match provider {
        Provider::Linear => args
            .linear_api_key
            .as_ref()
            .map(|k| Credential::Linear { api_key: k.clone() }),
        Provider::Jira => match (&args.jira_base_url, &args.jira_email, &args.jira_api_token) {
            (Some(u), Some(e), Some(t)) => Some(Credential::Jira {
                base_url: u.clone(),
                email: e.clone(),
                api_token: t.clone(),
            }),
            _ => None,
        },
    }
}

#[tool(tool_box)]
impl NeusymMcp {
    #[tool(description = "Search issues in a provider (Linear or Jira)")]
    async fn search(&self, #[tool(aggr)] args: SearchArgs) -> Result<CallToolResult, rmcp::Error> {
        let creds = to_credential(args.provider, &args);
        match self.service.search(args.provider, &args.query, creds).await {
            Ok(issues) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&issues).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Get a single issue by identifier")]
    async fn get(&self, #[tool(aggr)] args: GetArgs) -> Result<CallToolResult, rmcp::Error> {
        let creds = get_to_credential(args.provider, &args);
        match self
            .service
            .get(args.provider, &args.identifier, creds)
            .await
        {
            Ok(issue) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&issue).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Link two issues for bidirectional sync")]
    async fn sync_link(&self, #[tool(aggr)] args: LinkArgs) -> Result<CallToolResult, rmcp::Error> {
        match self
            .service
            .link(&args.source, &args.target, args.direction)
            .await
        {
            Ok(mapping) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&mapping).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Push changes from source to target provider")]
    async fn sync_push(&self, #[tool(aggr)] args: PushArgs) -> Result<CallToolResult, rmcp::Error> {
        match self.service.push(&args.mapping_id, args.strategy).await {
            Ok(event) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&event).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Show all sync mappings and drift state")]
    async fn sync_status(&self) -> Result<CallToolResult, rmcp::Error> {
        match self.service.status().await {
            Ok(mappings) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&mappings).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Health check for providers and sync mappings")]
    async fn sync_health(&self) -> Result<CallToolResult, rmcp::Error> {
        match self.service.health().await {
            Ok(report) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&report).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for NeusymMcp {}
