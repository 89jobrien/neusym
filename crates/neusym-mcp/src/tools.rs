use std::sync::Arc;

use obfsck::ObfuscationLevel;
use rmcp::model::RawContent;
use rmcp::model::*;
use rmcp::{ServerHandler, tool};
use schemars::JsonSchema;
use serde::Deserialize;

use neusym_core::ports::{HealthCheck, ProviderQuery, SyncOperations};
use neusym_core::{ConflictStrategy, Credential, Provider, SyncDirection};
use neusym_sync::NeusymService;

fn scrub_result(result: CallToolResult) -> CallToolResult {
    let content = result
        .content
        .into_iter()
        .map(|c| {
            if let RawContent::Text(ref t) = c.raw {
                let (scrubbed, _) = obfsck::obfuscate_text(&t.text, ObfuscationLevel::Standard);
                Content::text(scrubbed)
            } else {
                c
            }
        })
        .collect();
    CallToolResult {
        content,
        is_error: result.is_error,
    }
}

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
    pub source_provider: Provider,
    pub source: String,
    pub target_provider: Provider,
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

trait HasCredentialFields {
    fn linear_api_key(&self) -> Option<&String>;
    fn jira_base_url(&self) -> Option<&String>;
    fn jira_email(&self) -> Option<&String>;
    fn jira_api_token(&self) -> Option<&String>;
}

impl HasCredentialFields for SearchArgs {
    fn linear_api_key(&self) -> Option<&String> {
        self.linear_api_key.as_ref()
    }
    fn jira_base_url(&self) -> Option<&String> {
        self.jira_base_url.as_ref()
    }
    fn jira_email(&self) -> Option<&String> {
        self.jira_email.as_ref()
    }
    fn jira_api_token(&self) -> Option<&String> {
        self.jira_api_token.as_ref()
    }
}

impl HasCredentialFields for GetArgs {
    fn linear_api_key(&self) -> Option<&String> {
        self.linear_api_key.as_ref()
    }
    fn jira_base_url(&self) -> Option<&String> {
        self.jira_base_url.as_ref()
    }
    fn jira_email(&self) -> Option<&String> {
        self.jira_email.as_ref()
    }
    fn jira_api_token(&self) -> Option<&String> {
        self.jira_api_token.as_ref()
    }
}

fn extract_credential(provider: Provider, args: &dyn HasCredentialFields) -> Option<Credential> {
    match provider {
        Provider::Linear => args
            .linear_api_key()
            .map(|k| Credential::Linear { api_key: k.clone() }),
        Provider::Jira => {
            match (
                args.jira_base_url(),
                args.jira_email(),
                args.jira_api_token(),
            ) {
                (Some(u), Some(e), Some(t)) => Some(Credential::Jira {
                    base_url: u.clone(),
                    email: e.clone(),
                    api_token: t.clone(),
                }),
                _ => None,
            }
        }
    }
}

#[tool(tool_box)]
impl NeusymMcp {
    #[tool(description = "Search issues in a provider (Linear or Jira)")]
    async fn search(&self, #[tool(aggr)] args: SearchArgs) -> Result<CallToolResult, rmcp::Error> {
        let creds = extract_credential(args.provider, &args);
        let result = match self.service.search(args.provider, &args.query, creds).await {
            Ok(issues) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&issues).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
        };
        Ok(scrub_result(result))
    }

    #[tool(description = "Get a single issue by identifier")]
    async fn get(&self, #[tool(aggr)] args: GetArgs) -> Result<CallToolResult, rmcp::Error> {
        let creds = extract_credential(args.provider, &args);
        let result = match self
            .service
            .get(args.provider, &args.identifier, creds)
            .await
        {
            Ok(issue) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&issue).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
        };
        Ok(scrub_result(result))
    }

    #[tool(description = "Link two issues for bidirectional sync")]
    async fn sync_link(&self, #[tool(aggr)] args: LinkArgs) -> Result<CallToolResult, rmcp::Error> {
        let result = match self
            .service
            .link(
                args.source_provider,
                &args.source,
                args.target_provider,
                &args.target,
                args.direction,
            )
            .await
        {
            Ok(mapping) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&mapping).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
        };
        Ok(scrub_result(result))
    }

    #[tool(description = "Push changes from source to target provider")]
    async fn sync_push(&self, #[tool(aggr)] args: PushArgs) -> Result<CallToolResult, rmcp::Error> {
        let result = match self.service.push(&args.mapping_id, args.strategy).await {
            Ok(event) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&event).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
        };
        Ok(scrub_result(result))
    }

    #[tool(description = "Show all sync mappings and drift state")]
    async fn sync_status(&self) -> Result<CallToolResult, rmcp::Error> {
        let result = match self.service.status().await {
            Ok(mappings) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&mappings).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
        };
        Ok(scrub_result(result))
    }

    #[tool(description = "Health check for providers and sync mappings")]
    async fn sync_health(&self) -> Result<CallToolResult, rmcp::Error> {
        let result = match self.service.health().await {
            Ok(report) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&report).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
        };
        Ok(scrub_result(result))
    }
}

#[tool(tool_box)]
impl ServerHandler for NeusymMcp {}
