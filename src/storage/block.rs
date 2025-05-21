// TimberDB: A high-performance distributed log database
// storage/block.rs - Block-based storage implementation

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{BlockRotationPolicy, CompressionAlgorithm};
use crate::storage::compression::{compress_data, decompress_data, CompressionError};
use crate::storage::index::{Index, IndexEntry, IndexError};

// Block-related errors
#[derive(Error, Debug)]
pub enum BlockError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Compression error: {0}")]
    Compression(String),
    
    #[error("Index error: {0}")]
    Index(#[from] IndexError),
    
    #[error("Invalid block format: {0}")]
    InvalidFormat(String),
    
    #[error("Block corrupted: {0}")]
    Corrupted(String),
    
    #[error("Block not found: {0}")]
    NotFound(String),
}

// Implement From<CompressionError> for BlockError
impl From<CompressionError> for BlockError {
    fn from(err: CompressionError) -> Self {
        BlockError::Compression(err.to_string())
    }
}

// Block status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockStatus {
    /// Active block that is currently being written to
    Active,
    /// Sealed block that is no longer accepting writes
    Sealed,
    /// Archived block that has been compressed
    Archived,
}

// Block metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMetadata {
    /// Unique block ID
    pub id: String,
    /// Block creation timestamp
    pub created_at: DateTime<Utc>,
    /// Block sealing timestamp (if sealed)
    pub sealed_at: Option<DateTime<Utc>>,
    /// Number of entries in the block
    pub entry_count: u64,
    /// Uncompressed size in bytes
    pub uncompressed_size: u64,
    /// Compressed size in bytes (if compressed)
    pub compressed_size: Option<u64>,
    /// Compression algorithm used (if compressed)
    pub compression: CompressionAlgorithm,
    /// CRC32 checksum of the entire block
    pub checksum: u32,
    /// Is this block marked for deletion
    pub marked_for_deletion: bool,
}

// Individual log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Entry timestamp
    pub timestamp: DateTime<Utc>,
    /// Entry source (hostname, service name, etc.)
    pub source: String,
    /// Optional structured tags
    pub tags: BTreeMap<String, String>,
    /// The log message itself
    pub message: String,
}

// Block header stored at the beginning of each block file
#[derive(Debug, Serialize, Deserialize)]
struct BlockHeader {
    /// Magic bytes to identify TimberDB block files
    magic: [u8; 8],
    /// Format version
    version: u16,
    /// Block metadata
    metadata: BlockMetadata,
}

impl BlockHeader {
    const MAGIC: [u8; 8] = *b"TIMBERDB";
    const VERSION: u16 = 1;
    
    fn new(metadata: BlockMetadata) -> Self {
        BlockHeader {
            magic: Self::MAGIC,
            version: Self::VERSION,
            metadata,
        }
    }
    
    fn validate(&self) -> Result<(), BlockError> {
        if self.magic != Self::MAGIC {
            return Err(BlockError::InvalidFormat(
                "Invalid magic bytes".to_string(),
            ));
        }
        
        if self.version != Self::VERSION {
            return Err(BlockError::InvalidFormat(
                format!("Unsupported version: {}", self.version),
            ));
        }
        
        Ok(())
    }
}

// A block of log entries
#[derive(Debug)]
pub struct Block {
    /// Block metadata
    metadata: BlockMetadata,
    /// Path to the block file
    path: PathBuf,
    /// Current status of the block
    status: BlockStatus,
    /// File handle for active blocks
    file: Option<File>,
    /// In-memory index for fast lookups
    index: Arc<RwLock<Index>>,
    /// Writer position for appending entries
    write_pos: u64,
}

impl Block {
    /// Create a new active block
    pub fn create(
        block_id: String,
        dir_path: &Path,
        compression: CompressionAlgorithm,
    ) -> Result<Self, BlockError> {
        let now = Utc::now();
        let metadata = BlockMetadata {
            id: block_id.clone(),
            created_at: now,
            sealed_at: None,
            entry_count: 0,
            uncompressed_size: 0,
            compressed_size: None,
            compression,
            checksum: 0,
            marked_for_deletion: false,
        };
        
        // Ensure directory exists
        fs::create_dir_all(dir_path)?;
        
        // Create block filename and path
        let filename = format!("{}.block", block_id);
        let path = dir_path.join(filename);
        
        // Create and initialize block file
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        
        // Write block header
        let header = BlockHeader::new(metadata.clone());
        let header_bytes = bincode::serialize(&header)?;
        file.write_all(&header_bytes)?;
        
        // Create index file
        let index_path = dir_path.join(format!("{}.index", block_id));
        let index = Index::create(&index_path)?;
        
        Ok(Block {
            metadata,
            path,
            status: BlockStatus::Active,
            file: Some(file),
            index: Arc::new(RwLock::new(index)),
            write_pos: header_bytes.len() as u64,
        })
    }
    
