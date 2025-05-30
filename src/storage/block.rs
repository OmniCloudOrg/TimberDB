use crate::{Error, Result, StorageConfig};
use super::{LogEntry, compression};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use uuid::Uuid;

const BLOCK_MAGIC: &[u8] = b"TMBRBLK2"; 
const BLOCK_VERSION: u32 = 2;
const MAX_ENTRIES_PER_BLOCK: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockStatus {
    Active,
    Sealed,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMetadata {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub sealed_at: Option<DateTime<Utc>>,
    pub status: BlockStatus,
    pub entry_count: u64,
    pub uncompressed_size: u64,
    pub compressed_size: Option<u64>,
    pub checksum: u32,
    pub compression_algorithm: crate::CompressionAlgorithm,
    pub compression_level: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct BlockHeader {
    magic: [u8; 8],
    version: u32,
    metadata: BlockMetadata,
    data_offset: u64,
}

impl BlockHeader {
    fn new(metadata: BlockMetadata) -> Self {
        BlockHeader {
            magic: BLOCK_MAGIC.try_into().unwrap(),
            version: BLOCK_VERSION,
            metadata,
            data_offset: 0,
        }
    }
    
    fn validate(&self) -> Result<()> {
        if self.magic != BLOCK_MAGIC {
            return Err(Error::InvalidFormat("Invalid block magic bytes".to_string()));
        }
        
        if self.version != BLOCK_VERSION {
            return Err(Error::InvalidFormat(format!(
                "Unsupported block version: {} (expected: {})",
                self.version, BLOCK_VERSION
            )));
        }
        
        if self.metadata.entry_count > MAX_ENTRIES_PER_BLOCK {
            return Err(Error::InvalidFormat(format!(
                "Too many entries in block: {} (max: {})",
                self.metadata.entry_count, MAX_ENTRIES_PER_BLOCK
            )));
        }
        
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct EntryHeader {
    size: u32,
    checksum: u32,
    timestamp: DateTime<Utc>,
}

pub struct Block {
    metadata: Mutex<BlockMetadata>,
    path: PathBuf,
    file: Mutex<Option<File>>,
    config: StorageConfig,
    entry_index: Mutex<Vec<u64>>,
}

impl Block {
    pub async fn create(
        id: String,
        path: PathBuf,
        config: StorageConfig,
    ) -> Result<Self> {
        if id.is_empty() || id.len() > 255 {
            return Err(Error::InvalidFormat("Invalid block ID".to_string()));
        }
        
        let metadata = BlockMetadata {
            id: id.clone(),
            created_at: Utc::now(),
            sealed_at: None,
            status: BlockStatus::Active,
            entry_count: 0,
            uncompressed_size: 0,
            compressed_size: None,
            checksum: 0,
            compression_algorithm: config.compression.algorithm,
            compression_level: config.compression.level,
        };
        
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await?;
        
        let block = Block {
            metadata: Mutex::new(metadata),
            path,
            file: Mutex::new(Some(file)),
            config,
            entry_index: Mutex::new(Vec::new()),
        };
        
        block.write_header().await?;
        
        Ok(block)
    }
    
    pub async fn open(path: PathBuf, config: StorageConfig) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .await?;
        
        let file_size = file.metadata().await?.len();
        if file_size < 32 {
            return Err(Error::InvalidFormat("Block file too small".to_string()));
        }
        
        let header = Self::read_header(&mut file).await?;
        header.validate()?;
        
        let entry_index = if header.metadata.status == BlockStatus::Active {
            Self::build_entry_index(&mut file, &header).await?
        } else {
            Vec::new()
        };
        
        let file_handle = if header.metadata.status == BlockStatus::Active {
            Some(file)
        } else {
            None
        };
        
        Ok(Block {
            metadata: Mutex::new(header.metadata),
            path,
            file: Mutex::new(file_handle),
            config,
            entry_index: Mutex::new(entry_index),
        })
    }
    
    async fn write_header(&self) -> Result<()> {
        let metadata = {
            let metadata_guard = self.metadata.lock().await;
            metadata_guard.clone()
        };

        let mut header = BlockHeader::new(metadata);
        
        let header_size = bincode::serialized_size(&header)?;
        header.data_offset = header_size;
        
        let mut file_guard = self.file.lock().await;
        let file = file_guard.as_mut().ok_or_else(|| {
            Error::Storage("Block file not available for writing".to_string())
        })?;
        
        file.seek(SeekFrom::Start(0)).await?;
        let header_bytes = bincode::serialize(&header)?;
        file.write_all(&header_bytes).await?;
        
        if self.config.sync_writes {
            file.flush().await?;
        }
        
        Ok(())
    }
    
    async fn read_header(file: &mut File) -> Result<BlockHeader> {
        file.seek(SeekFrom::Start(0)).await?;
        
        let mut magic = [0u8; 8];
        if file.read_exact(&mut magic).await.is_err() {
            return Err(Error::InvalidFormat("Failed to read block magic bytes".to_string()));
        }
        
        if magic != BLOCK_MAGIC {
            return Err(Error::InvalidFormat("Invalid block magic bytes".to_string()));
        }
        
        let mut version_bytes = [0u8; 4];
        if file.read_exact(&mut version_bytes).await.is_err() {
            return Err(Error::InvalidFormat("Failed to read block version".to_string()));
        }
        
        let version = u32::from_le_bytes(version_bytes);
        if version != BLOCK_VERSION {
            return Err(Error::InvalidFormat(format!(
                "Unsupported block version: {}",
                version
            )));
        }
        
        file.seek(SeekFrom::Start(0)).await?;
        
        let file_size = file.metadata().await?.len();
        let read_size = std::cmp::min(file_size, 8192) as usize;
        
        let mut header_bytes = vec![0u8; read_size];
        let bytes_read = file.read(&mut header_bytes).await?;
        header_bytes.truncate(bytes_read);
        
        if header_bytes.len() < 12 {
            return Err(Error::InvalidFormat("Block header too small".to_string()));
        }
        
        let header: BlockHeader = bincode::deserialize(&header_bytes)
            .map_err(|e| Error::InvalidFormat(format!("Failed to deserialize block header: {}", e)))?;
        
        Ok(header)
    }
    
    async fn build_entry_index(file: &mut File, header: &BlockHeader) -> Result<Vec<u64>> {
        let mut index = Vec::new();
        let mut offset = header.data_offset;
        
        file.seek(SeekFrom::Start(offset)).await?;
        
        for _ in 0..header.metadata.entry_count {
            index.push(offset);
            
            let entry_header_size = bincode::serialized_size(&EntryHeader {
                size: 0,
                checksum: 0,
                timestamp: Utc::now(),
            })?;
            
            let mut header_bytes = vec![0u8; entry_header_size as usize];
            if file.read_exact(&mut header_bytes).await.is_err() {
                break;
            }
            
            let entry_header: EntryHeader = match bincode::deserialize(&header_bytes) {
                Ok(header) => header,
                Err(_) => break,
            };
            
            offset += entry_header_size + entry_header.size as u64;
            if file.seek(SeekFrom::Start(offset)).await.is_err() {
                break;
            }
        }
        
        Ok(index)
    }
    
    pub async fn append(&self, mut entry: LogEntry) -> Result<u64> {
        entry.validate().map_err(|e| Error::InvalidFormat(e))?;
        
        let entry_id = {
            let mut metadata = self.metadata.lock().await;
            
            if metadata.status != BlockStatus::Active {
                return Err(Error::Storage("Cannot append to non-active block".to_string()));
            }
            
            if metadata.entry_count >= MAX_ENTRIES_PER_BLOCK {
                return Err(Error::ResourceLimit("Block has reached maximum entry count".to_string()));
            }
            
            let entry_id = metadata.entry_count;
            entry.set_entry_id(entry_id);
            entry_id
        };
        
        let entry_bytes = bincode::serialize(&entry)?;
        let entry_size = entry_bytes.len() as u32;
        let entry_checksum = crc32fast::hash(&entry_bytes);
        
        let entry_header = EntryHeader {
            size: entry_size,
            checksum: entry_checksum,
            timestamp: entry.timestamp,
        };
        let header_bytes = bincode::serialize(&entry_header)?;
        
        let offset = {
            let mut file_guard = self.file.lock().await;
            let file = file_guard.as_mut().ok_or_else(|| {
                Error::Storage("Block file not available for writing".to_string())
            })?;
            
            file.seek(SeekFrom::End(0)).await?;
            let offset = file.stream_position().await?;
            
            file.write_all(&header_bytes).await?;
            file.write_all(&entry_bytes).await?;
            
            if self.config.sync_writes {
                file.flush().await?;
            }
            
            offset
        };
        
        self.entry_index.lock().await.push(offset);
        
        {
            let mut metadata = self.metadata.lock().await;
            metadata.entry_count += 1;
            metadata.uncompressed_size += header_bytes.len() as u64 + entry_bytes.len() as u64;
        }
        
        self.write_header().await?;
        
        Ok(entry_id)
    }
    
    pub async fn read_entry(&self, entry_id: u64) -> Result<LogEntry> {
        let metadata = self.metadata.lock().await;
        
        if entry_id >= metadata.entry_count {
            return Err(Error::EntryNotFound("".to_string(), entry_id));
        }
        
        let offset = {
            let index = self.entry_index.lock().await;
            if entry_id as usize >= index.len() {
                drop(index);
                drop(metadata);
                return self.read_entry_from_disk(entry_id).await;
            }
            index[entry_id as usize]
        };
        
        drop(metadata);
        
        let mut file_guard = self.file.lock().await;
        if let Some(ref mut file) = *file_guard {
            self.read_entry_at_offset(file, offset).await
        } else {
            drop(file_guard);
            self.read_entry_from_disk(entry_id).await
        }
    }
    
    async fn read_entry_from_disk(&self, entry_id: u64) -> Result<LogEntry> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(&self.path)
            .await?;
        
        let header = Self::read_header(&mut file).await?;
        
        let mut offset = header.data_offset;
        file.seek(SeekFrom::Start(offset)).await?;
        
        for current_id in 0..=entry_id {
            if current_id == entry_id {
                return self.read_entry_at_offset(&mut file, offset).await;
            }
            
            let entry_header_size = bincode::serialized_size(&EntryHeader {
                size: 0,
                checksum: 0,
                timestamp: Utc::now(),
            })?;
            
            let mut header_bytes = vec![0u8; entry_header_size as usize];
            file.read_exact(&mut header_bytes).await?;
            
            let entry_header: EntryHeader = bincode::deserialize(&header_bytes)?;
            
            offset += entry_header_size + entry_header.size as u64;
            file.seek(SeekFrom::Start(offset)).await?;
        }
        
        Err(Error::EntryNotFound("".to_string(), entry_id))
    }
    
    async fn read_entry_at_offset(&self, file: &mut File, offset: u64) -> Result<LogEntry> {
        file.seek(SeekFrom::Start(offset)).await?;
        
        let entry_header_size = bincode::serialized_size(&EntryHeader {
            size: 0,
            checksum: 0,
            timestamp: Utc::now(),
        })?;
        
        let mut header_bytes = vec![0u8; entry_header_size as usize];
        file.read_exact(&mut header_bytes).await?;
        
        let entry_header: EntryHeader = bincode::deserialize(&header_bytes)?;
        
        let mut entry_bytes = vec![0u8; entry_header.size as usize];
        file.read_exact(&mut entry_bytes).await?;
        
        let checksum = crc32fast::hash(&entry_bytes);
        if checksum != entry_header.checksum {
            return Err(Error::Corruption(format!(
                "Entry checksum mismatch: expected {}, got {}",
                entry_header.checksum, checksum
            )));
        }
        
        let entry: LogEntry = bincode::deserialize(&entry_bytes)?;
        Ok(entry)
    }
    
    pub async fn seal(&self) -> Result<()> {
        let checksum = self.calculate_checksum().await?;
        
        {
            let mut metadata = self.metadata.lock().await;
            
            if metadata.status != BlockStatus::Active {
                return Err(Error::Storage("Block is not active".to_string()));
            }
            
            metadata.status = BlockStatus::Sealed;
            metadata.sealed_at = Some(Utc::now());
            metadata.checksum = checksum;
        }
        
        *self.file.lock().await = None;
        
        self.write_header_to_file().await?;
        
        Ok(())
    }
    
    async fn write_header_to_file(&self) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .open(&self.path)
            .await?;
        
        let metadata = {
            let metadata_guard = self.metadata.lock().await;
            metadata_guard.clone()
        };
        
        let mut header = BlockHeader::new(metadata);
        
        let header_size = bincode::serialized_size(&header)?;
        header.data_offset = header_size;
        
        file.seek(SeekFrom::Start(0)).await?;
        let header_bytes = bincode::serialize(&header)?;
        file.write_all(&header_bytes).await?;
        file.flush().await?;
        
        Ok(())
    }
    
    async fn calculate_checksum(&self) -> Result<u32> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(&self.path)
            .await?;
        
        let header = Self::read_header(&mut file).await?;
        
        file.seek(SeekFrom::Start(header.data_offset)).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        
        Ok(crc32fast::hash(&data))
    }
    
    pub async fn metadata(&self) -> BlockMetadata {
        let metadata_guard = self.metadata.lock().await;
        metadata_guard.clone()
    }
    
    pub fn path(&self) -> &Path {
        &self.path
    }
    
    pub async fn should_rotate(&self, policy: &crate::BlockRotationPolicy) -> bool {
        let metadata = self.metadata.lock().await;
        
        match policy {
            crate::BlockRotationPolicy::Time(duration) => {
                let age = Utc::now().signed_duration_since(metadata.created_at);
                age.to_std().map_or(false, |age| age >= *duration)
            }
            crate::BlockRotationPolicy::Size(max_size) => {
                metadata.uncompressed_size >= *max_size
            }
            crate::BlockRotationPolicy::Count(max_count) => {
                metadata.entry_count >= *max_count
            }
        }
    }
    
    pub async fn flush(&self) -> Result<()> {
        let mut file_guard = self.file.lock().await;
        if let Some(ref mut file) = *file_guard {
            file.flush().await?;
        }
        Ok(())
    }
}