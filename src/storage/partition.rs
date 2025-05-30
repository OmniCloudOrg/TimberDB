use crate::{Error, Result, StorageConfig};
use super::{Block, LogEntry};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionMetadata {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub block_ids: Vec<String>,
    pub active_block_id: Option<String>,
    pub total_entries: u64,
    pub total_size: u64,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub total_entries: u64,
    pub total_size: u64,
    pub block_count: usize,
    pub active_block_id: Option<String>,
}

pub struct Partition {
    metadata: RwLock<PartitionMetadata>,
    path: PathBuf,
    config: StorageConfig,
    active_block: RwLock<Option<Arc<Block>>>,
    sealed_blocks: RwLock<HashMap<String, Arc<Block>>>,
    next_entry_id: RwLock<u64>,
    entry_mapping: RwLock<HashMap<u64, (String, u64)>>,
}

impl Partition {
    pub async fn create(
        id: String,
        name: String,
        path: PathBuf,
        config: StorageConfig,
    ) -> Result<Self> {
        fs::create_dir_all(&path).await?;
        
        let metadata = PartitionMetadata {
            id: id.clone(),
            name,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            block_ids: Vec::new(),
            active_block_id: None,
            total_entries: 0,
            total_size: 0,
            version: 1,
        };
        
        let partition = Partition {
            metadata: RwLock::new(metadata),
            path,
            config,
            active_block: RwLock::new(None),
            sealed_blocks: RwLock::new(HashMap::new()),
            next_entry_id: RwLock::new(0),
            entry_mapping: RwLock::new(HashMap::new()),
        };
        
        partition.save_metadata().await?;
        
        Ok(partition)
    }
    
    pub async fn load(path: PathBuf, config: StorageConfig) -> Result<Self> {
        let metadata_path = path.join("partition.meta");
        
        if !metadata_path.exists() {
            return Err(Error::PartitionNotFound(format!("Metadata file not found: {:?}", metadata_path)));
        }
        
        let metadata = Self::load_metadata(&metadata_path).await?;
        
        let partition = Partition {
            metadata: RwLock::new(metadata.clone()),
            path,
            config,
            active_block: RwLock::new(None),
            sealed_blocks: RwLock::new(HashMap::new()),
            next_entry_id: RwLock::new(metadata.total_entries),
            entry_mapping: RwLock::new(HashMap::new()),
        };
        
        if let Some(active_block_id) = &metadata.active_block_id {
            if let Err(e) = partition.load_active_block(active_block_id).await {
                log::warn!("Failed to load active block {}: {}", active_block_id, e);
            }
        }
        
        partition.build_entry_mapping().await?;
        
        Ok(partition)
    }
    
