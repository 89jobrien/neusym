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
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".ctx").join("neusym")
    }

    fn channel_path(&self, channel: &str) -> PathBuf {
        self.ctx_dir.join(channel)
    }
}

#[async_trait]
impl OutputStore for FileOutputStore {
    async fn append(&self, channel: &str, entry: &serde_json::Value) -> Result<()> {
        let path = self.channel_path(channel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        let line = serde_json::to_string(entry)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    async fn overwrite(&self, channel: &str, data: &serde_json::Value) -> Result<()> {
        let path = self.channel_path(channel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(data)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
