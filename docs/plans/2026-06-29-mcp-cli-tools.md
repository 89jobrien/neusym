# Plan: MCP Tools, CLI Interface, and Provider Completion

## Goal

Complete Linear/Jira provider adapters and expose all operations as
MCP tools and CLI subcommands from a single `neusym` binary.

## Architecture

- Crates affected: `neusym-core`, `neusym-linear`, `neusym-jira`,
  `neusym-sync`, `neusym-mcp`
- New traits: `ProviderQuery`, `SyncOperations`, `HealthCheck`,
  `CredentialResolver`, `MappingStore` (trait), `OutputStore`
- New types: `Credential`, `ConflictStrategy`, `FieldResolution`,
  `FieldStrategy`, `HealthReport`, `ProviderHealth`
- Data flow: CLI/MCP -> NeusymService -> IssueProvider adapters

## Tech Stack

- Rust edition 2024
- New deps: `async-trait`, `clap` (derive)
- Existing: `rmcp`, `reqwest`, `serde`, `chrono`, `miette`,
  `thiserror`, `schemars`, `crux-types`

## Tasks

### Task 1: Add workspace dependencies

**Crate**: workspace root
**File(s)**: `Cargo.toml`, `crates/neusym-core/Cargo.toml`

1. Add to `[workspace.dependencies]`:

   ```toml
   async-trait = "0.1"
   clap = { version = "4", features = ["derive"] }
   ```

2. Add to `crates/neusym-core/Cargo.toml` `[dependencies]`:

   ```toml
   async-trait = { workspace = true }
   ```

3. Verify:

   ```
   cargo check --workspace   -> compiles
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "chore: add async-trait and clap workspace deps"`

---

### Task 2: Add new domain types to neusym-core

**Crate**: `neusym-core`
**File(s)**: `crates/neusym-core/src/types.rs`
**Run**: `cargo nextest run -p neusym-core`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

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
           let back: ConflictStrategy =
               serde_json::from_str(&json).unwrap();
           assert!(matches!(back, ConflictStrategy::FieldLevel(v) if v.len() == 2));
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
   ```

   Run: `cargo nextest run -p neusym-core`
   Expected: FAIL (types don't exist yet)

2. Add types to `crates/neusym-core/src/types.rs` (append after
   existing types):

   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
   #[serde(rename_all = "snake_case")]
   pub enum Credential {
       Linear { api_key: String },
       Jira {
           base_url: String,
           email: String,
           api_token: String,
       },
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
   ```

3. Verify:

   ```
   cargo nextest run -p neusym-core         -> all green
   cargo clippy -p neusym-core -- -D warnings -> zero warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(core): add Credential, ConflictStrategy, HealthReport types"`

---

### Task 3: Add MissingCredential error variant

**Crate**: `neusym-core`
**File(s)**: `crates/neusym-core/src/error.rs`
**Run**: `cargo nextest run -p neusym-core`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn missing_credential_displays_field() {
           let err = NeusymError::MissingCredential {
               field: "LINEAR_API_KEY".to_string(),
           };
           let msg = err.to_string();
           assert!(msg.contains("LINEAR_API_KEY"));
       }
   }
   ```

   Run: `cargo nextest run -p neusym-core -- missing_credential`
   Expected: FAIL

2. Add variant to `NeusymError` in `crates/neusym-core/src/error.rs`:

   ```rust
   #[error("missing credential: {field}")]
   #[diagnostic(
       code(neusym::missing_credential),
       help("set {field} via environment variable or pass per-call")
   )]
   MissingCredential { field: String },
   ```

3. Verify:

   ```
   cargo nextest run -p neusym-core         -> all green
   cargo clippy -p neusym-core -- -D warnings -> zero warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(core): add MissingCredential error variant"`

---

### Task 4: Add port traits to neusym-core

**Crate**: `neusym-core`
**File(s)**: `crates/neusym-core/src/ports.rs`, `crates/neusym-core/src/lib.rs`
**Run**: `cargo check -p neusym-core`

1. Replace contents of `crates/neusym-core/src/ports.rs`:

   ```rust
   use async_trait::async_trait;

   use crate::{
       ConflictStrategy, Credential, HealthReport, Mapping,
       NormalizedIssue, Provider, Result, SyncDirection, SyncEvent,
   };

   #[async_trait]
   pub trait IssueProvider: Send + Sync {
       async fn search(
           &self,
           query: &str,
       ) -> Result<Vec<NormalizedIssue>>;
       async fn get(
           &self,
           identifier: &str,
       ) -> Result<NormalizedIssue>;
       async fn create(
           &self,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue>;
       async fn update(
           &self,
           identifier: &str,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue>;
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
       async fn link(
           &self,
           source: &str,
           target: &str,
           direction: SyncDirection,
       ) -> Result<Mapping>;
       async fn push(
           &self,
           mapping_id: &str,
           strategy: ConflictStrategy,
       ) -> Result<SyncEvent>;
       async fn status(&self) -> Result<Vec<Mapping>>;
   }

   #[async_trait]
   pub trait HealthCheck: Send + Sync {
       async fn health(&self) -> Result<HealthReport>;
   }

   #[async_trait]
   pub trait CredentialResolver: Send + Sync {
       async fn resolve(
           &self,
           provider: Provider,
       ) -> Result<Credential>;
   }

   #[async_trait]
   pub trait MappingStore: Send + Sync {
       async fn load(&self) -> Result<Vec<Mapping>>;
       async fn save(&self, mappings: &[Mapping]) -> Result<()>;
       async fn add(&self, mapping: Mapping) -> Result<()>;
       async fn find_by_identifier(
           &self,
           identifier: &str,
       ) -> Result<Option<Mapping>>;
   }

   #[async_trait]
   pub trait OutputStore: Send + Sync {
       async fn append(
           &self,
           channel: &str,
           entry: &serde_json::Value,
       ) -> Result<()>;
       async fn overwrite(
           &self,
           channel: &str,
           data: &serde_json::Value,
       ) -> Result<()>;
   }
   ```

2. Verify:

   ```
   cargo check -p neusym-core              -> compiles
   cargo clippy -p neusym-core -- -D warnings -> zero warnings
   ```

3. Run: `git branch --show-current`
   Commit: `git commit -m "feat(core): add ISP port traits with async_trait"`

