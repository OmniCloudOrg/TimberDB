// TimberDB: A high-performance distributed log database
// storage/manager.rs - Storage management for blocks and partitions

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::thread;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::config::StorageConfig;
use crate::storage::block::{Block, BlockMetadata, BlockStatus, LogEntry};

// Storage manager errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Block error: {0}")]
    Block(#[from] crate::storage::block::BlockError),
    
    #[error("Partition not found: {0}")]
    PartitionNotFound(String),
    
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    #[error("Storage limit exceeded")]
    StorageLimitExceeded,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

// Partition metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionMetadata {
    /// Partition ID
    pub id: String,
    /// Partition name
    pub name: String,
    /// Partition creation time
    pub created_at: DateTime<Utc>,
    /// Blocks in this partition
    pub blocks: Vec<String>,
    /// Active block ID
    pub active_block: Option<String>,
    /// Total entry count
    pub entry_count: u64,
    /// Total size in bytes
    pub size_bytes: u64,
}

// Storage manager for handling blocks and partitions
#[derive(Debug)]
pub struct StorageManager {
    /// Base directory for all storage
    data_dir: PathBuf,
    /// Configuration
    config: StorageConfig,
    /// Partitions by ID
    partitions: RwLock<HashMap<String, PartitionMetadata>>,
    /// Active blocks by partition ID
    active_blocks: RwLock<HashMap<String, Arc<Mutex<Block>>>>,
    /// Block metadata cache
    block_metadata: RwLock<HashMap<String, BlockMetadata>>,
    /// Background task handle
    maintenance_handle: Option<thread::JoinHandle<()>>,
    /// Signal to stop background tasks
    shutdown: Arc<RwLock<bool>>,
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(config: StorageConfig) -> Result<Self, StorageError> {
        // Validate configuration
        if !config.data_dir.exists() {
            fs::create_dir_all(&config.data_dir)?;
        }
        
        // Initialize storage manager
        let manager = StorageManager {
            data_dir: config.data_dir.clone(),
            config,
            partitions: RwLock::new(HashMap::new()),
            active_blocks: RwLock::new(HashMap::new()),
            block_metadata: RwLock::new(HashMap::new()),
            maintenance_handle: None,
            shutdown: Arc::new(RwLock::new(false)),
        };
        
        // Load existing partitions
        manager.load_partitions()?;
        
        // Start background maintenance
        manager.start_maintenance();
        
        Ok(manager)
    }
    
