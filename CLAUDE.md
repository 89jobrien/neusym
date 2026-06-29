# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working
with code in this repository.

## What This Is

neusym is an MCP server that bridges Jira and Linear for bidirectional
issue sync. Built with rmcp over stdio transport.

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo test -p neusym-sync             # single crate
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

Rust edition 2024. Dual licensed MIT OR Apache-2.0.

## Prerequisites

`crux-types` is a path dependency at `../crux/crates/crux-types`. The
[crux](https://github.com/89jobrien/crux) repo must be cloned alongside
neusym at `~/dev/crux`.

## Architecture

### Workspace Crates

| Crate           | Role                                                |
| --------------- | --------------------------------------------------- |
| `neusym-core`   | Domain types, `IssueProvider` port trait, errors    |
| `neusym-linear` | Linear GraphQL adapter (implements `IssueProvider`) |
| `neusym-jira`   | Jira REST adapter (implements `IssueProvider`)      |
| `neusym-sync`   | Sync engine, mapping store, conflict detection      |
| `neusym-mcp`    | MCP server binary (rmcp, stdio transport)           |

### Hexagonal Ports

`neusym-core::ports::IssueProvider` is the single provider boundary.
Both Linear and Jira crates implement it with boxed futures for
dyn-compatibility. The sync engine accepts `&dyn IssueProvider` --
no direct coupling to either API.

### Error Handling

Dual error strategy:

- **miette** (`Diagnostic` derive) for rich CLI error display with
  codes, help text, and source spans
- **crux-types** (`CruxErr`) for pipeline integration via
  `NeusymError::into_crux_err()`

`neusym_core::Result<T>` is `std::result::Result<T, NeusymError>`.

### Mapping Store

JSON file at `~/.ctx/neusym/mappings.json`. Tracks which issues are
linked across providers, sync direction, and last-synced timestamp.

## Downstream Tool Ecosystem (35 projects)

All consumed as CLI tools, not library deps:

| #   | Layer                 | Project      |
| --- | --------------------- | ------------ |
| 1   | AI/decisions          | disyn        |
| 2   | Task sync             | taskit       |
| 3   | Drift detection       | coursers     |
| 4   | Health/diagnostics    | checkup      |
| 5   | Infrastructure        | minibox      |
| 6   | Orchestration         | braid        |
| 7   | Validation            | agentlint    |
| 8   | Config deployment     | notfiles     |
| 9   | Context management    | rslm         |
| 10  | Security/redaction    | obfsck       |
| 11  | Task tracking         | doob         |
| 12  | Workflows             | crux         |
| 13  | Command DSL           | slash        |
| 14  | Command learning      | rx           |
| 15  | Command rewriting     | prefixe      |
| 16  | Skills/agents         | godmode      |
| 17  | Retry/resilience      | looprs       |
| 18  | CI/feedback loops     | devloop      |
| 19  | Structured LLM output | bamlish      |
| 20  | Knowledge graph       | kgx          |
| 21  | File routing          | maid         |
| 22  | MCP piping            | mcpipe       |
| 23  | Scaffolding           | sparkfile    |
| 24  | Agent improvement     | updog        |
| 25  | Self-improvement      | praxis       |
| 26  | Property testing      | propkit      |
| 27  | Preflight checks      | hooklings    |
| 28  | Query analysis        | groovenance  |
| 29  | Session state         | hj           |
| 30  | Deduplication         | cannibalizer |
| 31  | UI rendering          | gooey        |
| 32  | LLM chain composition | langchainx   |
| 33  | Shell scripting       | nu_libs      |
| 34  | Project tracking      | devobs       |
| 35  | Long-term memory      | pieces-ob    |

## MCP Tools (planned)

| Tool            | Purpose                            |
| --------------- | ---------------------------------- |
| `linear_search` | Search Linear issues by filter     |
| `linear_get`    | Get a Linear issue by identifier   |
| `jira_search`   | Search Jira issues by JQL          |
| `jira_get`      | Get a Jira issue by key            |
| `sync_link`     | Create a bidirectional mapping     |
| `sync_push`     | Push changes from source to target |
| `sync_status`   | Show all mappings and drift state  |
| `sync_health`   | Checkup-powered health report      |
