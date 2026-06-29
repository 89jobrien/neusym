use async_trait::async_trait;

use crate::{
    ConflictStrategy, Credential, HealthReport, Mapping, NormalizedIssue, Provider, Result,
    SyncDirection, SyncEvent,
};

#[async_trait]
pub trait IssueProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<NormalizedIssue>>;
    async fn get(&self, identifier: &str) -> Result<NormalizedIssue>;
    async fn create(&self, issue: &NormalizedIssue) -> Result<NormalizedIssue>;
    async fn update(&self, identifier: &str, issue: &NormalizedIssue) -> Result<NormalizedIssue>;
}

#[async_trait]
pub trait ProviderQuery: Send + Sync {
    async fn search(
        &self,
        provider: Provider,
        query: &str,
        creds: Option<Credential>,
    ) -> Result<Vec<NormalizedIssue>>;
    async fn get(
        &self,
        provider: Provider,
        identifier: &str,
        creds: Option<Credential>,
    ) -> Result<NormalizedIssue>;
}

#[async_trait]
pub trait SyncOperations: Send + Sync {
    async fn link(&self, source: &str, target: &str, direction: SyncDirection) -> Result<Mapping>;
    async fn push(&self, mapping_id: &str, strategy: ConflictStrategy) -> Result<SyncEvent>;
    async fn status(&self) -> Result<Vec<Mapping>>;
}

#[async_trait]
pub trait HealthCheck: Send + Sync {
    async fn health(&self) -> Result<HealthReport>;
}

#[async_trait]
pub trait CredentialResolver: Send + Sync {
    async fn resolve(&self, provider: Provider) -> Result<Credential>;
}

#[async_trait]
pub trait MappingStore: Send + Sync {
    async fn load(&self) -> Result<Vec<Mapping>>;
    async fn save(&self, mappings: &[Mapping]) -> Result<()>;
    async fn add(&self, mapping: Mapping) -> Result<()>;
    async fn find_by_identifier(&self, identifier: &str) -> Result<Option<Mapping>>;
}

#[async_trait]
pub trait OutputStore: Send + Sync {
    async fn append(&self, channel: &str, entry: &serde_json::Value) -> Result<()>;
    async fn overwrite(&self, channel: &str, data: &serde_json::Value) -> Result<()>;
}