    /// Load existing partitions from disk
    fn load_partitions(&self) -> Result<(), StorageError> {
        log::info!("Loading partitions from {}", self.data_dir.display());
        
        // Find all partition directories
        let entries = fs::read_dir(&self.data_dir)?;
        let mut partitions = self.partitions.write().unwrap();
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Try to load partition metadata
                if let Some(partition_id) = path.file_name().and_then(|n| n.to_str()) {
                    let metadata_path = path.join("partition.meta");
                    
                    if metadata_path.exists() {
                        let metadata_bytes = fs::read(&metadata_path)?;
                        let metadata: PartitionMetadata = bincode::deserialize(&metadata_bytes)
                            .map_err(|e| {
                                StorageError::InvalidConfig(format!(
                                    "Failed to deserialize partition metadata: {}",
                                    e
                                ))
                            })?;
                        
                        partitions.insert(partition_id.to_string(), metadata.clone());
                        
                        // Load active block if any
                        if let Some(active_block_id) = &metadata.active_block {
                            let block_path = path.join(format!("{}.block", active_block_id));
                            if block_path.exists() {
                                let block = Block::open(block_path)?;
                                
                                if block.status() == BlockStatus::Active {
                                    let mut active_blocks = self.active_blocks.write().unwrap();
                                    active_blocks.insert(
                                        partition_id.to_string(),
                                        Arc::new(Mutex::new(block)),
                                    );
                                }
                            }
                        }
                        
                        // Load all block metadata
                        for block_id in &metadata.blocks {
                            self.load_block_metadata(partition_id, block_id)?;
                        }
                    }
                }
            }
        }
        
        log::info!("Loaded {} partitions", partitions.len());
        
        Ok(())
    }
    
    /// Load block metadata into cache
    fn load_block_metadata(
        &self,
        partition_id: &str,
        block_id: &str,
    ) -> Result<(), StorageError> {
        let partition_dir = self.data_dir.join(partition_id);
        
        // Try different file extensions
        let extensions = ["block", "block.archive", "meta"];
        
        for ext in &extensions {
            let path = partition_dir.join(format!("{}.{}", block_id, ext));
            
            if path.exists() {
                let block = Block::open(path)?;
                let metadata = block.metadata().clone();
                
                let mut block_metadata = self.block_metadata.write().unwrap();
                block_metadata.insert(format!("{}:{}", partition_id, block_id), metadata);
                
                return Ok(());
            }
        }
        
        Err(StorageError::BlockNotFound(format!(
            "Block not found: {}:{}",
            partition_id, block_id
        )))
    }
    
    /// Start background maintenance tasks
    fn start_maintenance(&self) {
        let config = self.config.clone();
        let data_dir = self.data_dir.clone();
        let shutdown = self.shutdown.clone();
        
        let handle = thread::spawn(move || {
            let mut next_housekeeping = Instant::now();
            
            while !*shutdown.read().unwrap() {
                // Sleep for a bit
                thread::sleep(Duration::from_secs(1));
                
                // Check if it's time for housekeeping
                if Instant::now() >= next_housekeeping {
                    // Perform maintenance tasks
                    if let Err(e) = Self::maintenance_task(&data_dir, &config) {
                        log::error!("Maintenance task error: {}", e);
                    }
                    
                    // Schedule next run (every 15 minutes)
                    next_housekeeping = Instant::now() + Duration::from_secs(15 * 60);
                }
            }
        });
        
        // Store the handle
        let this = self as *const _ as *mut StorageManager;
        unsafe {
            (*this).maintenance_handle = Some(handle);
        }
    }
    
    /// Background maintenance tasks
    fn maintenance_task(
        data_dir: &Path,
        config: &StorageConfig,
    ) -> Result<(), StorageError> {
        log::info!("Running maintenance tasks");
        
        // Archive old blocks
        Self::archive_old_blocks(data_dir, config)?;
        
        // Apply retention policy
        Self::apply_retention_policy(data_dir, config)?;
        
        // Enforce storage limits
        Self::enforce_storage_limits(data_dir, config)?;
        
        log::info!("Maintenance tasks completed");
        
        Ok(())
    }
    
    /// Archive old blocks to save space
    fn archive_old_blocks(
        data_dir: &Path,
        config: &StorageConfig,
    ) -> Result<(), StorageError> {
        // Find all partition directories
        let entries = fs::read_dir(data_dir)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Find all block files
                let block_entries = fs::read_dir(&path)?;
                
                for block_entry in block_entries {
                    let block_entry = block_entry?;
                    let block_path = block_entry.path();
                    
                    // Only process .block files that are not active
                    if block_path.extension().and_then(|e| e.to_str()) == Some("block") {
                        // Skip active blocks
                        if let Ok(block) = Block::open(block_path.clone()) {
                            if block.status() == BlockStatus::Sealed {
                                // Archive the block
                                log::info!("Archiving block: {}", block_path.display());
                                let mut block = block;
                                block.archive(config.compression_level)?;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Apply retention policy and delete old blocks
    fn apply_retention_policy(
        data_dir: &Path,
        config: &StorageConfig,
    ) -> Result<(), StorageError> {
        // Skip if retention is disabled
        if config.retention_days == 0 {
            return Ok(());
        }
        
        let now = Utc::now();
        let retention_threshold = now - chrono::Duration::days(config.retention_days as i64);
        
        log::info!(
            "Applying retention policy: deleting blocks older than {}",
            retention_threshold
        );
        
        // Find all partition directories
        let entries = fs::read_dir(data_dir)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Find all block files
                let block_entries = fs::read_dir(&path)?;
                
                for block_entry in block_entries {
                    let block_entry = block_entry?;
                    let block_path = block_entry.path();
                    
                    // Process block files and metadata
                    let extensions = ["block", "block.archive", "meta"];
                    if let Some(ext) = block_path.extension().and_then(|e| e.to_str()) {
                        if extensions.contains(&ext) {
                            // Open the block to check its metadata
                            if let Ok(block) = Block::open(block_path.clone()) {
                                let metadata = block.metadata();
                                
                                // Check if the block is older than retention threshold
                                if metadata.created_at < retention_threshold
                                    && metadata.marked_for_deletion
                                {
                                    // Delete the block
                                    log::info!("Deleting old block: {}", block_path.display());
                                    block.delete()?;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Enforce storage limits
    fn enforce_storage_limits(
        data_dir: &Path,
        config: &StorageConfig,
    ) -> Result<(), StorageError> {
        // Skip if no storage limit
        if config.max_storage_size == 0 {
            return Ok(());
        }
        
        // Calculate current storage size
        let mut total_size = 0u64;
        let mut blocks_info = Vec::new();
        
        // Find all partition directories
        let entries = fs::read_dir(data_dir)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Find all block files
                let block_entries = fs::read_dir(&path)?;
                
                for block_entry in block_entries {
                    let block_entry = block_entry?;
                    let block_path = block_entry.path();
                    let metadata = fs::metadata(&block_path)?;
                    
                    total_size += metadata.len();
                    
                    // Only collect info for actual block files
                    let extensions = ["block", "block.archive"];
                    if let Some(ext) = block_path.extension().and_then(|e| e.to_str()) {
                        if extensions.contains(&ext) {
                            if let Ok(block) = Block::open(block_path.clone()) {
                                let block_metadata = block.metadata().clone();
                                blocks_info.push((block_path.clone(), block_metadata));
                            }
                        }
                    }
                }
            }
        }
        
        // Check if we need to free up space
        if total_size > config.max_storage_size {
            log::warn!(
                "Storage limit exceeded: {} bytes used, {} bytes limit",
                total_size,
                config.max_storage_size
            );
            
            // Sort blocks by creation time (oldest first)
            blocks_info.sort_by(|a, b| a.1.created_at.cmp(&b.1.created_at));
            
            // Mark blocks for deletion until we're under the limit
            let mut size_to_free = total_size - config.max_storage_size;
            let mut freed = 0u64;
            
            for (path, _) in &blocks_info {
                if freed >= size_to_free {
                    break;
                }
                
                let metadata = fs::metadata(path)?;
                freed += metadata.len();
                
                // Mark this block for deletion
                log::info!("Marking block for deletion: {}", path.display());
                
                let mut block = Block::open(path.clone())?;
                block.mark_for_deletion()?;
            }
        }
        
        Ok(())
    }
    
    /// Create a new partition
    pub fn create_partition(&self, name: &str) -> Result<String, StorageError> {
        let partition_id = Uuid::new_v4().to_string();
        let partition_dir = self.data_dir.join(&partition_id);
        
        // Create partition directory
        fs::create_dir_all(&partition_dir)?;
        
        // Create partition metadata
        let metadata = PartitionMetadata {
            id: partition_id.clone(),
            name: name.to_string(),
            created_at: Utc::now(),
            blocks: Vec::new(),
            active_block: None,
            entry_count: 0,
            size_bytes: 0,
        };
        
        // Save metadata
        let metadata_path = partition_dir.join("partition.meta");
        let metadata_bytes = bincode::serialize(&metadata).map_err(|e| {
            StorageError::InvalidConfig(format!("Failed to serialize partition metadata: {}", e))
        })?;
        fs::write(&metadata_path, metadata_bytes)?;
        
        // Update in-memory state
        self.partitions
            .write()
            .unwrap()
            .insert(partition_id.clone(), metadata);
        
        log::info!("Created partition: {} ({})", name, partition_id);
        
        Ok(partition_id)
    }
    
    /// Get partition metadata
    pub fn get_partition(&self, partition_id: &str) -> Result<PartitionMetadata, StorageError> {
        self.partitions
            .read()
            .unwrap()
            .get(partition_id)
            .cloned()
            .ok_or_else(|| StorageError::PartitionNotFound(partition_id.to_string()))
    }
    
    /// List all partitions
    pub fn list_partitions(&self) -> Vec<PartitionMetadata> {
        self.partitions
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }
    
    /// Get active block for a partition
    fn get_active_block(
        &self,
        partition_id: &str,
    ) -> Result<Arc<Mutex<Block>>, StorageError> {
        // Check if we already have an active block
        {
            let active_blocks = self.active_blocks.read().unwrap();
            if let Some(block) = active_blocks.get(partition_id) {
                return Ok(block.clone());
            }
        }
        
        // No active block, create a new one
        let block_id = Uuid::new_v4().to_string();
        let partition_dir = self.data_dir.join(partition_id);
        
        // Create new block
        let block = Block::create(
            block_id.clone(),
            &partition_dir,
            self.config.compression,
        )?;
        
        // Update partition metadata
        {
            let mut partitions = self.partitions.write().unwrap();
            if let Some(metadata) = partitions.get_mut(partition_id) {
                metadata.blocks.push(block_id.clone());
                metadata.active_block = Some(block_id.clone());
                
                // Save updated metadata
                let metadata_path = partition_dir.join("partition.meta");
                let metadata_bytes = bincode::serialize(&metadata).map_err(|e| {
                    StorageError::InvalidConfig(format!(
                        "Failed to serialize partition metadata: {}",
                        e
                    ))
                })?;
                fs::write(&metadata_path, metadata_bytes)?;
            } else {
                return Err(StorageError::PartitionNotFound(partition_id.to_string()));
            }
        }
        
        // Store block in active blocks map
        let block_arc = Arc::new(Mutex::new(block));
        self.active_blocks
            .write()
            .unwrap()
            .insert(partition_id.to_string(), block_arc.clone());
        
        Ok(block_arc)
    }
    
    /// Append a log entry to a partition
    pub fn append(
        &self,
        partition_id: &str,
        entry: LogEntry,
    ) -> Result<u64, StorageError> {
        // Get the active block
        let block_arc = self.get_active_block(partition_id)?;
        let mut block = block_arc.lock().unwrap();
        
        // Check if block should be rotated
        if block.should_rotate(&self.config.block_rotation) {
            // Seal the current block
            block.seal()?;
            
            // Create a new active block
            let block_id = Uuid::new_v4().to_string();
            let partition_dir = self.data_dir.join(partition_id);
            
            let new_block = Block::create(
                block_id.clone(),
                &partition_dir,
                self.config.compression,
            )?;
            
            // Update partition metadata
            {
                let mut partitions = self.partitions.write().unwrap();
                if let Some(metadata) = partitions.get_mut(partition_id) {
                    metadata.blocks.push(block_id.clone());
                    metadata.active_block = Some(block_id.clone());
                    
                    // Save updated metadata
                    let metadata_path = partition_dir.join("partition.meta");
                    let metadata_bytes = bincode::serialize(&metadata).map_err(|e| {
                        StorageError::InvalidConfig(format!(
                            "Failed to serialize partition metadata: {}",
                            e
                        ))
                    })?;
                    fs::write(&metadata_path, metadata_bytes)?;
                }
            }
            
            // Replace block in map
            *block = new_block;
        }
        
        // Append the entry
        let entry_id = block.append(entry)?;
        
        // Update partition metadata
        {
            let mut partitions = self.partitions.write().unwrap();
            if let Some(metadata) = partitions.get_mut(partition_id) {
                metadata.entry_count += 1;
                metadata.size_bytes += block.metadata().uncompressed_size;
                
                // Save updated metadata
                let metadata_path = self.data_dir.join(partition_id).join("partition.meta");
                let metadata_bytes = bincode::serialize(&metadata).map_err(|e| {
                    StorageError::InvalidConfig(format!(
                        "Failed to serialize partition metadata: {}",
                        e
                    ))
                })?;
                fs::write(&metadata_path, metadata_bytes)?;
            }
        }
        
        Ok(entry_id)
    }
    
    /// Read a log entry from a partition
    pub fn read_entry(
        &self,
        partition_id: &str,
        block_id: &str,
        entry_id: u64,
    ) -> Result<LogEntry, StorageError> {
        // Check active blocks first
        {
            let active_blocks = self.active_blocks.read().unwrap();
            if let Some(block_arc) = active_blocks.get(partition_id) {
                let block = block_arc.lock().unwrap();
                if block.metadata().id == block_id {
                    return Ok(block.read_entry(entry_id)?);
                }
            }
        }
        
        // Open the block file
        let partition_dir = self.data_dir.join(partition_id);
        
        // Try different file extensions
        let extensions = ["block", "block.archive", "meta"];
        
        for ext in &extensions {
            let path = partition_dir.join(format!("{}.{}", block_id, ext));
            
            if path.exists() {
                let block = Block::open(path)?;
                return Ok(block.read_entry(entry_id)?);
            }
        }
        
        Err(StorageError::BlockNotFound(format!(
            "Block not found: {}:{}",
            partition_id, block_id
        )))
    }
    
    /// Shut down the storage manager
    pub fn shutdown(&mut self) -> Result<(), StorageError> {
        // Signal background tasks to stop
        *self.shutdown.write().unwrap() = true;
        
        // Wait for background tasks to finish
        if let Some(handle) = self.maintenance_handle.take() {
            if handle.join().is_err() {
                log::error!("Failed to join maintenance thread");
            }
        }
        
        // Seal all active blocks
        {
            let active_blocks = self.active_blocks.read().unwrap();
            for (partition_id, block_arc) in active_blocks.iter() {
                let mut block = block_arc.lock().unwrap();
                
                if block.status() == BlockStatus::Active {
                    if let Err(e) = block.seal() {
                        log::error!("Failed to seal block for partition {}: {}", partition_id, e);
                    }
                }
            }
        }
        
        Ok(())
    }
}

impl Drop for StorageManager {
    fn drop(&mut self) {
        // Signal background tasks to stop
        *self.shutdown.write().unwrap() = true;
        
        // Note: We don't join the thread here to avoid blocking,
        // TODO: We may want to handle this more gracefully in the future.
    }
}