    /// Open an existing block
    pub fn open(path: PathBuf) -> Result<Self, BlockError> {
        if !path.exists() {
            return Err(BlockError::NotFound(
                format!("Block file not found: {}", path.display()),
            ));
        }
        
        let file = File::open(&path)?;
        
        // Read and validate header
        let mut reader = BufReader::new(&file);
        let mut header_bytes = Vec::new();
        reader.read_to_end(&mut header_bytes)?;
        
        let header: BlockHeader = bincode::deserialize(&header_bytes)?;
        header.validate()?;
        
        // Determine block status
        let status = if header.metadata.sealed_at.is_some() {
            if header.metadata.compressed_size.is_some() {
                BlockStatus::Archived
            } else {
                BlockStatus::Sealed
            }
        } else {
            BlockStatus::Active
        };
        
        // Open index
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let index_path = path.with_file_name(format!("{}.index", stem));
        let index = Index::open(&index_path)?;
        
        // For active blocks, get write position
        let write_pos = if status == BlockStatus::Active {
            file.metadata()?.len()
        } else {
            0
        };
        
        // Only keep file handle for active blocks
        let file_handle = if status == BlockStatus::Active {
            Some(file)
        } else {
            None
        };
        
        Ok(Block {
            metadata: header.metadata,
            path,
            status,
            file: file_handle,
            index: Arc::new(RwLock::new(index)),
            write_pos,
        })
    }
    
    /// Append a log entry to the block (only for active blocks)
    pub fn append(&mut self, entry: LogEntry) -> Result<u64, BlockError> {
        if self.status != BlockStatus::Active {
            return Err(BlockError::InvalidFormat(
                "Cannot append to sealed or archived block".to_string(),
            ));
        }
        
        let file = self.file.as_mut().unwrap();
        
        // Serialize entry
        let entry_bytes = bincode::serialize(&entry)?;
        let entry_len = entry_bytes.len() as u32;
        
        // Calculate entry CRC
        let mut hasher = Hasher::new();
        hasher.update(&entry_bytes);
        let entry_crc = hasher.finalize();
        
        // Format: [entry_length:u32][entry_crc:u32][entry_data]
        file.seek(SeekFrom::Start(self.write_pos))?;
        file.write_all(&entry_len.to_le_bytes())?;
        file.write_all(&entry_crc.to_le_bytes())?;
        let entry_pos = file.stream_position()?;
        file.write_all(&entry_bytes)?;
        
        // Update metadata
        self.metadata.entry_count += 1;
        self.metadata.uncompressed_size = file.stream_position()?;
        
        // Update write position
        self.write_pos = self.metadata.uncompressed_size;
        
        // Add to index
        let entry_id = self.metadata.entry_count - 1;
        let index_entry = IndexEntry {
            id: entry_id,
            timestamp: entry.timestamp,
            position: entry_pos,
            length: entry_len,
        };
        
        let mut index = self.index.write().map_err(|e| 
            BlockError::InvalidFormat(format!("Failed to acquire index write lock: {}", e))
        )?;
        index.add_entry(index_entry)?;
        
        Ok(entry_id)
    }
    
    /// Seal the block, preventing further writes
    pub fn seal(&mut self) -> Result<(), BlockError> {
        if self.status != BlockStatus::Active {
            return Err(BlockError::InvalidFormat(
                "Block is already sealed".to_string(),
            ));
        }
        
        // Update metadata
        self.metadata.sealed_at = Some(Utc::now());
        
        // Update header
        let file = self.file.as_mut().unwrap();
        file.seek(SeekFrom::Start(0))?;
        
        let header = BlockHeader::new(self.metadata.clone());
        let header_bytes = bincode::serialize(&header)?;
        file.write_all(&header_bytes)?;
        file.flush()?;
        
        // Close file handle
        self.file = None;
        self.status = BlockStatus::Sealed;
        
        Ok(())
    }
    
    /// Archive (compress) the block to save space
    pub fn archive(&mut self, compression_level: i32) -> Result<(), BlockError> {
        if self.status != BlockStatus::Sealed {
            return Err(BlockError::InvalidFormat(
                "Only sealed blocks can be archived".to_string(),
            ));
        }
        
        // Read the entire block file
        let file_content = fs::read(&self.path)?;
        
        // Compress the data
        let compressed_data = compress_data(
            &file_content,
            self.metadata.compression,
            compression_level,
        )?;
        
        // Create archive file
        let archive_path = self.path.with_extension("block.archive");
        let mut archive_file = File::create(&archive_path)?;
        
        // Write compressed data
        archive_file.write_all(&compressed_data)?;
        archive_file.flush()?;
        
        // Update metadata
        self.metadata.compressed_size = Some(compressed_data.len() as u64);
        
        // Write updated metadata to a separate file
        let meta_path = self.path.with_extension("meta");
        let meta_file = File::create(&meta_path)?;
        let mut writer = BufWriter::new(meta_file);
        
        let header = BlockHeader::new(self.metadata.clone());
        bincode::serialize_into(&mut writer, &header)?;
        writer.flush()?;
        
        // Remove original file to save space
        fs::remove_file(&self.path)?;
        
        // Update internal state
        self.path = archive_path;
        self.status = BlockStatus::Archived;
        
        Ok(())
    }
    
