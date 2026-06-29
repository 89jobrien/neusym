use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use neusym_core::ports::{
    CredentialResolver, HealthCheck, IssueProvider, MappingStore, OutputStore, ProviderQuery,
    SyncOperations,
};
use neusym_core::{
    ConflictStrategy, Credential, FieldStrategy, HealthReport, IssueRef, Mapping, NeusymError,
    NormalizedIssue, Provider, ProviderHealth, Result, SyncAction, SyncDirection, SyncEvent,
};
use neusym_jira::JiraClient;
use neusym_linear::LinearClient;

pub struct NeusymService {
    resolver: Box<dyn CredentialResolver>,
    mapping_store: Box<dyn MappingStore>,
    output_store: Box<dyn OutputStore>,
}

impl NeusymService {
    pub fn new(
        resolver: Box<dyn CredentialResolver>,
        mapping_store: Box<dyn MappingStore>,
        output_store: Box<dyn OutputStore>,
    ) -> Self {
        Self {
            resolver,
            mapping_store,
            output_store,
        }
    }

    async fn resolve_creds(
        &self,
        provider: Provider,
        override_creds: Option<Credential>,
    ) -> Result<Credential> {
        if let Some(creds) = override_creds {
            return Ok(creds);
        }
        self.resolver.resolve(provider).await
    }

    fn make_provider(creds: &Credential) -> Box<dyn IssueProvider> {
        match creds {
            Credential::Linear { api_key } => Box::new(LinearClient::new(api_key.clone())),
            Credential::Jira {
                base_url,
                email,
                api_token,
            } => Box::new(JiraClient::new(
                base_url.clone(),
                email.clone(),
                api_token.clone(),
            )),
        }
    }

    async fn ping_provider(&self, provider: Provider) -> ProviderHealth {
        let start = Instant::now();
        match self.resolver.resolve(provider).await {
            Err(e) => ProviderHealth {
                provider,
                reachable: false,
                latency_ms: None,
                error: Some(e.to_string()),
            },
            Ok(creds) => {
                let client = Self::make_provider(&creds);
                match client.search("__ping__").await {
                    Ok(_) => ProviderHealth {
                        provider,
                        reachable: true,
                        latency_ms: Some(start.elapsed().as_millis() as u64),
                        error: None,
                    },
                    Err(e) => ProviderHealth {
                        provider,
                        reachable: false,
                        latency_ms: Some(start.elapsed().as_millis() as u64),
                        error: Some(e.to_string()),
                    },
                }
            }
        }
    }
}

#[async_trait]
impl ProviderQuery for NeusymService {
    async fn search(
        &self,
        provider: Provider,
        query: &str,
        creds: Option<Credential>,
    ) -> Result<Vec<NormalizedIssue>> {
        let resolved = self.resolve_creds(provider, creds).await?;
        let client = Self::make_provider(&resolved);
        let results = client.search(query).await?;
        self.output_store
            .append(
                "sync.log",
                &serde_json::json!({
                    "action": "search",
                    "provider": format!("{:?}", provider),
                    "query": query,
                    "results": results.len(),
                    "timestamp": Utc::now(),
                }),
            )
            .await?;
        Ok(results)
    }

    async fn get(
        &self,
        provider: Provider,
        identifier: &str,
        creds: Option<Credential>,
    ) -> Result<NormalizedIssue> {
        let resolved = self.resolve_creds(provider, creds).await?;
        let client = Self::make_provider(&resolved);
        client.get(identifier).await
    }
}

#[async_trait]
impl SyncOperations for NeusymService {
    async fn link(&self, source: &str, target: &str, direction: SyncDirection) -> Result<Mapping> {
        let source_creds = self.resolver.resolve(Provider::Linear).await?;
        let source_client = Self::make_provider(&source_creds);
        let source_issue = source_client.get(source).await?;

        let target_creds = self.resolver.resolve(Provider::Jira).await?;
        let target_client = Self::make_provider(&target_creds);
        let target_issue = target_client.get(target).await?;

        let mapping = Mapping {
            id: format!("{}:{}", source_issue.identifier, target_issue.identifier),
            source: IssueRef {
                provider: source_issue.provider,
                project: String::new(),
                issue_id: source_issue.id.clone(),
                identifier: source_issue.identifier.clone(),
            },
            target: IssueRef {
                provider: target_issue.provider,
                project: String::new(),
                issue_id: target_issue.id.clone(),
                identifier: target_issue.identifier.clone(),
            },
            direction,
            created_at: Utc::now(),
            last_synced: None,
        };
        self.mapping_store.add(mapping.clone()).await?;
        self.output_store
            .append(
                "sync.log",
                &serde_json::json!({
                    "action": "link",
                    "mapping_id": mapping.id,
                    "timestamp": Utc::now(),
                }),
            )
            .await?;
        Ok(mapping)
    }