---

### Task 5: Update LinearClient to async_trait

**Crate**: `neusym-linear`
**File(s)**: `crates/neusym-linear/Cargo.toml`,
`crates/neusym-linear/src/client.rs`
**Run**: `cargo check -p neusym-linear`

1. Add `async-trait` to `crates/neusym-linear/Cargo.toml`:

   ```toml
   async-trait = { workspace = true }
   ```

2. Rewrite `IssueProvider` impl in `client.rs` to use
   `#[async_trait]` instead of manual `Pin<Box<...>>`:

   ```rust
   use async_trait::async_trait;

   #[async_trait]
   impl IssueProvider for LinearClient {
       async fn search(
           &self,
           query: &str,
       ) -> Result<Vec<NormalizedIssue>> {
           let gql = r#"query($filter: IssueFilter) {
               issues(filter: $filter, first: 50) {
                   nodes { id identifier title description
                       state { name } priority priorityLabel
                       labels { nodes { name } }
                       assignee { name } parent { identifier }
                       url }
               }
           }"#;
           let variables = serde_json::json!({
               "filter": { "title": { "contains": query } }
           });
           let data = self.graphql(gql, variables).await?;
           let nodes = &data["data"]["issues"]["nodes"];
           let issues = nodes
               .as_array()
               .unwrap_or(&vec![])
               .iter()
               .map(Self::parse_issue)
               .collect();
           Ok(issues)
       }

       async fn get(
           &self,
           identifier: &str,
       ) -> Result<NormalizedIssue> {
           let gql = r#"query($id: String!) {
               issue(id: $id) { id identifier title description
                   state { name } priority priorityLabel
                   labels { nodes { name } }
                   assignee { name } parent { identifier }
                   url }
           }"#;
           let variables = serde_json::json!({ "id": identifier });
           let data = self.graphql(gql, variables).await?;
           Ok(Self::parse_issue(&data["data"]["issue"]))
       }

       async fn create(
           &self,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue> {
           let gql = r#"mutation($input: IssueCreateInput!) {
               issueCreate(input: $input) {
                   success
                   issue { id identifier title description
                       state { name } priority priorityLabel
                       labels { nodes { name } }
                       assignee { name } parent { identifier }
                       url }
               }
           }"#;
           let mut input = serde_json::json!({
               "title": issue.title,
           });
           if let Some(ref desc) = issue.description {
               input["description"] = serde_json::json!(desc);
           }
           if let Some(ref priority) = issue.priority {
               input["priority"] = serde_json::json!(
                   Self::priority_to_int(priority)
               );
           }
           let variables = serde_json::json!({ "input": input });
           let data = self.graphql(gql, variables).await?;
           if data["data"]["issueCreate"]["success"]
               .as_bool()
               .unwrap_or(false)
           {
               Ok(Self::parse_issue(
                   &data["data"]["issueCreate"]["issue"],
               ))
           } else {
               Err(NeusymError::Provider(
                   "Linear issueCreate failed".to_string(),
               ))
           }
       }

       async fn update(
           &self,
           identifier: &str,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue> {
           let gql = r#"mutation($id: String!, $input: IssueUpdateInput!) {
               issueUpdate(id: $id, input: $input) {
                   success
                   issue { id identifier title description
                       state { name } priority priorityLabel
                       labels { nodes { name } }
                       assignee { name } parent { identifier }
                       url }
               }
           }"#;
           let mut input = serde_json::json!({
               "title": issue.title,
           });
           if let Some(ref desc) = issue.description {
               input["description"] = serde_json::json!(desc);
           }
           if let Some(ref priority) = issue.priority {
               input["priority"] = serde_json::json!(
                   Self::priority_to_int(priority)
               );
           }
           let variables = serde_json::json!({
               "id": identifier,
               "input": input,
           });
           let data = self.graphql(gql, variables).await?;
           if data["data"]["issueUpdate"]["success"]
               .as_bool()
               .unwrap_or(false)
           {
               Ok(Self::parse_issue(
                   &data["data"]["issueUpdate"]["issue"],
               ))
           } else {
               Err(NeusymError::Provider(
                   "Linear issueUpdate failed".to_string(),
               ))
           }
       }
   }
   ```

3. Add helper method to `LinearClient`:

   ```rust
   fn priority_to_int(label: &str) -> u8 {
       match label.to_lowercase().as_str() {
           "urgent" => 1,
           "high" => 2,
           "medium" => 3,
           "low" => 4,
           _ => 0, // no priority
       }
   }
   ```

4. Remove `use std::future::Future` and `use std::pin::Pin` imports.

5. Verify:

   ```
   cargo check -p neusym-linear             -> compiles
   cargo clippy -p neusym-linear -- -D warnings -> zero warnings
   ```

6. Run: `git branch --show-current`
   Commit: `git commit -m "feat(linear): implement create/update, migrate to async_trait"`

---

### Task 6: Update JiraClient to async_trait

**Crate**: `neusym-jira`
**File(s)**: `crates/neusym-jira/Cargo.toml`,
`crates/neusym-jira/src/client.rs`
**Run**: `cargo check -p neusym-jira`

1. Add `async-trait` to `crates/neusym-jira/Cargo.toml`:

   ```toml
   async-trait = { workspace = true }
   ```

