use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::services::local_file;

use super::{file_io::operation_id, planner::ConfigValue};

const MAX_BACKUP_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleBackup {
    pub id: String,
    pub rule_id: String,
    pub target_file: PathBuf,
    pub field_path: Vec<String>,
    pub old_value: ConfigValue,
    pub applied_value: ConfigValue,
    pub before_hash: String,
    pub after_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct RuleBackupStore {
    directory: PathBuf,
}

impl RuleBackupStore {
    pub fn production() -> Option<Self> {
        dirs::data_local_dir().map(|directory| Self {
            directory: directory.join("ProxyEnv").join("app-rule-backups"),
        })
    }

    #[cfg(test)]
    pub fn at(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn create(&self, mut backup: RuleBackup) -> Result<RuleBackup, ()> {
        fs::create_dir_all(&self.directory).map_err(|_| ())?;
        local_file::ensure_safe_directory(&self.directory).map_err(|_| ())?;
        backup.id = operation_id();
        if !valid_backup_id(&backup.id) {
            return Err(());
        }
        let path = self.path(&backup.id);
        let bytes = serde_json::to_vec_pretty(&backup).map_err(|_| ())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| ())?;
        if file
            .write_all(&bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .is_err()
        {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(());
        }
        Ok(backup)
    }

    pub fn load(&self, id: &str) -> Result<RuleBackup, ()> {
        if !valid_backup_id(id) {
            return Err(());
        }
        let path = self.path(id);
        let bytes = local_file::safe_read(&path, MAX_BACKUP_BYTES)
            .map_err(|_| ())?
            .ok_or(())?;
        let backup: RuleBackup = serde_json::from_slice(&bytes).map_err(|_| ())?;
        if backup.id != id {
            return Err(());
        }
        Ok(backup)
    }

    pub fn remove(&self, id: &str) {
        if valid_backup_id(id) {
            let _ = fs::remove_file(self.path(id));
        }
    }

    fn path(&self, id: &str) -> PathBuf {
        self.directory.join(format!("{id}.json"))
    }
}

fn valid_backup_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 96 && id.bytes().all(|byte| byte.is_ascii_digit() || byte == b'-')
}
