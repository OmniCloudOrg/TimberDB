// TimberDB: A high-performance distributed log database
// storage/index.rs - Indexing for efficient log retrieval

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Index errors
#[derive(Error, Debug)]
pub enum IndexError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Entry not found: {0}")]
    NotFound(String),
    
    #[error("Invalid index format: {0}")]
    InvalidFormat(String),
}

// Index header stored at the beginning of each index file
#[derive(Debug, Serialize, Deserialize)]
struct IndexHeader {
    /// Magic bytes to identify TimberDB index files
    magic: [u8; 8],
    /// Format version
    version: u16,
    /// Number of entries in the index
    entry_count: u64,
    /// Timestamp of oldest entry
    oldest_timestamp: DateTime<Utc>,
    /// Timestamp of newest entry
    newest_timestamp: DateTime<Utc>,
}

impl IndexHeader {
    const MAGIC: [u8; 8] = *b"TDBRIDGE";
    const VERSION: u16 = 1;
    
    fn new(
        entry_count: u64,
        oldest_timestamp: DateTime<Utc>,
        newest_timestamp: DateTime<Utc>,
    ) -> Self {
        IndexHeader {
            magic: Self::MAGIC,
            version: Self::VERSION,
            entry_count,
            oldest_timestamp,
            newest_timestamp,
        }
    }
    
    fn validate(&self) -> Result<(), IndexError> {
        if self.magic != Self::MAGIC {
            return Err(IndexError::InvalidFormat(
                "Invalid magic bytes".to_string(),
            ));
        }
        
        if self.version != Self::VERSION {
            return Err(IndexError::InvalidFormat(
                format!("Unsupported version: {}", self.version),
            ));
        }
        
        Ok(())
    }
}

// Entry in the index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Entry ID
    pub id: u64,
    /// Entry timestamp
    pub timestamp: DateTime<Utc>,
    /// Position in the block file
    pub position: u64,
    /// Length of the entry data
    pub length: u32,
}

// In-memory index
#[derive(Debug)]
pub struct Index {
    /// Path to the index file
    path: PathBuf,
    /// File handle for writing
    file: Option<File>,
    /// Entry count
    entry_count: AtomicU64,
    /// Entries by ID
    entries_by_id: BTreeMap<u64, IndexEntry>,
    /// Entries by timestamp
    entries_by_timestamp: BTreeMap<DateTime<Utc>, Vec<IndexEntry>>,
    /// Offset where entries start in the file
    entries_offset: u64,
    /// Oldest timestamp in the index
    oldest_timestamp: DateTime<Utc>,
    /// Newest timestamp in the index
    newest_timestamp: DateTime<Utc>,
}

impl Index {
    /// Create a new index
    pub fn create(path: &Path) -> Result<Self, IndexError> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Create and open index file
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        
        let now = Utc::now();
        
        // Write empty header
        let header = IndexHeader::new(0, now, now);
        let header_bytes = bincode::serialize(&header)?;
        
        {
            let mut writer = BufWriter::new(&file);
            writer.write_all(&header_bytes)?;
            writer.flush()?;
        }
        