2. Rewrite `IssueProvider` impl in `client.rs` to use
   `#[async_trait]`:

   ```rust
   use async_trait::async_trait;

   #[async_trait]
   impl IssueProvider for JiraClient {
       async fn search(
           &self,
           query: &str,
       ) -> Result<Vec<NormalizedIssue>> {
           let jql = format!(
               "summary ~ \"{}\" ORDER BY updated DESC",
               query
           );
           let url = format!(
               "{}/rest/api/3/search",
               self.base_url
           );
           let resp = self
               .client
               .get(&url)
               .basic_auth(&self.email, Some(&self.api_token))
               .query(&[
                   ("jql", &jql),
                   ("maxResults", &"50".to_string()),
               ])
               .send()
               .await
               .map_err(|e| NeusymError::Http(e.to_string()))?;

           let json: serde_json::Value = resp
               .json()
               .await
               .map_err(|e| NeusymError::Http(e.to_string()))?;

           let issues = json["issues"]
               .as_array()
               .unwrap_or(&vec![])
               .iter()
               .map(|i| self.parse_issue(i))
               .collect();
           Ok(issues)
       }

       async fn get(
           &self,
           identifier: &str,
       ) -> Result<NormalizedIssue> {
           let url = format!(
               "{}/rest/api/3/issue/{}",
               self.base_url, identifier
           );
           let resp = self
               .client
               .get(&url)
               .basic_auth(&self.email, Some(&self.api_token))
               .send()
               .await
               .map_err(|e| NeusymError::Http(e.to_string()))?;

           let i: serde_json::Value = resp
               .json()
               .await
               .map_err(|e| NeusymError::Http(e.to_string()))?;

           Ok(self.parse_issue(&i))
       }

       async fn create(
           &self,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue> {
           let url = format!(
               "{}/rest/api/3/issue",
               self.base_url
           );
           let mut fields = serde_json::json!({
               "summary": issue.title,
               "issuetype": { "name": "Task" },
           });
           if let Some(ref desc) = issue.description {
               fields["description"] = serde_json::json!({
                   "type": "doc",
                   "version": 1,
                   "content": [{
                       "type": "paragraph",
                       "content": [{
                           "type": "text",
                           "text": desc,
                       }]
                   }]
               });
           }
           if !issue.labels.is_empty() {
               fields["labels"] = serde_json::json!(issue.labels);
           }
           let body = serde_json::json!({ "fields": fields });
           let resp = self
               .client
               .post(&url)
               .basic_auth(&self.email, Some(&self.api_token))
               .json(&body)
               .send()
               .await
               .map_err(|e| NeusymError::Http(e.to_string()))?;

           let created: serde_json::Value = resp
               .json()
               .await
               .map_err(|e| NeusymError::Http(e.to_string()))?;

           let key = created["key"]
               .as_str()
               .ok_or_else(|| {
                   NeusymError::Provider(
                       "Jira create: no key in response".to_string(),
                   )
               })?;
           self.get(key).await
       }

       async fn update(
           &self,
           identifier: &str,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue> {
           let url = format!(
               "{}/rest/api/3/issue/{}",
               self.base_url, identifier
           );
           let mut fields = serde_json::json!({
               "summary": issue.title,
           });
           if let Some(ref desc) = issue.description {
               fields["description"] = serde_json::json!({
                   "type": "doc",
                   "version": 1,
                   "content": [{
                       "type": "paragraph",
                       "content": [{
                           "type": "text",
                           "text": desc,
                       }]
                   }]
               });
           }
           if !issue.labels.is_empty() {
               fields["labels"] = serde_json::json!(issue.labels);
           }
           let body = serde_json::json!({ "fields": fields });
           self.client
               .put(&url)
               .basic_auth(&self.email, Some(&self.api_token))
               .json(&body)
               .send()
               .await
               .map_err(|e| NeusymError::Http(e.to_string()))?;

           self.get(identifier).await
       }
   }
   ```

3. Remove `use std::future::Future` and `use std::pin::Pin` imports.

4. Verify:

   ```
   cargo check -p neusym-jira               -> compiles
   cargo clippy -p neusym-jira -- -D warnings -> zero warnings
   ```

5. Run: `git branch --show-current`
   Commit: `git commit -m "feat(jira): implement create/update, migrate to async_trait"`

---

### Task 7: Rename MappingStore struct, add adapter impls

**Crate**: `neusym-sync`
**File(s)**: `crates/neusym-sync/Cargo.toml`,
`crates/neusym-sync/src/store.rs`,
`crates/neusym-sync/src/credentials.rs`,
`crates/neusym-sync/src/output.rs`,
`crates/neusym-sync/src/lib.rs`
**Run**: `cargo nextest run -p neusym-sync`

1. Add deps to `crates/neusym-sync/Cargo.toml`:

   ```toml
   async-trait = { workspace = true }
   neusym-linear = { workspace = true }
   neusym-jira = { workspace = true }
   ```

