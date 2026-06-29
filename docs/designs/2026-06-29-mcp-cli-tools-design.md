# Design: MCP Tools, CLI Interface, and Provider Completion

## Goal

Complete the Linear/Jira provider adapters (create/update) and expose
all operations as both MCP tools (rmcp/stdio) and CLI subcommands from
a single `neusym` binary.

## Approved Approach

Trait-first with ISP-segregated port traits. Three domain port traits
(`ProviderQuery`, `SyncOperations`, `HealthCheck`) plus infrastructure
ports (`CredentialResolver`, `MappingStore`, `OutputStore`). Adapters
in `neusym-sync`, `neusym-linear`, `neusym-jira`. Two presentation
adapters: MCP server and CLI -- both thin shells over the same ports.

## Crate Ownership

- **`neusym-core`** -- all domain types, port traits, errors. Zero
  adapter code.
- **`neusym-linear`** -- `LinearClient` implements `IssueProvider`.
  Affected: add `create`/`update` mutations.
- **`neusym-jira`** -- `JiraClient` implements `IssueProvider`.
  Affected: add `create`/`update` REST calls.
- **`neusym-sync`** -- `NeusymService` implements `ProviderQuery`,
  `SyncOperations`, `HealthCheck`. Also contains adapter impls:
  `JsonMappingStore`, `EnvCredentialResolver`, `FileOutputStore`.
- **`neusym-mcp`** -- renamed to single binary crate. Contains both
  MCP server (`neusym serve`) and CLI subcommands. Depends on all
  other crates. Binary name: `neusym`.

No new crates. `neusym-cli` is not a separate crate -- CLI and MCP
live in `neusym-mcp` behind subcommands.

## Public API

### New Workspace Dependencies

```toml
async-trait = "0.1"
clap = { version = "4", features = ["derive"] }
```

### Port Traits (`neusym-core::ports`)

```rust
use async_trait::async_trait;
use crate::{
    Credential, ConflictStrategy, HealthReport, Mapping,
    NormalizedIssue, Provider, Result, SyncDirection, SyncEvent,
};

/// Existing trait, updated to use #[async_trait].
#[async_trait]
pub trait IssueProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<NormalizedIssue>>;
    async fn get(&self, identifier: &str) -> Result<NormalizedIssue>;
    async fn create(&self, issue: &NormalizedIssue) -> Result<NormalizedIssue>;
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
    async fn resolve(&self, provider: Provider) -> Result<Credential>;
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
    async fn append(&self, channel: &str, entry: &serde_json::Value)
        -> Result<()>;
    async fn overwrite(&self, channel: &str, data: &serde_json::Value)
        -> Result<()>;
}
```

### Domain Types (`neusym-core::types`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

### Error Additions (`neusym-core::error`)

```rust
// New variant added to NeusymError
#[error("missing credential: {field}")]
#[diagnostic(
    code(neusym::missing_credential),
    help("set {field} via environment variable or pass per-call")
)]
MissingCredential { field: String },
```

### Service (`neusym-sync::service`)

```rust
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
    ) -> Self;
}
```

`NeusymService` implements `ProviderQuery`, `SyncOperations`, and
`HealthCheck`. It constructs `LinearClient` or `JiraClient` per-call
using resolved credentials.

### Adapters (`neusym-sync`)

```rust
// neusym-sync::store (renamed from MappingStore struct)
pub struct JsonMappingStore {
    path: PathBuf,
}
impl MappingStore for JsonMappingStore { /* ... */ }

// neusym-sync::credentials
pub struct EnvCredentialResolver;
impl CredentialResolver for EnvCredentialResolver { /* ... */ }

// neusym-sync::output
pub struct FileOutputStore {
    ctx_dir: PathBuf,
}
impl OutputStore for FileOutputStore { /* ... */ }
```

### CLI (`neusym-mcp::cli`)

```rust
#[derive(clap::Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(clap::Subcommand)]
pub enum Command {
    Serve,
    Search {
        #[arg(long)]
        provider: Provider,
        query: String,
    },
    Get {
        #[arg(long)]
        provider: Provider,
        identifier: String,
    },
    Sync {
        #[command(subcommand)]
        action: SyncCommand,
    },
    Health,
}

#[derive(clap::Subcommand)]
pub enum SyncCommand {
    Link {
        #[arg(long)]
        source: String,
        #[arg(long)]
        target: String,
        #[arg(long, default_value = "bidirectional")]
        direction: SyncDirection,
    },
    Push {
        mapping_id: String,
        #[arg(long, default_value = "source-wins")]
        strategy: ConflictStrategy,
    },
    Status,
}
```

## Data Flow

### Provider query (search/get)

1. CLI or MCP receives request with `Provider`, query, optional creds
2. `NeusymService::search()` resolves creds: per-call > resolver
3. Constructs `LinearClient` or `JiraClient` with resolved creds
4. Delegates to `IssueProvider::search()` on the constructed client
5. Returns `Vec<NormalizedIssue>`
6. Appends to `sync.log` via `OutputStore::append()`

