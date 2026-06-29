use async_trait::async_trait;
use std::sync::Mutex;

use crate::ports::IssueProvider;
use crate::{NeusymError, NormalizedIssue, Provider, Result};

pub struct InMemoryProvider {
    issues: Mutex<Vec<NormalizedIssue>>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        Self {
            issues: Mutex::new(vec![]),
        }
    }
}

impl Default for InMemoryProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IssueProvider for InMemoryProvider {
    async fn search(&self, query: &str) -> Result<Vec<NormalizedIssue>> {
        let issues = self.issues.lock().unwrap();
        Ok(issues
            .iter()
            .filter(|i| i.title.contains(query) || i.identifier.contains(query))
            .cloned()
            .collect())
    }

    async fn get(&self, identifier: &str) -> Result<NormalizedIssue> {
        let issues = self.issues.lock().unwrap();
        issues
            .iter()
            .find(|i| i.identifier == identifier)
            .cloned()
            .ok_or_else(|| NeusymError::MappingNotFound(identifier.to_string()))
    }

    async fn create(&self, issue: &NormalizedIssue) -> Result<NormalizedIssue> {
        let mut issues = self.issues.lock().unwrap();
        let created = NormalizedIssue {
            id: format!("id-{}", issues.len()),
            ..issue.clone()
        };
        issues.push(created.clone());
        Ok(created)
    }

    async fn update(&self, identifier: &str, issue: &NormalizedIssue) -> Result<NormalizedIssue> {
        let mut issues = self.issues.lock().unwrap();
        let pos = issues
            .iter()
            .position(|i| i.identifier == identifier)
            .ok_or_else(|| NeusymError::MappingNotFound(identifier.to_string()))?;
        let updated = NormalizedIssue {
            id: issues[pos].id.clone(),
            identifier: identifier.to_string(),
            ..issue.clone()
        };
        issues[pos] = updated.clone();
        Ok(updated)
    }
}

/// Conformance suite: verifies any IssueProvider impl satisfies
/// the trait contract.
pub async fn assert_issue_provider_contract(provider: &dyn IssueProvider) {
    let issue = NormalizedIssue {
        provider: Provider::Linear,
        id: String::new(),
        identifier: "TEST-1".to_string(),
        title: "Test issue".to_string(),
        description: Some("A description".to_string()),
        status: "Open".to_string(),
        priority: Some("High".to_string()),
        labels: vec!["bug".to_string()],
        assignee: Some("Alice".to_string()),
        parent_id: None,
        url: None,
    };

    // create -> get round-trip
    let created = provider.create(&issue).await.unwrap();
    assert_eq!(created.title, "Test issue");
    assert_eq!(created.identifier, "TEST-1");

    let fetched = provider.get("TEST-1").await.unwrap();
    assert_eq!(fetched.title, created.title);
    assert_eq!(fetched.description, created.description);

    // update -> get round-trip
    let mut modified = fetched.clone();
    modified.title = "Updated title".to_string();
    let updated = provider.update("TEST-1", &modified).await.unwrap();
    assert_eq!(updated.title, "Updated title");

    let re_fetched = provider.get("TEST-1").await.unwrap();
    assert_eq!(re_fetched.title, "Updated title");

    // search finds the issue
    let results = provider.search("Updated").await.unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|i| i.identifier == "TEST-1"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_satisfies_contract() {
        let provider = InMemoryProvider::new();
        assert_issue_provider_contract(&provider).await;
    }
}