2. Write failing tests in `crates/neusym-sync/src/store.rs`:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use neusym_core::{IssueRef, Provider, SyncDirection};
       use chrono::Utc;

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
           let found = MappingStore::find_by_identifier(
               &store, "JOB-1",
           )
           .await
           .unwrap();
           assert!(found.is_some());
           let not_found = MappingStore::find_by_identifier(
               &store, "NOPE",
           )
           .await
           .unwrap();
           assert!(not_found.is_none());
       }
   }
   ```

   Run: `cargo nextest run -p neusym-sync`
   Expected: FAIL

3. Rename `MappingStore` struct to `JsonMappingStore`, implement
   the `MappingStore` trait:

   ```rust
   use async_trait::async_trait;
   use std::path::{Path, PathBuf};

   use neusym_core::{Mapping, Result};
   use neusym_core::ports::MappingStore;

   pub struct JsonMappingStore {
       path: PathBuf,
   }

   impl JsonMappingStore {
       pub fn new(path: impl Into<PathBuf>) -> Self {
           Self { path: path.into() }
       }

       pub fn default_path() -> PathBuf {
           let home = std::env::var("HOME")
               .unwrap_or_else(|_| ".".to_string());
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
           let data = std::fs::read_to_string(&self.path)?;
           let mappings: Vec<Mapping> =
               serde_json::from_str(&data)?;
           Ok(mappings)
       }

       async fn save(
           &self,
           mappings: &[Mapping],
       ) -> Result<()> {
           if let Some(parent) = self.path.parent() {
               std::fs::create_dir_all(parent)?;
           }
           let data = serde_json::to_string_pretty(mappings)?;
           std::fs::write(&self.path, data)?;
           Ok(())
       }

       async fn add(&self, mapping: Mapping) -> Result<()> {
           let mut mappings = self.load().await?;
           mappings.push(mapping);
           self.save(&mappings).await
       }

       async fn find_by_identifier(
           &self,
           identifier: &str,
       ) -> Result<Option<Mapping>> {
           let mappings = self.load().await?;
           Ok(mappings.into_iter().find(|m| {
               m.source.identifier == identifier
                   || m.target.identifier == identifier
           }))
       }
   }
   ```

4. Create `crates/neusym-sync/src/credentials.rs`:

   ```rust
   use async_trait::async_trait;
   use neusym_core::{
       Credential, NeusymError, Provider, Result,
   };
   use neusym_core::ports::CredentialResolver;

   pub struct EnvCredentialResolver;

   #[async_trait]
   impl CredentialResolver for EnvCredentialResolver {
       async fn resolve(
           &self,
           provider: Provider,
       ) -> Result<Credential> {
           match provider {
               Provider::Linear => {
                   let api_key = std::env::var("LINEAR_API_KEY")
                       .map_err(|_| NeusymError::MissingCredential {
                           field: "LINEAR_API_KEY".to_string(),
                       })?;
                   Ok(Credential::Linear { api_key })
               }
               Provider::Jira => {
                   let base_url = std::env::var("JIRA_BASE_URL")
                       .map_err(|_| NeusymError::MissingCredential {
                           field: "JIRA_BASE_URL".to_string(),
                       })?;
                   let email = std::env::var("JIRA_EMAIL")
                       .map_err(|_| NeusymError::MissingCredential {
                           field: "JIRA_EMAIL".to_string(),
                       })?;
                   let api_token = std::env::var("JIRA_API_TOKEN")
                       .map_err(|_| NeusymError::MissingCredential {
                           field: "JIRA_API_TOKEN".to_string(),
                       })?;
                   Ok(Credential::Jira {
                       base_url,
                       email,
                       api_token,
                   })
               }
           }
       }
   }
   ```

5. Create `crates/neusym-sync/src/output.rs`:

   ```rust
   use async_trait::async_trait;
   use std::fs::OpenOptions;
   use std::io::Write;
   use std::path::{Path, PathBuf};

   use neusym_core::Result;
   use neusym_core::ports::OutputStore;

   pub struct FileOutputStore {
       ctx_dir: PathBuf,
   }

   impl FileOutputStore {
       pub fn new(ctx_dir: impl Into<PathBuf>) -> Self {
           Self {
               ctx_dir: ctx_dir.into(),
           }
       }

       pub fn default_path() -> PathBuf {
           let home = std::env::var("HOME")
               .unwrap_or_else(|_| ".".to_string());
           Path::new(&home).join(".ctx").join("neusym")
       }

       fn channel_path(&self, channel: &str) -> PathBuf {
           self.ctx_dir.join(channel)
       }
   }

   #[async_trait]
   impl OutputStore for FileOutputStore {
       async fn append(
           &self,
           channel: &str,
           entry: &serde_json::Value,
       ) -> Result<()> {
           let path = self.channel_path(channel);
           if let Some(parent) = path.parent() {
               std::fs::create_dir_all(parent)?;
           }
           let mut file = OpenOptions::new()
               .create(true)
               .append(true)
               .open(&path)?;
           let line = serde_json::to_string(entry)?;
           writeln!(file, "{}", line)?;
           Ok(())
       }

       async fn overwrite(
           &self,
           channel: &str,
           data: &serde_json::Value,
       ) -> Result<()> {
           let path = self.channel_path(channel);
           if let Some(parent) = path.parent() {
               std::fs::create_dir_all(parent)?;
           }
           let content = serde_json::to_string_pretty(data)?;
           std::fs::write(&path, content)?;
           Ok(())
       }
   }
   ```

6. Update `crates/neusym-sync/src/lib.rs`:

   ```rust
   mod credentials;
   mod engine;
   mod output;
   mod service;
   mod store;

   pub use credentials::EnvCredentialResolver;
   pub use engine::SyncEngine;
   pub use output::FileOutputStore;
   pub use service::NeusymService;
   pub use store::JsonMappingStore;
   ```

7. Add `tempfile` dev-dependency to `crates/neusym-sync/Cargo.toml`:

   ```toml
   [dev-dependencies]
   tempfile = "3"
   ```

8. Verify:

   ```
   cargo nextest run -p neusym-sync         -> all green
   cargo clippy -p neusym-sync -- -D warnings -> zero warnings
   ```

9. Run: `git branch --show-current`
   Commit: `git commit -m "feat(sync): add JsonMappingStore, EnvCredentialResolver, FileOutputStore adapters"`

---

### Task 8: Implement NeusymService

**Crate**: `neusym-sync`
**File(s)**: `crates/neusym-sync/src/service.rs`
**Run**: `cargo nextest run -p neusym-sync`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use async_trait::async_trait;
       use neusym_core::{
           Credential, Mapping, NormalizedIssue, Provider, Result,
       };
       use neusym_core::ports::{
           CredentialResolver, MappingStore, OutputStore,
       };

       struct FakeResolver;

       #[async_trait]
       impl CredentialResolver for FakeResolver {
           async fn resolve(
               &self,
               provider: Provider,
           ) -> Result<Credential> {
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
           async fn save(
               &self,
               _mappings: &[Mapping],
           ) -> Result<()> {
               Ok(())
           }
           async fn add(&self, _mapping: Mapping) -> Result<()> {
               Ok(())
           }
           async fn find_by_identifier(
               &self,
               _id: &str,
           ) -> Result<Option<Mapping>> {
               Ok(None)
           }
       }

       struct FakeOutputStore;

       #[async_trait]
       impl OutputStore for FakeOutputStore {
           async fn append(
               &self,
               _channel: &str,
               _entry: &serde_json::Value,
           ) -> Result<()> {
               Ok(())
           }
           async fn overwrite(
               &self,
               _channel: &str,
               _data: &serde_json::Value,
           ) -> Result<()> {
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
   ```

   Run: `cargo nextest run -p neusym-sync -- service_status`
   Expected: FAIL