    async fn push(&self, mapping_id: &str, strategy: ConflictStrategy) -> Result<SyncEvent> {
        let mapping = self
            .mapping_store
            .find_by_identifier(mapping_id)
            .await?
            .ok_or_else(|| NeusymError::MappingNotFound(mapping_id.to_string()))?;

        let source_creds = self.resolver.resolve(mapping.source.provider).await?;
        let source_client = Self::make_provider(&source_creds);
        let source_issue = source_client.get(&mapping.source.identifier).await?;

        let target_creds = self.resolver.resolve(mapping.target.provider).await?;
        let target_client = Self::make_provider(&target_creds);
        let target_issue = target_client.get(&mapping.target.identifier).await?;

        let event = match strategy {
            ConflictStrategy::ReportOnly => {
                let mut conflicts = vec![];
                if source_issue.title != target_issue.title {
                    conflicts.push("title".to_string());
                }
                if source_issue.description != target_issue.description {
                    conflicts.push("description".to_string());
                }
                if source_issue.status != target_issue.status {
                    conflicts.push("status".to_string());
                }
                if source_issue.priority != target_issue.priority {
                    conflicts.push("priority".to_string());
                }
                SyncEvent {
                    mapping_id: mapping.id.clone(),
                    timestamp: Utc::now(),
                    action: if conflicts.is_empty() {
                        SyncAction::Updated
                    } else {
                        SyncAction::Conflict {
                            field: conflicts.join(", "),
                            source: source_issue.identifier.clone(),
                            target: target_issue.identifier.clone(),
                        }
                    },
                    fields_changed: conflicts,
                }
            }
            ConflictStrategy::SourceWins => {
                target_client
                    .update(&mapping.target.identifier, &source_issue)
                    .await?;
                SyncEvent {
                    mapping_id: mapping.id.clone(),
                    timestamp: Utc::now(),
                    action: SyncAction::Updated,
                    fields_changed: vec!["title".into(), "description".into(), "status".into()],
                }
            }
            ConflictStrategy::TargetWins => {
                source_client
                    .update(&mapping.source.identifier, &target_issue)
                    .await?;
                SyncEvent {
                    mapping_id: mapping.id.clone(),
                    timestamp: Utc::now(),
                    action: SyncAction::Updated,
                    fields_changed: vec!["title".into(), "description".into(), "status".into()],
                }
            }
            ConflictStrategy::FieldLevel(ref resolutions) => {
                let mut merged = target_issue.clone();
                let mut changed = vec![];
                for res in resolutions {
                    match res.strategy {
                        FieldStrategy::SourceWins => {
                            apply_field(&mut merged, &res.field, &source_issue);
                            changed.push(res.field.clone());
                        }
                        FieldStrategy::TargetWins | FieldStrategy::Skip => {}
                    }
                }
                target_client
                    .update(&mapping.target.identifier, &merged)
                    .await?;
                SyncEvent {
                    mapping_id: mapping.id.clone(),
                    timestamp: Utc::now(),
                    action: SyncAction::Updated,
                    fields_changed: changed,
                }
            }
        };

        self.output_store
            .append(
                "sync.log",
                &serde_json::json!({
                    "action": "push",
                    "mapping_id": mapping.id,
                    "strategy": format!("{:?}", strategy),
                    "timestamp": Utc::now(),
                }),
            )
            .await?;

        Ok(event)
    }

    async fn status(&self) -> Result<Vec<Mapping>> {
        self.mapping_store.load().await
    }
}

#[async_trait]
impl HealthCheck for NeusymService {
    async fn health(&self) -> Result<HealthReport> {
        let linear = self.ping_provider(Provider::Linear).await;
        let jira = self.ping_provider(Provider::Jira).await;

        let mappings = self.mapping_store.load().await?;
        let stale_threshold = chrono::Duration::hours(24);
        let now = Utc::now();
        let stale = mappings
            .iter()
            .filter(|m| {
                m.last_synced
                    .map(|t| now - t > stale_threshold)
                    .unwrap_or(true)
            })
            .count();

        let report = HealthReport {
            providers: vec![linear, jira],
            mappings_total: mappings.len(),
            mappings_stale: stale,
            conflicts_pending: 0,
        };

        self.output_store
            .overwrite("health.json", &serde_json::to_value(&report)?)
            .await?;

        Ok(report)
    }
}

fn apply_field(target: &mut NormalizedIssue, field: &str, source: &NormalizedIssue) {
    match field {
        "title" => target.title = source.title.clone(),
        "description" => target.description = source.description.clone(),
        "status" => target.status = source.status.clone(),
        "priority" => target.priority = source.priority.clone(),
        "labels" => target.labels = source.labels.clone(),
        "assignee" => target.assignee = source.assignee.clone(),
        "parent_id" => target.parent_id = source.parent_id.clone(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use neusym_core::ports::{CredentialResolver, MappingStore, OutputStore};

    struct FakeResolver;

    #[async_trait]
    impl CredentialResolver for FakeResolver {
        async fn resolve(&self, provider: Provider) -> Result<Credential> {
            match provider {
                Provider::Linear => Ok(Credential::Linear {
                    api_key: "fake".to_string(),
                }),
                Provider::Jira => Ok(Credential::Jira {
                    base_url: "https://fake".to_string(),
                    email: "a@b.com".to_string(),
                    api_token: "tok".to_string(),
                }),
            }
        }
    }

    struct FakeMappingStore;

    #[async_trait]
    impl MappingStore for FakeMappingStore {
        async fn load(&self) -> Result<Vec<Mapping>> {
            Ok(vec![])
        }
        async fn save(&self, _mappings: &[Mapping]) -> Result<()> {
            Ok(())
        }
        async fn add(&self, _mapping: Mapping) -> Result<()> {
            Ok(())
        }
        async fn find_by_identifier(&self, _id: &str) -> Result<Option<Mapping>> {
            Ok(None)
        }
    }

    struct FakeOutputStore;

    #[async_trait]
    impl OutputStore for FakeOutputStore {
        async fn append(&self, _channel: &str, _entry: &serde_json::Value) -> Result<()> {
            Ok(())
        }
        async fn overwrite(&self, _channel: &str, _data: &serde_json::Value) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn service_status_returns_empty() {
        let svc = NeusymService::new(
            Box::new(FakeResolver),
            Box::new(FakeMappingStore),
            Box::new(FakeOutputStore),
        );
        let result = SyncOperations::status(&svc).await.unwrap();
        assert!(result.is_empty());
    }
}