### Sync push

1. CLI or MCP receives mapping_id + `ConflictStrategy`
2. `NeusymService::push()` loads mapping from `MappingStore`
3. Resolves creds for both source and target providers
4. Fetches current state from both via `IssueProvider::get()`
5. If `ReportOnly`: compares fields, returns `SyncEvent` with
   `SyncAction::Conflict` for each divergent field
6. If `SourceWins`/`TargetWins`: calls `IssueProvider::update()`
   on the losing side
7. If `FieldLevel`: applies per-field strategy, constructs merged
   `NormalizedIssue`, updates target
8. Records event via `OutputStore::append("sync.log", ...)`
9. Updates `last_synced` on mapping via `MappingStore`

### Health check

1. CLI or MCP receives health request
2. `NeusymService::health()` resolves creds for all configured
   providers
3. Pings each provider with a lightweight query, measures latency
4. Loads all mappings, counts stale (last_synced > threshold) and
   conflicts
5. Returns `HealthReport`
6. Overwrites `.ctx/health.json` via `OutputStore::overwrite()`

## .ctx Output Layout

```
.ctx/
  neusym/
    mappings.json       # MappingStore data (read/write)
    sync.log            # JSONL, append-mode, one line per sync event
    health.json         # overwritten on each health check
    last-run.json       # overwritten, last tool invocation metadata
```

## Hexagonal Boundaries

### Ports (traits in `neusym-core::ports`)

| Port                 | Responsibility              |
| -------------------- | --------------------------- |
| `IssueProvider`      | Single-provider CRUD        |
| `ProviderQuery`      | Cross-provider search/get   |
| `SyncOperations`     | Link, push, status          |
| `HealthCheck`        | Provider + mapping health   |
| `CredentialResolver` | Dynamic credential lookup   |
| `MappingStore`       | Mapping persistence         |
| `OutputStore`        | Structured log/state output |

### Adapters

| Adapter                 | Crate           | Implements                                       |
| ----------------------- | --------------- | ------------------------------------------------ |
| `LinearClient`          | `neusym-linear` | `IssueProvider`                                  |
| `JiraClient`            | `neusym-jira`   | `IssueProvider`                                  |
| `NeusymService`         | `neusym-sync`   | `ProviderQuery`, `SyncOperations`, `HealthCheck` |
| `JsonMappingStore`      | `neusym-sync`   | `MappingStore`                                   |
| `EnvCredentialResolver` | `neusym-sync`   | `CredentialResolver`                             |
| `FileOutputStore`       | `neusym-sync`   | `OutputStore`                                    |
| MCP handler             | `neusym-mcp`    | rmcp `ServerHandler`                             |
| CLI handler             | `neusym-mcp`    | clap dispatch                                    |

## Testing Strategy

### Conformance tests (per trait)

Reusable test suites for each port trait, run against every impl:

- `assert_issue_provider_contract()` -- verifies `IssueProvider`
  round-trips (create then get returns same data). Run against
  in-memory fake, and optionally against real APIs in integration.
- `assert_mapping_store_contract()` -- add/load/find consistency.
  Run against `JsonMappingStore` with tempdir.
- `assert_credential_resolver_contract()` -- resolve returns
  correct variant per provider.

### Unit tests

- `NeusymService` with in-memory fakes for all ports -- test
  conflict strategies, credential fallback logic, error paths.
- `ConflictStrategy` field-level merge logic.
- `HealthReport` aggregation from provider pings.

### Integration tests

- `LinearClient` against real API (gated on env var).
- `JiraClient` against real API (gated on env var).
- CLI subcommands end-to-end with `assert_cmd`.

## Out of Scope

- Webhooks / real-time sync
- Bulk operations (multi-issue push)
- Checkup CLI integration for health
- Additional providers beyond Linear/Jira
- Timestamp-based "newest wins" conflict strategy

## Risk

- [x] Breaking API changes: yes -- `IssueProvider` trait signature
      changes from manual `Pin<Box<...>>` to `#[async_trait]`. All
      existing impls (`LinearClient`, `JiraClient`) must be updated.
      `MappingStore` struct renamed to `JsonMappingStore`; trait takes
      the name.
- [x] New external dependency: yes -- `async-trait` (proc-macro,
      widely used, no risk), `clap` (CLI only, already common in
      workspace ecosystem).
- [ ] Feature flag required: no.

## Dependency Graph (post-design)

```
neusym-core          (types, port traits, errors)
  neusym-linear      (IssueProvider impl)
  neusym-jira        (IssueProvider impl)
  neusym-sync        (service + adapter impls; depends on
                      neusym-core, neusym-linear, neusym-jira)
    neusym-mcp       (MCP server + CLI binary; depends on all)
```