2. Create `crates/neusym-sync/src/service.rs`:

   ```rust
   use std::time::Instant;

   use async_trait::async_trait;
   use chrono::Utc;
   use neusym_core::{
       ConflictStrategy, Credential, HealthReport, IssueRef,
       Mapping, NeusymError, NormalizedIssue, Provider,
       ProviderHealth, Result, SyncAction, SyncDirection,
       SyncEvent,
   };
   use neusym_core::ports::{
       CredentialResolver, HealthCheck, IssueProvider,
       MappingStore, OutputStore, ProviderQuery, SyncOperations,
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

       fn make_provider(
           &self,
           creds: &Credential,
       ) -> Box<dyn IssueProvider> {
           match creds {
               Credential::Linear { api_key } => {
                   Box::new(LinearClient::new(api_key.clone()))
               }
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

       fn provider_for_credential(
           creds: &Credential,
       ) -> Provider {
           match creds {
               Credential::Linear { .. } => Provider::Linear,
               Credential::Jira { .. } => Provider::Jira,
           }
       }

       async fn ping_provider(
           &self,
           provider: Provider,
       ) -> ProviderHealth {
           let start = Instant::now();
           match self.resolver.resolve(provider).await {
               Err(e) => ProviderHealth {
                   provider,
                   reachable: false,
                   latency_ms: None,
                   error: Some(e.to_string()),
               },
               Ok(creds) => {
                   let client = self.make_provider(&creds);
                   match client.search("__ping__").await {
                       Ok(_) => ProviderHealth {
                           provider,
                           reachable: true,
                           latency_ms: Some(
                               start.elapsed().as_millis() as u64,
                           ),
                           error: None,
                       },
                       Err(e) => ProviderHealth {
                           provider,
                           reachable: false,
                           latency_ms: Some(
                               start.elapsed().as_millis() as u64,
                           ),
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
           let resolved = self
               .resolve_creds(provider, creds)
               .await?;
           let client = self.make_provider(&resolved);
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
           let resolved = self
               .resolve_creds(provider, creds)
               .await?;
           let client = self.make_provider(&resolved);
           client.get(identifier).await
       }
   }

   #[async_trait]
   impl SyncOperations for NeusymService {
       async fn link(
           &self,
           source: &str,
           target: &str,
           direction: SyncDirection,
       ) -> Result<Mapping> {
           let source_creds = self
               .resolver
               .resolve(Provider::Linear)
               .await?;
           let source_client = self.make_provider(&source_creds);
           let source_issue = source_client.get(source).await?;

           let target_creds = self
               .resolver
               .resolve(Provider::Jira)
               .await?;
           let target_client = self.make_provider(&target_creds);
           let target_issue = target_client.get(target).await?;

           let mapping = Mapping {
               id: format!(
                   "{}:{}",
                   source_issue.identifier,
                   target_issue.identifier
               ),
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

       async fn push(
           &self,
           mapping_id: &str,
           strategy: ConflictStrategy,
       ) -> Result<SyncEvent> {
           let mapping = self
               .mapping_store
               .find_by_identifier(mapping_id)
               .await?
               .ok_or_else(|| {
                   NeusymError::MappingNotFound(
                       mapping_id.to_string(),
                   )
               })?;

           let source_creds = self
               .resolver
               .resolve(mapping.source.provider)
               .await?;
           let source_client =
               self.make_provider(&source_creds);
           let source_issue = source_client
               .get(&mapping.source.identifier)
               .await?;

           let target_creds = self
               .resolver
               .resolve(mapping.target.provider)
               .await?;
           let target_client =
               self.make_provider(&target_creds);
           let target_issue = target_client
               .get(&mapping.target.identifier)
               .await?;

           let event = match strategy {
               ConflictStrategy::ReportOnly => {
                   let mut conflicts = vec![];
                   if source_issue.title != target_issue.title {
                       conflicts.push("title".to_string());
                   }
                   if source_issue.description
                       != target_issue.description
                   {
                       conflicts.push("description".to_string());
                   }
                   if source_issue.status != target_issue.status {
                       conflicts.push("status".to_string());
                   }
                   if source_issue.priority
                       != target_issue.priority
                   {
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
                               source: source_issue
                                   .identifier
                                   .clone(),
                               target: target_issue
                                   .identifier
                                   .clone(),
                           }
                       },
                       fields_changed: conflicts,
                   }
               }
               ConflictStrategy::SourceWins => {
                   target_client
                       .update(
                           &mapping.target.identifier,
                           &source_issue,
                       )
                       .await?;
                   SyncEvent {
                       mapping_id: mapping.id.clone(),
                       timestamp: Utc::now(),
                       action: SyncAction::Updated,
                       fields_changed: vec![
                           "title".into(),
                           "description".into(),
                           "status".into(),
                       ],
                   }
               }
               ConflictStrategy::TargetWins => {
                   source_client
                       .update(
                           &mapping.source.identifier,
                           &target_issue,
                       )
                       .await?;
                   SyncEvent {
                       mapping_id: mapping.id.clone(),
                       timestamp: Utc::now(),
                       action: SyncAction::Updated,
                       fields_changed: vec![
                           "title".into(),
                           "description".into(),
                           "status".into(),
                       ],
                   }
               }
               ConflictStrategy::FieldLevel(ref resolutions) => {
                   let mut merged = target_issue.clone();
                   let mut changed = vec![];
                   for res in resolutions {
                       match res.strategy {
                           neusym_core::FieldStrategy::SourceWins => {
                               apply_field(
                                   &mut merged,
                                   &res.field,
                                   &source_issue,
                               );
                               changed.push(
                                   res.field.clone(),
                               );
                           }
                           neusym_core::FieldStrategy::TargetWins => {
                               // already target value
                           }
                           neusym_core::FieldStrategy::Skip => {}
                       }
                   }
                   target_client
                       .update(
                           &mapping.target.identifier,
                           &merged,
                       )
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
           let linear = self
               .ping_provider(Provider::Linear)
               .await;
           let jira = self
               .ping_provider(Provider::Jira)
               .await;

           let mappings = self.mapping_store.load().await?;
           let stale_threshold =
               chrono::Duration::hours(24);
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
               .overwrite(
                   "health.json",
                   &serde_json::to_value(&report)?,
               )
               .await?;

           Ok(report)
       }
   }

   fn apply_field(
       target: &mut NormalizedIssue,
       field: &str,
       source: &NormalizedIssue,
   ) {
       match field {
           "title" => target.title = source.title.clone(),
           "description" => {
               target.description = source.description.clone()
           }
           "status" => target.status = source.status.clone(),
           "priority" => {
               target.priority = source.priority.clone()
           }
           "labels" => target.labels = source.labels.clone(),
           "assignee" => {
               target.assignee = source.assignee.clone()
           }
           "parent_id" => {
               target.parent_id = source.parent_id.clone()
           }
           _ => {}
       }
   }
   ```