    /// Read a specific log entry by ID
    pub fn read_entry(&self, entry_id: u64) -> Result<LogEntry, BlockError> {
        if entry_id >= self.metadata.entry_count {
            return Err(BlockError::NotFound(
                format!("Entry ID {} not found in block", entry_id),
            ));
        }
        
        // Get entry position from index
        let index_entry = self.index.read().map_err(|e|
            BlockError::InvalidFormat(format!("Failed to acquire index read lock: {}", e))
        )?.get_entry_by_id(entry_id)?;
        
        // Read the entry based on block status
        match self.status {
            BlockStatus::Active => {
                // For active blocks, we have an open file handle
                let mut file = self.file.as_ref().unwrap().try_clone()?;
                file.seek(SeekFrom::Start(index_entry.position))?;
                
                let mut entry_data = vec![0u8; index_entry.length as usize];
                file.read_exact(&mut entry_data)?;
                
                let entry: LogEntry = bincode::deserialize(&entry_data)?;
                Ok(entry)
            }
            BlockStatus::Sealed => {
                // For sealed blocks, open the file
                let mut file = File::open(&self.path)?;
                file.seek(SeekFrom::Start(index_entry.position))?;
                
                let mut entry_data = vec![0u8; index_entry.length as usize];
                file.read_exact(&mut entry_data)?;
                
                let entry: LogEntry = bincode::deserialize(&entry_data)?;
                Ok(entry)
            }
            BlockStatus::Archived => {
                // For archived blocks, decompress first
                let compressed_data = fs::read(&self.path)?;
                
                // Decompress the data
                let data = decompress_data(
                    &compressed_data,
                    self.metadata.compression,
                )?;
                
                // Extract the entry from decompressed data
                let entry_data = &data[index_entry.position as usize..(index_entry.position as usize + index_entry.length as usize)];
                
                let entry: LogEntry = bincode::deserialize(entry_data)?;
                Ok(entry)
            }
        }
    }
    
    /// Get block metadata
    pub fn metadata(&self) -> &BlockMetadata {
        &self.metadata
    }
    
    /// Get mutable reference to block metadata
    pub fn metadata_mut(&mut self) -> &mut BlockMetadata {
        &mut self.metadata
    }
    
    /// Get block status
    pub fn status(&self) -> BlockStatus {
        self.status
    }
    
    /// Get block path
    pub fn path(&self) -> &Path {
        &self.path
    }
    
    /// Mark block for deletion
    pub fn mark_for_deletion(&mut self) -> Result<(), BlockError> {
        self.metadata.marked_for_deletion = true;
        
        // Update header or metadata file
        match self.status {
            BlockStatus::Active => {
                let file = self.file.as_mut().unwrap();
                file.seek(SeekFrom::Start(0))?;
                
                let header = BlockHeader::new(self.metadata.clone());
                let header_bytes = bincode::serialize(&header)?;
                file.write_all(&header_bytes)?;
                file.flush()?;
            }
            BlockStatus::Sealed | BlockStatus::Archived => {
                let meta_path = self.path.with_extension("meta");
                let meta_file = File::create(&meta_path)?;
                let mut writer = BufWriter::new(meta_file);
                
                let header = BlockHeader::new(self.metadata.clone());
                bincode::serialize_into(&mut writer, &header)?;
                writer.flush()?;
            }
        }
        
        Ok(())
    }
    
    /// Delete the block permanently
    pub fn delete(self) -> Result<(), BlockError> {
        // Delete block file
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        
        // Delete index file
        let index_path = self.path.with_extension("index");
        if index_path.exists() {
            fs::remove_file(&index_path)?;
        }
        
        // Delete metadata file if exists
        let meta_path = self.path.with_extension("meta");
        if meta_path.exists() {
            fs::remove_file(&meta_path)?;
        }
        
        Ok(())
    }
    
    /// Check if block should be rotated based on policy
    pub fn should_rotate(&self, policy: &BlockRotationPolicy) -> bool {
        match policy {
            BlockRotationPolicy::Time(duration) => {
                let block_age = Utc::now()
                    .signed_duration_since(self.metadata.created_at)
                    .to_std()
                    .unwrap_or_else(|_| Duration::from_secs(0));
                
                block_age >= *duration
            }
            BlockRotationPolicy::Size(max_size) => {
                self.metadata.uncompressed_size >= *max_size
            }
            BlockRotationPolicy::Count(max_entries) => {
                self.metadata.entry_count >= *max_entries
            }
        }
    }
}