        Ok(Index {
            path: path.to_path_buf(),
            file: Some(file),
            entry_count: AtomicU64::new(0),
            entries_by_id: BTreeMap::new(),
            entries_by_timestamp: BTreeMap::new(),
            entries_offset: header_bytes.len() as u64,
            oldest_timestamp: now,
            newest_timestamp: now,
        })
    }
    
    /// Open an existing index
    pub fn open(path: &Path) -> Result<Self, IndexError> {
        // Open index file
        let file = match OpenOptions::new().read(true).write(true).open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Create new index if file doesn't exist
                return Self::create(path);
            }
            Err(e) => return Err(IndexError::Io(e)),
        };
        
        let mut reader = BufReader::new(&file);
        
        // Read and validate header
        let header_bytes = bincode::deserialize_from(&mut reader)?;
        let header: IndexHeader = header_bytes;
        header.validate()?;
        
        // Calculate entries offset
        let entries_offset = reader.stream_position()?;
        
        // Read all entries
        let mut entries_by_id = BTreeMap::new();
        let mut entries_by_timestamp = BTreeMap::new();
        
        for _ in 0..header.entry_count {
            let entry: IndexEntry = bincode::deserialize_from(&mut reader)?;
            
            // Add to maps
            entries_by_id.insert(entry.id, entry.clone());
            
            entries_by_timestamp
                .entry(entry.timestamp)
                .or_insert_with(Vec::new)
                .push(entry);
        }
        
        Ok(Index {
            path: path.to_path_buf(),
            file: Some(file),
            entry_count: AtomicU64::new(header.entry_count),
            entries_by_id,
            entries_by_timestamp,
            entries_offset,
            oldest_timestamp: header.oldest_timestamp,
            newest_timestamp: header.newest_timestamp,
        })
    }
    
    /// Add a new entry to the index
    pub fn add_entry(&mut self, entry: IndexEntry) -> Result<(), IndexError> {
        // Update oldest/newest timestamps
        if self.entry_count.load(Ordering::Relaxed) == 0 {
            self.oldest_timestamp = entry.timestamp;
            self.newest_timestamp = entry.timestamp;
        } else {
            if entry.timestamp < self.oldest_timestamp {
                self.oldest_timestamp = entry.timestamp;
            }
            if entry.timestamp > self.newest_timestamp {
                self.newest_timestamp = entry.timestamp;
            }
        }
        
        // Write entry to file
        if let Some(file) = &mut self.file {
            file.seek(SeekFrom::End(0))?;
            let mut writer = BufWriter::new(file);
            bincode::serialize_into(&mut writer, &entry)?;
            writer.flush()?;
        }
        
        // Update in-memory maps
        self.entries_by_id.insert(entry.id, entry.clone());
        
        self.entries_by_timestamp
            .entry(entry.timestamp)
            .or_insert_with(Vec::new)
            .push(entry);
        
        // Increment entry count
        self.entry_count.fetch_add(1, Ordering::SeqCst);
        
        // Update header
        self.update_header()?;
        
        Ok(())
    }
    
    /// Update the header with current metadata
    fn update_header(&mut self) -> Result<(), IndexError> {
        if let Some(file) = &mut self.file {
            file.seek(SeekFrom::Start(0))?;
            
            let header = IndexHeader::new(
                self.entry_count.load(Ordering::Relaxed),
                self.oldest_timestamp,
                self.newest_timestamp,
            );
            
            bincode::serialize_into(file, &header)?;
        }
        
        Ok(())
    }
    
    /// Get an entry by ID
    pub fn get_entry_by_id(&self, id: u64) -> Result<IndexEntry, IndexError> {
        self.entries_by_id
            .get(&id)
            .cloned()
            .ok_or_else(|| IndexError::NotFound(format!("Entry with ID {} not found", id)))
    }
    
    /// Get entries by timestamp
    pub fn get_entries_by_timestamp(
        &self,
        timestamp: DateTime<Utc>,
    ) -> Vec<IndexEntry> {
        self.entries_by_timestamp
            .get(&timestamp)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Get entries within a time range
    pub fn get_entries_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<IndexEntry> {
        let mut result = Vec::new();
        
        for (ts, entries) in self.entries_by_timestamp.range(start..=end) {
            result.extend(entries.clone());
        }
        
        result
    }
    
    /// Close the index, flushing any pending changes
    pub fn close(mut self) -> Result<(), IndexError> {
        if let Some(mut file) = self.file.take() {
            // Update header one last time
            self.update_header()?;
            
            // Flush and close
            file.flush()?;
        }
        
        Ok(())
    }
    
    /// Get the number of entries in the index
    pub fn entry_count(&self) -> u64 {
        self.entry_count.load(Ordering::Relaxed)
    }
    
    /// Get the oldest timestamp in the index
    pub fn oldest_timestamp(&self) -> DateTime<Utc> {
        self.oldest_timestamp
    }
    
    /// Get the newest timestamp in the index
    pub fn newest_timestamp(&self) -> DateTime<Utc> {
        self.newest_timestamp
    }
}