3. Verify:

   ```
   cargo nextest run -p neusym-sync         -> all green
   cargo clippy -p neusym-sync -- -D warnings -> zero warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(sync): implement NeusymService with ProviderQuery, SyncOperations, HealthCheck"`

---

### Task 9: Update SyncEngine to use trait-based MappingStore

**Crate**: `neusym-sync`
**File(s)**: `crates/neusym-sync/src/engine.rs`
**Run**: `cargo check -p neusym-sync`

1. Update `SyncEngine` to accept `Box<dyn MappingStore>` instead
   of the concrete struct. This is now a secondary API -- the
   primary path is through `NeusymService`. `SyncEngine` remains
   for backward compat but delegates to the trait:

   ```rust
   use async_trait::async_trait;
   use chrono::Utc;
   use neusym_core::{
       IssueRef, Mapping, NormalizedIssue, Result, SyncAction,
       SyncDirection, SyncEvent,
   };
   use neusym_core::ports::{IssueProvider, MappingStore};

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
               id: format!(
                   "{}:{}",
                   source.identifier, target.identifier
               ),
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
           let source_issue = source_provider
               .get(&mapping.source.identifier)
               .await?;
           let _updated = target_provider
               .update(
                   &mapping.target.identifier,
                   &source_issue,
               )
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
   ```

2. Verify:

   ```
   cargo check -p neusym-sync               -> compiles
   cargo clippy -p neusym-sync -- -D warnings -> zero warnings
   ```

3. Run: `git branch --show-current`
   Commit: `git commit -m "refactor(sync): update SyncEngine to use MappingStore trait"`

---

### Task 10: Wire MCP server with tool handlers

**Crate**: `neusym-mcp`
**File(s)**: `crates/neusym-mcp/Cargo.toml`,
`crates/neusym-mcp/src/main.rs`,
`crates/neusym-mcp/src/tools.rs`
**Run**: `cargo check -p neusym-mcp`

1. Add deps to `crates/neusym-mcp/Cargo.toml`:

   ```toml
   clap = { workspace = true }
   async-trait = { workspace = true }
   ```

2. Create `crates/neusym-mcp/src/tools.rs` with rmcp tool
   definitions. Each MCP tool maps to a `NeusymService` method:

   ```rust
   use std::sync::Arc;

   use rmcp::{ServerHandler, model::*, tool};
   use schemars::JsonSchema;
   use serde::Deserialize;
   use neusym_core::{
       ConflictStrategy, Credential, Provider, SyncDirection,
   };
   use neusym_core::ports::{
       HealthCheck, ProviderQuery, SyncOperations,
   };
   use neusym_sync::NeusymService;

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

   impl SearchArgs {
       fn to_credential(&self) -> Option<Credential> {
           match self.provider {
               Provider::Linear => self
                   .linear_api_key
                   .as_ref()
                   .map(|k| Credential::Linear {
                       api_key: k.clone(),
                   }),
               Provider::Jira => {
                   match (
                       &self.jira_base_url,
                       &self.jira_email,
                       &self.jira_api_token,
                   ) {
                       (Some(u), Some(e), Some(t)) => {
                           Some(Credential::Jira {
                               base_url: u.clone(),
                               email: e.clone(),
                               api_token: t.clone(),
                           })
                       }
                       _ => None,
                   }
               }
           }
       }
   }

   impl GetArgs {
       fn to_credential(&self) -> Option<Credential> {
           match self.provider {
               Provider::Linear => self
                   .linear_api_key
                   .as_ref()
                   .map(|k| Credential::Linear {
                       api_key: k.clone(),
                   }),
               Provider::Jira => {
                   match (
                       &self.jira_base_url,
                       &self.jira_email,
                       &self.jira_api_token,
                   ) {
                       (Some(u), Some(e), Some(t)) => {
                           Some(Credential::Jira {
                               base_url: u.clone(),
                               email: e.clone(),
                               api_token: t.clone(),
                           })
                       }
                       _ => None,
                   }
               }
           }
       }
   }

   #[tool(tool_box)]
   impl NeusymMcp {
       #[tool(
           description = "Search issues in a provider"
       )]
       async fn search(
           &self,
           #[tool(aggr)] args: SearchArgs,
       ) -> Result<CallToolResult, rmcp::Error> {
           let creds = args.to_credential();
           match self
               .service
               .search(args.provider, &args.query, creds)
               .await
           {
               Ok(issues) => Ok(CallToolResult::success(vec![
                   Content::text(
                       serde_json::to_string_pretty(&issues)
                           .unwrap_or_default(),
                   ),
               ])),
               Err(e) => Ok(CallToolResult::error(vec![
                   Content::text(e.to_string()),
               ])),
           }
       }

       #[tool(description = "Get a single issue by identifier")]
       async fn get(
           &self,
           #[tool(aggr)] args: GetArgs,
       ) -> Result<CallToolResult, rmcp::Error> {
           let creds = args.to_credential();
           match self
               .service
               .get(args.provider, &args.identifier, creds)
               .await
           {
               Ok(issue) => Ok(CallToolResult::success(vec![
                   Content::text(
                       serde_json::to_string_pretty(&issue)
                           .unwrap_or_default(),
                   ),
               ])),
               Err(e) => Ok(CallToolResult::error(vec![
                   Content::text(e.to_string()),
               ])),
           }
       }

       #[tool(
           description = "Link two issues for bidirectional sync"
       )]
       async fn sync_link(
           &self,
           #[tool(aggr)] args: LinkArgs,
       ) -> Result<CallToolResult, rmcp::Error> {
           match self
               .service
               .link(&args.source, &args.target, args.direction)
               .await
           {
               Ok(mapping) => Ok(CallToolResult::success(vec![
                   Content::text(
                       serde_json::to_string_pretty(&mapping)
                           .unwrap_or_default(),
                   ),
               ])),
               Err(e) => Ok(CallToolResult::error(vec![
                   Content::text(e.to_string()),
               ])),
           }
       }

       #[tool(
           description = "Push changes from source to target"
       )]
       async fn sync_push(
           &self,
           #[tool(aggr)] args: PushArgs,
       ) -> Result<CallToolResult, rmcp::Error> {
           match self
               .service
               .push(&args.mapping_id, args.strategy)
               .await
           {
               Ok(event) => Ok(CallToolResult::success(vec![
                   Content::text(
                       serde_json::to_string_pretty(&event)
                           .unwrap_or_default(),
                   ),
               ])),
               Err(e) => Ok(CallToolResult::error(vec![
                   Content::text(e.to_string()),
               ])),
           }
       }

       #[tool(description = "Show all sync mappings and state")]
       async fn sync_status(
           &self,
       ) -> Result<CallToolResult, rmcp::Error> {
           match self.service.status().await {
               Ok(mappings) => Ok(CallToolResult::success(vec![
                   Content::text(
                       serde_json::to_string_pretty(&mappings)
                           .unwrap_or_default(),
                   ),
               ])),
               Err(e) => Ok(CallToolResult::error(vec![
                   Content::text(e.to_string()),
               ])),
           }
       }

       #[tool(
           description = "Health check for providers and mappings"
       )]
       async fn sync_health(
           &self,
       ) -> Result<CallToolResult, rmcp::Error> {
           match self.service.health().await {
               Ok(report) => Ok(CallToolResult::success(vec![
                   Content::text(
                       serde_json::to_string_pretty(&report)
                           .unwrap_or_default(),
                   ),
               ])),
               Err(e) => Ok(CallToolResult::error(vec![
                   Content::text(e.to_string()),
               ])),
           }
       }
   }

   #[tool(tool_box)]
   impl ServerHandler for NeusymMcp {}
   ```

