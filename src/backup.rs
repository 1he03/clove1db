// backup.rs
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::path::PathBuf;
use std::sync::Arc;
use chrono::Local;
use crate::units::ClError;
use crate::{event_emitter::EventEmitter, units::Result};

fn version_table_name(table_name: &str) -> String {
    format!("{}_version", table_name)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum BackupOperation {
    Set,
    Delete,
    Restore,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupRecord {
    pub version:   u64,
    pub timestamp: i64,
    pub date:      String,
    pub operation: BackupOperation,
    pub table:     String,
    pub key:       String,
    pub data:      Option<Vec<u8>>,  // None on Delete
    pub restored_version: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct BackupManager {
    pub db:      Arc<Database>,
    emitter:     Arc<EventEmitter>,
}

impl BackupManager {
    pub fn new(path: &PathBuf, emitter: EventEmitter) -> Result<Self> {
        // Last version in any table — we will search later when using
        // The tables are created in DatabaseManager::new() ✅
        let db = Arc::new(Database::create(path).map_err(|e| ClError::Database(redb::Error::from(e)))?);
        Ok(Self {
            db: db.clone(),
            emitter: Arc::new(emitter),
        })
    }

    pub fn init_table(&self, table_name: &str) -> Result<()> {
        let ver_name = version_table_name(table_name);
        let write_txn = self.db.begin_write()?;
        {
            // Backupd table
            let data_table: TableDefinition<&str, &[u8]> = TableDefinition::new(table_name);
            write_txn.open_table(data_table)?;

            // Backupd version table — key: entity_id, value: u64
            let ver_table: TableDefinition<&str, u64> = TableDefinition::new(ver_name.as_str());
            write_txn.open_table(ver_table)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Called from DatabaseManager::set()
    pub fn record_set<'db>(
        &self,
        table: TableDefinition<'db, &str, &[u8]>,  // ← The TableDefinition
        table_name: &str,
        key: &str,
        data: Vec<u8>,
    ) -> Result<()> {
        let version = self.next_version(table_name, key)?;
        let record = BackupRecord {
            version,
            timestamp: Local::now().timestamp_millis(),
            date:      Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            operation: BackupOperation::Set,
            table:     table_name.to_string(),
            key:       key.to_string(),
            data:      Some(data),
            restored_version: None,
        };
        self.write(table, table_name, key, version, &record)
    }

    /// Called from DatabaseManager::delete()
    pub fn record_delete<'db>(
        &self,
        table: TableDefinition<'db, &str, &[u8]>,  // ← The TableDefinition
        table_name: &str,
        key: &str,
    ) -> Result<()> {
        let version = self.next_version(table_name, key)?;

        let record = BackupRecord {
            version,
            timestamp: Local::now().timestamp_millis(),
            date:      Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            operation: BackupOperation::Delete,
            table:     table_name.to_string(),
            key:       key.to_string(),
            data:      None,
            restored_version: None,
        };
        self.write(table, table_name, key, version, &record)
    }

    pub fn record_restore<'db>(
        &self,
        table:            TableDefinition<'db, &str, &[u8]>,
        table_name:       &str,
        key:              &str,
        restored_version: u64,
        data:             Option<Vec<u8>>,  // None = Was Delete
    ) -> Result<()> {
        let version = self.next_version(table_name, key)?;
    
        let record = BackupRecord {
            version,
            timestamp:        Local::now().timestamp_millis(),
            date:             Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            operation:        BackupOperation::Restore,
            table:            table_name.to_string(),
            key:              key.to_string(),
            restored_version: Some(restored_version),
            data,
        };
    
        self.write(table, table_name, key, version, &record)
    }
    

    /// Retrieve history of a specific key — ordered from oldest to newest
    pub fn history<'db>(
        &self,
        table:      TableDefinition<'db, &str, &[u8]>,
        key:        &str,
    ) -> Result<Vec<BackupRecord>> {
        let read_txn = self.db.begin_read()?;
        let tbl      = read_txn.open_table(table)?;
        let prefix   = format!("{}:", key);

        let mut records: Vec<BackupRecord> = tbl
            .iter()?
            .filter_map(|e| e.ok())
            .filter(|(k, _)| k.value().starts_with(&prefix))
            .filter_map(|(_, v)| serde_json::from_slice::<BackupRecord>(v.value()).ok())
            .collect();

        records.sort_by_key(|r| r.version);
        Ok(records)
    }

    /// The last version of data before a specific timestamp
    pub fn view_at<'db>(
        &self,
        table:     TableDefinition<'db, &str, &[u8]>,
        key:       &str,
        timestamp: i64,
    ) -> Result<Option<Vec<u8>>> {
        Ok(self.history(table, key)?
            .into_iter()
            .filter(|r| r.timestamp <= timestamp)
            .last()
            .and_then(|r| match r.operation {
                BackupOperation::Set    => r.data,
                BackupOperation::Restore => r.data,
                BackupOperation::Delete => None,
            }))
    }

    pub fn view_by_version<'db>(
        &self,
        table:   TableDefinition<'db, &str, &[u8]>,
        key:     &str,
        version: u64,
    ) -> Result<Option<Vec<u8>>> {
        let backup_key = format!("{}:{}", key, version);
        let read_txn   = self.db.begin_read()?;
        let tbl        = read_txn.open_table(table)?;
    
        Ok(tbl
            .get(backup_key.as_str())?
            .and_then(|v| serde_json::from_slice::<BackupRecord>(v.value()).ok())
            .and_then(|r| match r.operation {
                BackupOperation::Set    => r.data,
                BackupOperation::Restore => r.data,
                BackupOperation::Delete => None,
            }))
    }

    pub fn current_version(&self, table_name: &str, key: &str) -> Result<u64> {
        let ver_name  = version_table_name(table_name);
        let ver_table: TableDefinition<&str, u64> = TableDefinition::new(ver_name.as_str());

        let read_txn = self.db.begin_read()?;
        let tbl      = read_txn.open_table(ver_table)?;

        Ok(tbl.get(key)?.map(|v| v.value()).unwrap_or(0))
    }


    // ── Internal ──────────────────────────────────────────────────────

    fn next_version(&self, table_name: &str, key: &str) -> Result<u64> {
        let ver_name  = version_table_name(table_name);

        let ver_table: TableDefinition<&str, u64> = TableDefinition::new(ver_name.as_str());

        let write_txn = self.db.begin_write()?;
        let new_version = {
            let mut tbl = write_txn.open_table(ver_table)?;
            let current = tbl.get(key)?.map(|v| v.value()).unwrap_or(0);
            let next    = current + 1;
            tbl.insert(key, next)?;
            next
        };
        write_txn.commit()?;

        Ok(new_version)
    }

    fn write<'db>(
        &self,
        table:      TableDefinition<'db, &str, &[u8]>,
        table_name: &str,
        key:        &str,
        version:    u64,
        record:     &BackupRecord,
    ) -> Result<()> {
        // backup key = "entity_id:version" → append-only ✅
        let backup_key = format!("{}:{}", key, version);
        let data       = serde_json::to_vec(record)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut tbl = write_txn.open_table(table)?;
            tbl.insert(backup_key.as_str(), data.as_slice())?;
        }
        write_txn.commit()?;

        self.emitter.debug(format!(
            "backup [{:?}] {}:{} v{}",
            record.operation, table_name, key, version
        ));

        Ok(())
    }
}
