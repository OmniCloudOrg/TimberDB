// TimberDB: A high-performance distributed log database
// storage/mod.rs - Storage module for managing blocks, partitions, and logs

pub mod block;
pub mod compression;
pub mod index;
pub mod manager;

use std::sync::{Arc, RwLock};
use thiserror::Error;

use crate::config::StorageConfig;
use self::block::LogEntry;
use self::manager::{PartitionMetadata, StorageError, StorageManager};

// Storage errors wrapper
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// Storage engine for TimberDB
#[derive(Debug)]
pub struct Storage {
    // The storage manager
    manager: Arc<StorageManager>,
}

impl Storage {
    /// Create a new storage engine
    pub fn new(config: StorageConfig) -> Result<Self, Error> {
        let manager = StorageManager::new(config)?;
        
        Ok(Storage {
            manager: Arc::new(manager),
        })
    }
    
    /// Create a new partition
    pub fn create_partition(&self, name: &str) -> Result<String, Error> {
        Ok(self.manager.create_partition(name)?)
    }
    
    /// Get partition metadata
    pub fn get_partition(&self, partition_id: &str) -> Result<PartitionMetadata, Error> {
        Ok(self.manager.get_partition(partition_id)?)
    }
    
    /// List all partitions
    pub fn list_partitions(&self) -> Vec<PartitionMetadata> {
        self.manager.list_partitions()
    }
    
    /// Append a log entry to a partition
    pub fn append(
        &self,
        partition_id: &str,
        entry: LogEntry,
    ) -> Result<u64, Error> {
        Ok(self.manager.append(partition_id, entry)?)
    }
    
    /// Read a log entry from a partition
    pub fn read_entry(
        &self,
        partition_id: &str,
        block_id: &str,
        entry_id: u64,
    ) -> Result<LogEntry, Error> {
        Ok(self.manager.read_entry(partition_id, block_id, entry_id)?)
    }
    
    /// Get a clone of the storage manager
    pub fn manager(&self) -> Arc<StorageManager> {
        self.manager.clone()
    }
    
    /// Shutdown the storage engine
    pub fn shutdown(self) -> Result<(), Error> {
        // Get exclusive access to the manager
        let mut manager = Arc::try_unwrap(self.manager)
            .map_err(|_| {
                Error::Storage(StorageError::InvalidConfig(
                    "Storage manager still has active references".to_string(),
                ))
            })?;
        
        // Shut down the manager
        manager.shutdown()?;
        
        Ok(())
    }
}