use chrono::Utc;
use neusym_core::ports::{IssueProvider, MappingStore};
use neusym_core::{
    IssueRef, Mapping, NormalizedIssue, Result, SyncAction, SyncDirection, SyncEvent,
};

pub struct SyncEngine {
    store: Box<dyn MappingStore>,
}

impl SyncEngine {
    pub fn new(store: Box<dyn MappingStore>) -> Self {
        Self { store }
    }

    pub async fn link(
        &self,
        source: &NormalizedIssue,
        target: &NormalizedIssue,
        direction: SyncDirection,
    ) -> Result<Mapping> {
        let mapping = Mapping {
            id: format!("{}:{}", source.identifier, target.identifier),
            source: IssueRef {
                provider: source.provider,
                project: String::new(),
                issue_id: source.id.clone(),
                identifier: source.identifier.clone(),
            },
            target: IssueRef {
                provider: target.provider,
                project: String::new(),
                issue_id: target.id.clone(),
                identifier: target.identifier.clone(),
            },
            direction,
            created_at: Utc::now(),
            last_synced: None,
        };
        self.store.add(mapping.clone()).await?;
        Ok(mapping)
    }

    pub async fn push(
        &self,
        mapping: &Mapping,
        source_provider: &dyn IssueProvider,
        target_provider: &dyn IssueProvider,
    ) -> Result<SyncEvent> {
        let source_issue = source_provider.get(&mapping.source.identifier).await?;
        let _updated = target_provider
            .update(&mapping.target.identifier, &source_issue)
            .await?;

        Ok(SyncEvent {
            mapping_id: mapping.id.clone(),
            timestamp: Utc::now(),
            action: SyncAction::Updated,
            fields_changed: vec![
                "title".to_string(),
                "description".to_string(),
                "status".to_string(),
            ],
        })
    }

    pub async fn status(&self) -> Result<Vec<Mapping>> {
        self.store.load().await
    }
}
