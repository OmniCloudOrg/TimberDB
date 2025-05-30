use crate::{Config, Error, Result, StorageConfig};
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

mod block;
mod partition;
mod compression;
mod log_entry;

pub use block::Block;
pub use partition::{Partition, PartitionInfo};
pub use log_entry::LogEntry;

/// Main storage engine
pub struct Storage {
    pub config: Config,
    pub partitions: Arc<RwLock<HashMap<String, Arc<Partition>>>>, // Wrapped in Arc
    pub background_handle: Option<tokio::task::JoinHandle<()>>,
    pub shutdown_signal: Arc<tokio::sync::Notify>,
}

impl Storage {
    /// Create a new storage instance
    pub async fn new(config: Config) -> Result<Self> {
        // Create data directory
        tokio::fs::create_dir_all(&config.data_dir).await?;
        
        let shutdown_signal = Arc::new(tokio::sync::Notify::new());
        
        let mut storage = Storage {
            config,
            partitions: Arc::new(RwLock::new(HashMap::new())), // Wrap in Arc
            background_handle: None,
            shutdown_signal,
        };
        
        // Load existing partitions
        storage.load_partitions().await?;
        
        // Start background tasks
        storage.start_background_tasks().await;
        
        Ok(storage)
    }
    
    /// Load existing partitions from disk
    async fn load_partitions(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.config.data_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip special directories
                    if name.starts_with('.') {
                        continue;
                    }
                    
                    // Try to load partition
                    let path_clone = path.clone();
                    match Partition::load(path_clone, self.config.storage.clone()).await {
                        Ok(partition) => {
                            let partition_id = partition.id().to_string();
                            self.partitions.write().await.insert(partition_id, Arc::new(partition));
                            log::info!("Loaded partition: {}", name);
                        }
                        Err(e) => {
                            log::warn!("Failed to load partition {}: {}", name, e);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Start background maintenance tasks
    async fn start_background_tasks(&mut self) {
        let config = self.config.clone();
        let partitions = Arc::clone(&self.partitions); // Clone the Arc, not the RwLock
        let shutdown_signal = self.shutdown_signal.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.storage.flush_interval);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Flush all partitions
                        let partition_list = {
                            partitions.read().await.values().cloned().collect::<Vec<_>>()
                        };
                        
                        for partition in partition_list {
                            if let Err(e) = partition.flush().await {
                                log::error!("Failed to flush partition {}: {}", partition.id(), e);
                            }
                        }
                        
                        // Run maintenance tasks
                        Self::run_maintenance(&partitions, &config.storage).await;
                    }
                    _ = shutdown_signal.notified() => {
                        log::info!("Background task shutting down");
                        break;
                    }
                }
            }
        });
        
        self.background_handle = Some(handle);
    }
    
    /// Run maintenance tasks
    async fn run_maintenance(
        partitions: &Arc<RwLock<HashMap<String, Arc<Partition>>>>, // Updated parameter type
        config: &StorageConfig,
    ) {
        let partition_list = {
            partitions.read().await.values().cloned().collect::<Vec<_>>()
        };
        
        for partition in partition_list {
            // Rotate blocks if needed
            if let Err(e) = partition.check_rotation().await {
                log::error!("Failed to check rotation for partition {}: {}", partition.id(), e);
            }
            
            // Apply retention policy
            if config.retention_days > 0 {
                if let Err(e) = partition.apply_retention(config.retention_days).await {
                    log::error!("Failed to apply retention for partition {}: {}", partition.id(), e);
                }
            }
        }
    }
    
    /// Create a new partition
    pub async fn create_partition(&self, name: &str) -> Result<String> {
        let partition_id = Uuid::new_v4().to_string();
        let partition_dir = self.config.data_dir.join(&partition_id);
        
        let partition = Partition::create(
            partition_id.clone(),
            name.to_string(),
            partition_dir,
            self.config.storage.clone(),
        ).await?;
        
        self.partitions.write().await.insert(partition_id.clone(), Arc::new(partition));
        
        log::info!("Created partition: {} ({})", name, partition_id);
        Ok(partition_id)
    }
    
    /// List all partitions
    pub async fn list_partitions(&self) -> Result<Vec<PartitionInfo>> {
        let partitions = self.partitions.read().await;
        let mut infos = Vec::new();
        
        for partition in partitions.values() {
            infos.push(partition.info().await?);
        }
        
        Ok(infos)
    }
    
    /// Get partition information
    pub async fn get_partition(&self, id: &str) -> Result<PartitionInfo> {
        let partitions = self.partitions.read().await;
        let partition = partitions
            .get(id)
            .ok_or_else(|| Error::PartitionNotFound(id.to_string()))?;
        
        partition.info().await
    }
    
    /// Append a log entry to a partition
    pub async fn append(&self, partition_id: &str, entry: LogEntry) -> Result<u64> {
        let partitions = self.partitions.read().await;
        let partition = partitions
            .get(partition_id)
            .ok_or_else(|| Error::PartitionNotFound(partition_id.to_string()))?;
        
        partition.append(entry).await
    }
    
    /// Get a specific log entry
    pub async fn get_entry(&self, partition_id: &str, entry_id: u64) -> Result<LogEntry> {
        let partitions = self.partitions.read().await;
        let partition = partitions
            .get(partition_id)
            .ok_or_else(|| Error::PartitionNotFound(partition_id.to_string()))?;
        
        partition.get_entry(entry_id).await
    }
    
    /// Flush all pending writes
    pub async fn flush(&self) -> Result<()> {
        let partitions = self.partitions.read().await;
        
        for partition in partitions.values() {
            partition.flush().await?;
        }
        
        Ok(())
    }
    
    /// Close the storage engine
    pub async fn close(mut self) -> Result<()> {
        // Signal shutdown
        self.shutdown_signal.notify_one();
        
        // Wait for background tasks to finish
        if let Some(handle) = self.background_handle.take() {
            let _ = handle.await;
        }
        
        // Flush all partitions
        self.flush().await?;
        
        log::info!("Storage engine closed");
        Ok(())
    }
    
    /// Get partition for queries (now async)
    pub async fn get_partition_for_query(&self, id: &str) -> Option<Arc<Partition>> {
        let partitions = self.partitions.read().await;
        partitions.get(id).cloned()
    }
    
    /// Get all partitions for queries (now async)
    pub async fn get_all_partitions_for_query(&self) -> Vec<Arc<Partition>> {
        let partitions = self.partitions.read().await;
        partitions.values().cloned().collect()
    }
}