use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// TimberDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory path
    pub data_dir: PathBuf,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// Query configuration
    pub query: QueryConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            data_dir: PathBuf::from("./timberdb_data"),
            storage: StorageConfig::default(),
            query: QueryConfig::default(),
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Block rotation policy
    pub block_rotation: BlockRotationPolicy,
    
    /// Compression configuration
    pub compression: CompressionConfig,
    
    /// Retention policy in days (0 = keep forever)
    pub retention_days: u32,
    
    /// Maximum storage size in bytes (0 = unlimited)
    pub max_storage_bytes: u64,
    
    /// Sync writes to disk immediately
    pub sync_writes: bool,
    
    /// Flush interval for background tasks
    pub flush_interval: Duration,
    
    /// Buffer size for writes
    pub write_buffer_size: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig {
            block_rotation: BlockRotationPolicy::default(),
            compression: CompressionConfig::default(),
            retention_days: 0,
            max_storage_bytes: 0,
            sync_writes: true,
            flush_interval: Duration::from_secs(30),
            write_buffer_size: 1024 * 1024, // 1MB
        }
    }
}

/// Block rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockRotationPolicy {
    /// Rotate after specified duration
    Time(Duration),
    /// Rotate after specified size in bytes
    Size(u64),
    /// Rotate after specified number of entries
    Count(u64),
}

impl Default for BlockRotationPolicy {
    fn default() -> Self {
        BlockRotationPolicy::Size(128 * 1024 * 1024) // 128MB
    }
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Compression algorithm
    pub algorithm: CompressionAlgorithm,
    
    /// Compression level (algorithm specific)
    pub level: i32,
    
    /// Enable compression for active blocks
    pub compress_active: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        CompressionConfig {
            algorithm: CompressionAlgorithm::Lz4,
            level: 1,
            compress_active: false,
        }
    }
}

/// Compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Lz4,
    #[cfg(feature = "compression")]
    Zstd,
}

/// Query configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// Default query timeout
    pub timeout: Duration,
    
    /// Maximum results per query
    pub max_results: usize,
    
    /// Enable query result caching
    pub enable_cache: bool,
    
    /// Cache TTL
    pub cache_ttl: Duration,
}

impl Default for QueryConfig {
    fn default() -> Self {
        QueryConfig {
            timeout: Duration::from_secs(30),
            max_results: 10_000,
            enable_cache: false,
            cache_ttl: Duration::from_secs(300), // 5 minutes
        }
    }
}