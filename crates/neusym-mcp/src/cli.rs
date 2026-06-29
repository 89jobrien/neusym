use clap::{Parser, Subcommand, ValueEnum};
use neusym_core::{ConflictStrategy, Provider, SyncDirection};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliStrategy {
    #[value(alias = "source_wins")]
    SourceWins,
    #[value(alias = "target_wins")]
    TargetWins,
    #[value(alias = "report_only")]
    ReportOnly,
}

impl From<CliStrategy> for ConflictStrategy {
    fn from(s: CliStrategy) -> Self {
        match s {
            CliStrategy::SourceWins => ConflictStrategy::SourceWins,
            CliStrategy::TargetWins => ConflictStrategy::TargetWins,
            CliStrategy::ReportOnly => ConflictStrategy::ReportOnly,
        }
    }
}

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
        source_provider: Provider,
        #[arg(long)]
        source: String,
        #[arg(long)]
        target_provider: Provider,
        #[arg(long)]
        target: String,
        #[arg(long, default_value = "bidirectional")]
        direction: SyncDirection,
    },
    /// Push changes from source to target
    Push {
        mapping_id: String,
        #[arg(long, default_value = "source-wins")]
        strategy: CliStrategy,
    },
    /// Show all sync mappings
    Status,
}
