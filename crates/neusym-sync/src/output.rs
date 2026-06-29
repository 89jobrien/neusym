use async_trait::async_trait;
use std::path::{Path, PathBuf};

use neusym_core::Result;
use neusym_core::ports::OutputStore;
use obfsck::ObfuscationLevel;
use tokio::fs;
use tokio::io::AsyncWriteExt;

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
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".ctx").join("neusym")
    }

    fn channel_path(&self, channel: &str) -> PathBuf {
        self.ctx_dir.join(channel)
    }
}

fn scrub(text: &str) -> String {
    let (scrubbed, _) = obfsck::obfuscate_text(text, ObfuscationLevel::Standard);
    scrubbed
}

#[async_trait]
impl OutputStore for FileOutputStore {
    async fn append(&self, channel: &str, entry: &serde_json::Value) -> Result<()> {
        let path = self.channel_path(channel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let line = serde_json::to_string(entry)?;
        let scrubbed = scrub(&line);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(scrubbed.as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }

    async fn overwrite(&self, channel: &str, data: &serde_json::Value) -> Result<()> {
        let path = self.channel_path(channel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let content = serde_json::to_string_pretty(data)?;
        let scrubbed = scrub(&content);
        fs::write(&path, scrubbed).await?;
        Ok(())
    }
}
