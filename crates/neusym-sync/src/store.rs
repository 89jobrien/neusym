use async_trait::async_trait;
use std::path::{Path, PathBuf};

use neusym_core::ports::MappingStore;
use neusym_core::{Mapping, Result};
use tokio::fs;

/// JSON file-backed mapping store.
pub struct JsonMappingStore {
    path: PathBuf,
}

impl JsonMappingStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home)
            .join(".ctx")
            .join("neusym")
            .join("mappings.json")
    }
}

#[async_trait]
impl MappingStore for JsonMappingStore {
    async fn load(&self) -> Result<Vec<Mapping>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let data = fs::read_to_string(&self.path).await?;
        let mappings: Vec<Mapping> = serde_json::from_str(&data)?;
        Ok(mappings)
    }

    async fn save(&self, mappings: &[Mapping]) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_string_pretty(mappings)?;
        fs::write(&self.path, data).await?;
        Ok(())
    }

    async fn add(&self, mapping: Mapping) -> Result<()> {
        let mut mappings = self.load().await?;
        mappings.push(mapping);
        self.save(&mappings).await
    }

    async fn find_by_identifier(&self, identifier: &str) -> Result<Option<Mapping>> {
        let mappings = self.load().await?;
        Ok(mappings
            .into_iter()
            .find(|m| m.source.identifier == identifier || m.target.identifier == identifier))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use neusym_core::{IssueRef, Provider, SyncDirection};

    fn test_mapping() -> Mapping {
        Mapping {
            id: "test:1".to_string(),
            source: IssueRef {
                provider: Provider::Linear,
                project: "proj".to_string(),
                issue_id: "id1".to_string(),
                identifier: "JOB-1".to_string(),
            },
            target: IssueRef {
                provider: Provider::Jira,
                project: "proj".to_string(),
                issue_id: "id2".to_string(),
                identifier: "PROJ-1".to_string(),
            },
            direction: SyncDirection::Bidirectional,
            created_at: Utc::now(),
            last_synced: None,
        }
    }

    #[tokio::test]
    async fn json_store_add_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mappings.json");
        let store = JsonMappingStore::new(&path);
        let m = test_mapping();
        MappingStore::add(&store, m.clone()).await.unwrap();
        let loaded = MappingStore::load(&store).await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "test:1");
    }

    #[tokio::test]
    async fn json_store_find_by_identifier() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mappings.json");
        let store = JsonMappingStore::new(&path);
        let m = test_mapping();
        MappingStore::add(&store, m).await.unwrap();
        let found = MappingStore::find_by_identifier(&store, "JOB-1")
            .await
            .unwrap();
        assert!(found.is_some());
        let not_found = MappingStore::find_by_identifier(&store, "NOPE")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }
}
