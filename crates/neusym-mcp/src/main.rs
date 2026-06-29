mod cli;
mod tools;

use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;

use neusym_core::ports::{HealthCheck, ProviderQuery, SyncOperations};
use neusym_sync::{EnvCredentialResolver, FileOutputStore, JsonMappingStore, NeusymService};

use crate::cli::{Cli, Command, SyncCommand};
use crate::tools::NeusymMcp;

fn build_service() -> Arc<NeusymService> {
    Arc::new(NeusymService::new(
        Box::new(EnvCredentialResolver),
        Box::new(JsonMappingStore::new(JsonMappingStore::default_path())),
        Box::new(FileOutputStore::new(FileOutputStore::default_path())),
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    miette::set_hook(Box::new(|_| {
        Box::new(miette::MietteHandlerOpts::new().build())
    }))?;

    let cli = Cli::parse();

    match cli.command {
        Command::Serve => {
            eprintln!("neusym MCP server starting (stdio)");
            let service = build_service();
            let server = NeusymMcp::new(service);
            let transport = rmcp::transport::io::stdio();
            let _svc = server.serve(transport).await?;
        }
        Command::Search { provider, query } => {
            let svc = build_service();
            let results = svc.search(provider, &query, None).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                for issue in &results {
                    println!("{} {}", issue.identifier, issue.title);
                }
            }
        }
        Command::Get {
            provider,
            identifier,
        } => {
            let svc = build_service();
            let issue = svc.get(provider, &identifier, None).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&issue)?);
            } else {
                println!("{} {}", issue.identifier, issue.title);
                if let Some(ref desc) = issue.description {
                    println!("{}", desc);
                }
            }
        }
        Command::Sync { action } => match action {
            SyncCommand::Link {
                source_provider,
                source,
                target_provider,
                target,
                direction,
            } => {
                let svc = build_service();
                let mapping = svc
                    .link(
                        source_provider,
                        &source,
                        target_provider,
                        &target,
                        direction,
                    )
                    .await?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&mapping)?);
                } else {
                    println!(
                        "Linked {} <-> {}",
                        mapping.source.identifier, mapping.target.identifier
                    );
                }
            }
            SyncCommand::Push {
                mapping_id,
                strategy,
            } => {
                let svc = build_service();
                let event = svc.push(&mapping_id, strategy.into()).await?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&event)?);
                } else {
                    println!("Pushed {}", event.mapping_id);
                }
            }
            SyncCommand::Status => {
                let svc = build_service();
                let mappings = svc.status().await?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&mappings)?);
                } else {
                    for m in &mappings {
                        println!(
                            "{} {} <-> {}",
                            m.id, m.source.identifier, m.target.identifier
                        );
                    }
                }
            }
        },
        Command::Health => {
            let svc = build_service();
            let report = svc.health().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for p in &report.providers {
                    println!(
                        "{:?}: {} ({}ms)",
                        p.provider,
                        if p.reachable { "OK" } else { "FAIL" },
                        p.latency_ms.unwrap_or(0)
                    );
                }
                println!(
                    "Mappings: {} total, {} stale",
                    report.mappings_total, report.mappings_stale
                );
            }
        }
    }
    Ok(())
}
