//! # TimberDB
//! 
//! A high-performance embeddable log database with block-based storage and configurable compression.
//! 
//! ## Features
//! 
//! - Block-based storage with automatic rotation
//! - Multiple compression algorithms (LZ4, Zstd)
//! - Time-based indexing for efficient queries
//! - Embeddable design with simple API
//! - Optional HTTP API server
//! - Production-ready with comprehensive error handling
//! 
//! ## Example
//! 
//! ```rust,no_run
//! use timberdb::{TimberDB, Config, LogEntry};
//! use std::collections::HashMap;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::default();
//!     let db = TimberDB::open(config).await?;
//!     
//!     // Create a partition
//!     let partition_id = db.create_partition("logs").await?;
//!     
//!     // Insert a log entry
//!     let mut tags = HashMap::new();
//!     tags.insert("level".to_string(), "info".to_string());
//!     
//!     let entry = LogEntry::new("application", "Server started", tags);
//!     let entry_id = db.append(&partition_id, entry).await?;
//!     
//!     // Query logs
//!     let results = db.query()
//!         .partition(&partition_id)
//!         .limit(100)
//!         .execute()
//!         .await?;
//!     
//!     println!("Found {} log entries", results.len());
//!     Ok(())
//! }
//! ```

use std::sync::Arc;

pub use chrono::{DateTime, Utc};
pub use uuid::Uuid;

mod config;
mod storage;
mod query;
mod error;

#[cfg(feature = "api")]
pub mod api;

pub use config::*;
pub use storage::{LogEntry, PartitionInfo};
pub use query::{Query, QueryBuilder, QueryResult};
pub use error::{Error, Result};

/// Main TimberDB database instance
#[derive(Clone)]
pub struct TimberDB {
    storage: Arc<storage::Storage>,
}

impl TimberDB {
    /// Open a TimberDB instance with the given configuration
    pub async fn open(config: Config) -> Result<Self> {
        let storage = storage::Storage::new(config).await?;
        
        Ok(TimberDB {
            storage: Arc::new(storage),
        })
    }
    
    /// Create a new partition
    pub async fn create_partition(&self, name: &str) -> Result<String> {
        self.storage.create_partition(name).await
    }
    
    /// List all partitions
    pub async fn list_partitions(&self) -> Result<Vec<PartitionInfo>> {
        self.storage.list_partitions().await
    }
    
    /// Get partition information
    pub async fn get_partition(&self, id: &str) -> Result<PartitionInfo> {
        self.storage.get_partition(id).await
    }
    
    /// Append a log entry to a partition
    pub async fn append(&self, partition_id: &str, entry: LogEntry) -> Result<u64> {
        self.storage.append(partition_id, entry).await
    }
    
    /// Get a specific log entry
    pub async fn get_entry(&self, partition_id: &str, entry_id: u64) -> Result<LogEntry> {
        self.storage.get_entry(partition_id, entry_id).await
    }
    
    /// Create a new query builder
    pub fn query(&self) -> QueryBuilder {
        QueryBuilder::new(self.storage.clone())
    }
    
    /// Flush all pending writes
    pub async fn flush(&self) -> Result<()> {
        self.storage.flush().await
    }
    
    /// Close the database and flush all data
    pub async fn close(self) -> Result<()> {
        Arc::try_unwrap(self.storage)
            .map_err(|_| Error::DatabaseInUse)?
            .close()
            .await
    }

    pub fn get_config(&self) -> &Config {
        &self.storage.config
    }
}