3. Verify:

   ```
   cargo check -p neusym-mcp               -> compiles
   cargo clippy -p neusym-mcp -- -D warnings -> zero warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(mcp): wire MCP tool handlers to NeusymService"`

---

### Task 11: Add CLI subcommands

**Crate**: `neusym-mcp`
**File(s)**: `crates/neusym-mcp/src/cli.rs`,
`crates/neusym-mcp/src/main.rs`
**Run**: `cargo check -p neusym-mcp`

1. Create `crates/neusym-mcp/src/cli.rs`:

   ```rust
   use clap::{Parser, Subcommand};
   use neusym_core::{ConflictStrategy, Provider, SyncDirection};

   #[derive(Parser)]
   #[command(name = "neusym", about = "Jira/Linear sync bridge")]
   pub struct Cli {
       #[command(subcommand)]
       pub command: Command,

       /// Output as JSON
       #[arg(long, global = true)]
       pub json: bool,
   }

   #[derive(Subcommand)]
   pub enum Command {
       /// Start MCP server (stdio transport)
       Serve,
       /// Search issues in a provider
       Search {
           #[arg(long)]
           provider: Provider,
           query: String,
       },
       /// Get a single issue by identifier
       Get {
           #[arg(long)]
           provider: Provider,
           identifier: String,
       },
       /// Sync operations
       Sync {
           #[command(subcommand)]
           action: SyncCommand,
       },
       /// Provider and mapping health check
       Health,
   }

   #[derive(Subcommand)]
   pub enum SyncCommand {
       /// Link two issues for sync
       Link {
           #[arg(long)]
           source: String,
           #[arg(long)]
           target: String,
           #[arg(long, default_value = "bidirectional")]
           direction: SyncDirection,
       },
       /// Push changes from source to target
       Push {
           mapping_id: String,
           #[arg(long, default_value = "source-wins")]
           strategy: ConflictStrategy,
       },
       /// Show all sync mappings
       Status,
   }
   ```

2. Rewrite `crates/neusym-mcp/src/main.rs`:

   ```rust
   mod cli;
   mod tools;

   use std::sync::Arc;

   use clap::Parser;
   use rmcp::ServiceExt;

   use neusym_core::ports::{
       HealthCheck, ProviderQuery, SyncOperations,
   };
   use neusym_sync::{
       EnvCredentialResolver, FileOutputStore,
       JsonMappingStore, NeusymService,
   };

   use crate::cli::{Cli, Command, SyncCommand};
   use crate::tools::NeusymMcp;

   fn build_service() -> Arc<NeusymService> {
       Arc::new(NeusymService::new(
           Box::new(EnvCredentialResolver),
           Box::new(JsonMappingStore::new(
               JsonMappingStore::default_path(),
           )),
           Box::new(FileOutputStore::new(
               FileOutputStore::default_path(),
           )),
       ))
   }

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error>> {
       miette::set_hook(Box::new(|_| {
           Box::new(
               miette::MietteHandlerOpts::new().build(),
           )
       }))?;

       let cli = Cli::parse();

       match cli.command {
           Command::Serve => {
               eprintln!(
                   "neusym MCP server starting (stdio)"
               );
               let service = build_service();
               let server = NeusymMcp::new(service);
               let transport =
                   rmcp::transport::io::stdio();
               let _svc =
                   server.serve(transport).await?;
           }
           Command::Search { provider, query } => {
               let svc = build_service();
               let results = svc
                   .search(provider, &query, None)
                   .await?;
               if cli.json {
                   println!(
                       "{}",
                       serde_json::to_string_pretty(
                           &results
                       )?
                   );
               } else {
                   for issue in &results {
                       println!(
                           "{} {}",
                           issue.identifier,
                           issue.title
                       );
                   }
               }
           }
           Command::Get {
               provider,
               identifier,
           } => {
               let svc = build_service();
               let issue = svc
                   .get(provider, &identifier, None)
                   .await?;
               if cli.json {
                   println!(
                       "{}",
                       serde_json::to_string_pretty(&issue)?
                   );
               } else {
                   println!(
                       "{} {}",
                       issue.identifier, issue.title
                   );
                   if let Some(ref desc) = issue.description
                   {
                       println!("{}", desc);
                   }
               }
           }
           Command::Sync { action } => match action {
               SyncCommand::Link {
                   source,
                   target,
                   direction,
               } => {
                   let svc = build_service();
                   let mapping = svc
                       .link(&source, &target, direction)
                       .await?;
                   if cli.json {
                       println!(
                           "{}",
                           serde_json::to_string_pretty(
                               &mapping
                           )?
                       );
                   } else {
                       println!(
                           "Linked {} <-> {}",
                           mapping.source.identifier,
                           mapping.target.identifier
                       );
                   }
               }
               SyncCommand::Push {
                   mapping_id,
                   strategy,
               } => {
                   let svc = build_service();
                   let event = svc
                       .push(&mapping_id, strategy)
                       .await?;
                   if cli.json {
                       println!(
                           "{}",
                           serde_json::to_string_pretty(
                               &event
                           )?
                       );
                   } else {
                       println!(
                           "Pushed {}",
                           event.mapping_id
                       );
                   }
               }
               SyncCommand::Status => {
                   let svc = build_service();
                   let mappings =
                       svc.status().await?;
                   if cli.json {
                       println!(
                           "{}",
                           serde_json::to_string_pretty(
                               &mappings
                           )?
                       );
                   } else {
                       for m in &mappings {
                           println!(
                               "{} {} <-> {}",
                               m.id,
                               m.source.identifier,
                               m.target.identifier
                           );
                       }
                   }
               }
           },
           Command::Health => {
               let svc = build_service();
               let report = svc.health().await?;
               if cli.json {
                   println!(
                       "{}",
                       serde_json::to_string_pretty(
                           &report
                       )?
                   );
               } else {
                   for p in &report.providers {
                       println!(
                           "{:?}: {} ({}ms)",
                           p.provider,
                           if p.reachable {
                               "OK"
                           } else {
                               "FAIL"
                           },
                           p.latency_ms.unwrap_or(0)
                       );
                   }
                   println!(
                       "Mappings: {} total, {} stale",
                       report.mappings_total,
                       report.mappings_stale
                   );
               }
           }
       }
       Ok(())
   }
   ```