    async fn load_metadata(path: &PathBuf) -> Result<PartitionMetadata> {
        let mut file = File::open(path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;
        
        if contents.is_empty() {
            return Err(Error::Corruption("Empty metadata file".to_string()));
        }
        
        let metadata: PartitionMetadata = bincode::deserialize(&contents)
            .map_err(|e| Error::Corruption(format!("Failed to deserialize metadata: {}", e)))?;
        
        Ok(metadata)
    }
    
    async fn save_metadata(&self) -> Result<()> {
        let metadata = {
            let metadata_guard = self.metadata.read().await;
            metadata_guard.clone()
        };
        
        let metadata_path = self.path.join("partition.meta");
        let temp_path = self.path.join("partition.meta.tmp");
        
        let contents = bincode::serialize(&metadata)?;
        
        let mut file = File::create(&temp_path).await?;
        file.write_all(&contents).await?;
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        
        tokio::fs::rename(&temp_path, &metadata_path).await?;
        
        Ok(())
    }
    
    async fn load_active_block(&self, block_id: &str) -> Result<()> {
        let block_path = self.path.join(format!("{}.block", block_id));
        
        if !block_path.exists() {
            return Err(Error::BlockNotFound(block_id.to_string()));
        }
        
        let block = Block::open(block_path, self.config.clone()).await?;
        let block_metadata = block.metadata().await;
        
        if block_metadata.status == super::block::BlockStatus::Active {
            *self.active_block.write().await = Some(Arc::new(block));
        }
        
        Ok(())
    }
    
    async fn build_entry_mapping(&self) -> Result<()> {
        let mut mapping = HashMap::new();
        let mut global_entry_id = 0u64;
        
        let metadata = {
            let metadata_guard = self.metadata.read().await;
            metadata_guard.clone()
        };
        
        for block_id in &metadata.block_ids {
            let block_path = self.path.join(format!("{}.block", block_id));
            
            if block_path.exists() {
                match Block::open(block_path, self.config.clone()).await {
                    Ok(block) => {
                        let block_metadata = block.metadata().await;
                        
                        for local_entry_id in 0..block_metadata.entry_count {
                            mapping.insert(global_entry_id, (block_id.clone(), local_entry_id));
                            global_entry_id += 1;
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to load block {} for entry mapping: {}", block_id, e);
                    }
                }
            }
        }
        
        *self.entry_mapping.write().await = mapping;
        *self.next_entry_id.write().await = global_entry_id;
        
        Ok(())
    }
    
    async fn get_or_create_active_block(&self) -> Result<Arc<Block>> {
        {
            let active_block = self.active_block.read().await;
            if let Some(ref block) = *active_block {
                let should_rotate = block.should_rotate(&self.config.block_rotation).await;
                if !should_rotate {
                    return Ok(block.clone());
                }
            }
        }
        
        self.rotate_block().await
    }
    
    async fn rotate_block(&self) -> Result<Arc<Block>> {
        {
            let mut active_block = self.active_block.write().await;
            if let Some(ref block) = *active_block {
                if let Err(e) = block.seal().await {
                    log::warn!("Failed to seal block during rotation: {}", e);
                } else {
                    let block_metadata = block.metadata().await;
                    self.sealed_blocks.write().await.insert(block_metadata.id.clone(), block.clone());
                }
            }
            *active_block = None;
        }
        
        let block_id = Uuid::new_v4().to_string();
        let block_path = self.path.join(format!("{}.block", block_id));
        
        let block = Block::create(block_id.clone(), block_path, self.config.clone()).await?;
        let block_arc = Arc::new(block);
        
        {
            let mut metadata = self.metadata.write().await;
            metadata.block_ids.push(block_id.clone());
            metadata.active_block_id = Some(block_id.clone());
            metadata.modified_at = Utc::now();
        }
        
        self.save_metadata().await?;
        
        *self.active_block.write().await = Some(block_arc.clone());
        
        log::info!("Rotated to new block: {}", block_id);
        
        Ok(block_arc)
    }
    
    pub async fn append(&self, entry: LogEntry) -> Result<u64> {
        let block = self.get_or_create_active_block().await?;
        
        let local_entry_id = block.append(entry).await?;
        
        let global_entry_id = {
            let mut next_id = self.next_entry_id.write().await;
            let id = *next_id;
            *next_id += 1;
            id
        };
        
        {
            let block_metadata = block.metadata().await;
            self.entry_mapping.write().await.insert(
                global_entry_id,
                (block_metadata.id, local_entry_id),
            );
        }
        
        {
            let mut metadata = self.metadata.write().await;
            metadata.total_entries += 1;
            let block_metadata = block.metadata().await;
            metadata.total_size = block_metadata.uncompressed_size;
            metadata.modified_at = Utc::now();
        }
        
        self.save_metadata().await?;
        
        Ok(global_entry_id)
    }
    
    pub async fn get_entry(&self, entry_id: u64) -> Result<LogEntry> {
        let (block_id, local_entry_id) = {
            let mapping = self.entry_mapping.read().await;
            mapping
                .get(&entry_id)
                .cloned()
                .ok_or_else(|| Error::EntryNotFound(self.id().to_string(), entry_id))?
        };
        
        {
            let active_block = self.active_block.read().await;
            if let Some(ref block) = *active_block {
                let block_metadata = block.metadata().await;
                if block_metadata.id == block_id {
                    return block.read_entry(local_entry_id).await;
                }
            }
        }
        
        {
            let sealed_blocks = self.sealed_blocks.read().await;
            if let Some(block) = sealed_blocks.get(&block_id) {
                return block.read_entry(local_entry_id).await;
            }
        }
        
        let block_path = self.path.join(format!("{}.block", block_id));
        let block = Block::open(block_path, self.config.clone()).await?;
        let entry = block.read_entry(local_entry_id).await?;
        
        self.sealed_blocks.write().await.insert(block_id, Arc::new(block));
        
        Ok(entry)
    }
    
    pub async fn get_entries_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<LogEntry>> {
        let mut results = Vec::new();
        let metadata = {
            let metadata_guard = self.metadata.read().await;
            metadata_guard.clone()
        };
        
        for block_id in metadata.block_ids.iter().rev() {
            if let Some(limit) = limit {
                if results.len() >= limit {
                    break;
                }
            }
            
            let block = {
                let active_block = self.active_block.read().await;
                if let Some(ref active) = *active_block {
                    let active_metadata = active.metadata().await;
                    if active_metadata.id == *block_id {
                        active.clone()
                    } else {
                        match self.load_block_for_search(block_id).await {
                            Ok(block) => block,
                            Err(_) => continue,
                        }
                    }
                } else {
                    match self.load_block_for_search(block_id).await {
                        Ok(block) => block,
                        Err(_) => continue,
                    }
                }
            };
            
            let block_metadata = block.metadata().await;
            for local_entry_id in (0..block_metadata.entry_count).rev() {
                if let Some(limit) = limit {
                    if results.len() >= limit {
                        break;
                    }
                }
                
                match block.read_entry(local_entry_id).await {
                    Ok(entry) => {
                        if entry.timestamp >= start && entry.timestamp <= end {
                            results.push(entry);
                        } else if entry.timestamp < start {
                            break;
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
        
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        Ok(results)
    }
    
    async fn load_block_for_search(&self, block_id: &str) -> Result<Arc<Block>> {
        {
            let sealed_blocks = self.sealed_blocks.read().await;
            if let Some(block) = sealed_blocks.get(block_id) {
                return Ok(block.clone());
            }
        }
        
        let block_path = self.path.join(format!("{}.block", block_id));
        let block = Block::open(block_path, self.config.clone()).await?;
        let block_arc = Arc::new(block);
        
        self.sealed_blocks.write().await.insert(block_id.to_string(), block_arc.clone());
        
        Ok(block_arc)
    }
    
    pub async fn info(&self) -> Result<PartitionInfo> {
        let metadata = {
            let metadata_guard = self.metadata.read().await;
            metadata_guard.clone()
        };
        
        Ok(PartitionInfo {
            id: metadata.id.clone(),
            name: metadata.name.clone(),
            created_at: metadata.created_at,
            modified_at: metadata.modified_at,
            total_entries: metadata.total_entries,
            total_size: metadata.total_size,
            block_count: metadata.block_ids.len(),
            active_block_id: metadata.active_block_id.clone(),
        })
    }
    
    pub fn id(&self) -> String {
        self.metadata.try_read()
            .map(|m| m.id.clone())
            .unwrap_or_else(|_| "unknown".to_string())
    }
    
    pub async fn check_rotation(&self) -> Result<()> {
        let active_block = self.active_block.read().await;
        if let Some(ref block) = *active_block {
            let should_rotate = block.should_rotate(&self.config.block_rotation).await;
            if should_rotate {
                drop(active_block);
                self.rotate_block().await?;
            }
        }
        Ok(())
    }
    
    pub async fn apply_retention(&self, retention_days: u32) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        let mut blocks_to_remove = Vec::new();
        
        let metadata = {
            let metadata_guard = self.metadata.read().await;
            metadata_guard.clone()
        };
        
        for block_id in &metadata.block_ids {
            if Some(block_id) == metadata.active_block_id.as_ref() {
                continue;
            }
            
            let block_path = self.path.join(format!("{}.block", block_id));
            if let Ok(block) = Block::open(block_path, self.config.clone()).await {
                let block_metadata = block.metadata().await;
                
                if block_metadata.created_at < cutoff {
                    blocks_to_remove.push(block_id.clone());
                }
            }
        }
        
        for block_id in blocks_to_remove {
            self.remove_block(&block_id).await?;
        }
        
        Ok(())
    }
    
    async fn remove_block(&self, block_id: &str) -> Result<()> {
        self.sealed_blocks.write().await.remove(block_id);
        
        let block_path = self.path.join(format!("{}.block", block_id));
        if block_path.exists() {
            fs::remove_file(&block_path).await?;
        }
        
        {
            let mut metadata = self.metadata.write().await;
            metadata.block_ids.retain(|id| id != block_id);
            metadata.modified_at = Utc::now();
        }
        
        self.save_metadata().await?;
        self.build_entry_mapping().await?;
        
        log::info!("Removed block: {}", block_id);
        
        Ok(())
    }
    
    pub async fn flush(&self) -> Result<()> {
        let active_block = self.active_block.read().await;
        if let Some(ref block) = *active_block {
            block.flush().await?;
        }
        drop(active_block);
        
        self.save_metadata().await?;
        
        Ok(())
    }
}