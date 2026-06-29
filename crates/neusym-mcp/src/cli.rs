use clap::{Parser, Subcommand};
use neusym_core::{Provider, SyncDirection};

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
        strategy: String,
    },
    /// Show all sync mappings
    Status,
}