3. Add `clap::ValueEnum` derives to `Provider`, `SyncDirection`,
   and `ConflictStrategy` in `neusym-core/src/types.rs` so clap
   can parse them from CLI args. Add `clap` as an optional dep
   to `neusym-core` behind a `cli` feature:

   In `crates/neusym-core/Cargo.toml`:

   ```toml
   [features]
   cli = ["clap"]

   [dependencies]
   clap = { workspace = true, optional = true }
   ```

   On the enum definitions, add:

   ```rust
   #[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
   ```

   In `crates/neusym-mcp/Cargo.toml`, use the feature:

   ```toml
   neusym-core = { workspace = true, features = ["cli"] }
   ```

4. Verify:

   ```
   cargo check -p neusym-mcp               -> compiles
   cargo clippy -p neusym-mcp -- -D warnings -> zero warnings
   ```

5. Run: `git branch --show-current`
   Commit: `git commit -m "feat(mcp): add CLI subcommands with --json output"`

---

### Task 12: Conformance tests for IssueProvider trait

**Crate**: `neusym-core`
**File(s)**: `crates/neusym-core/src/test_support.rs`,
`crates/neusym-core/src/lib.rs`
**Run**: `cargo nextest run -p neusym-core`

1. Create `crates/neusym-core/src/test_support.rs` with an
   in-memory `IssueProvider` fake and a conformance test suite:

   ```rust
   use async_trait::async_trait;
   use std::sync::Mutex;

   use crate::{NormalizedIssue, NeusymError, Provider, Result};
   use crate::ports::IssueProvider;

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

   #[async_trait]
   impl IssueProvider for InMemoryProvider {
       async fn search(
           &self,
           query: &str,
       ) -> Result<Vec<NormalizedIssue>> {
           let issues = self.issues.lock().unwrap();
           Ok(issues
               .iter()
               .filter(|i| {
                   i.title.contains(query)
                       || i.identifier.contains(query)
               })
               .cloned()
               .collect())
       }

       async fn get(
           &self,
           identifier: &str,
       ) -> Result<NormalizedIssue> {
           let issues = self.issues.lock().unwrap();
           issues
               .iter()
               .find(|i| i.identifier == identifier)
               .cloned()
               .ok_or_else(|| {
                   NeusymError::MappingNotFound(
                       identifier.to_string(),
                   )
               })
       }

       async fn create(
           &self,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue> {
           let mut issues = self.issues.lock().unwrap();
           let created = NormalizedIssue {
               id: format!("id-{}", issues.len()),
               ..issue.clone()
           };
           issues.push(created.clone());
           Ok(created)
       }

       async fn update(
           &self,
           identifier: &str,
           issue: &NormalizedIssue,
       ) -> Result<NormalizedIssue> {
           let mut issues = self.issues.lock().unwrap();
           let pos = issues
               .iter()
               .position(|i| i.identifier == identifier)
               .ok_or_else(|| {
                   NeusymError::MappingNotFound(
                       identifier.to_string(),
                   )
               })?;
           let updated = NormalizedIssue {
               id: issues[pos].id.clone(),
               identifier: identifier.to_string(),
               ..issue.clone()
           };
           issues[pos] = updated.clone();
           Ok(updated)
       }
   }

   /// Conformance suite: verifies any IssueProvider impl
   /// satisfies the trait contract.
   pub async fn assert_issue_provider_contract(
       provider: &dyn IssueProvider,
   ) {
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
       let updated = provider
           .update("TEST-1", &modified)
           .await
           .unwrap();
       assert_eq!(updated.title, "Updated title");

       let re_fetched = provider.get("TEST-1").await.unwrap();
       assert_eq!(re_fetched.title, "Updated title");

       // search finds the issue
       let results = provider.search("Updated").await.unwrap();
       assert!(!results.is_empty());
       assert!(results
           .iter()
           .any(|i| i.identifier == "TEST-1"));
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
   ```

2. Add module to `crates/neusym-core/src/lib.rs`:

   ```rust
   pub mod test_support;
   ```

3. Add `tokio` as dev-dep to `crates/neusym-core/Cargo.toml`:

   ```toml
   [dev-dependencies]
   tokio = { workspace = true }
   ```

4. Verify:

   ```
   cargo nextest run -p neusym-core         -> all green
   cargo clippy -p neusym-core -- -D warnings -> zero warnings
   ```

5. Run: `git branch --show-current`
   Commit: `git commit -m "test(core): add InMemoryProvider and IssueProvider conformance suite"`

---

### Task 13: Workspace-level build verification

**Run**: `cargo build --workspace && cargo clippy --workspace -- -D warnings`

1. Verify full workspace builds cleanly.
2. Run all tests: `cargo nextest run --workspace`
3. Run: `git branch --show-current`
   Commit: `git commit -m "chore: verify full workspace builds and passes tests"`
