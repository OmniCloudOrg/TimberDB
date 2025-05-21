// TimberDB: A high-performance distributed log database
// config.rs - Configuration management

use clap::{ArgMatches, Arg, Command};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

/// Compression algorithm options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    LZ4,
    Zstd,
    Snappy,
    Gzip,
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        CompressionAlgorithm::LZ4
    }
}

/// Block rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockRotationPolicy {
    /// Rotate blocks based on time
    Time(Duration),
    /// Rotate blocks based on size (in bytes)
    Size(u64),
    /// Rotate blocks based on number of entries
    Count(u64),
}

impl Default for BlockRotationPolicy {
    fn default() -> Self {
        BlockRotationPolicy::Time(Duration::from_secs(3600)) // 1 hour default
    }
}

/// Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Unique identifier for this node
    pub id: String,
    /// Role of this node in the cluster
    pub role: NodeRole,
}

/// Node roles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    /// Leader node, can accept writes and coordinate the cluster
    Leader,
    /// Follower node, only accepts reads
    Follower,
    /// Observer node, only accepts reads and doesn't participate in consensus
    Observer,
}

impl Default for NodeRole {
    fn default() -> Self {
        NodeRole::Follower
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Directory where TimberDB will store its data
    pub data_dir: PathBuf,
    /// Block rotation policy
    pub block_rotation: BlockRotationPolicy,
    /// Compression algorithm for old blocks
    pub compression: CompressionAlgorithm,
    /// Compression level (if applicable)
    pub compression_level: i32,
    /// Number of days to keep logs before deletion (0 = keep forever)
    pub retention_days: u32,
    /// Maximum storage size in bytes (0 = unlimited)
    pub max_storage_size: u64,
    /// Sync writes to disk (durability vs performance)
    pub sync_writes: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig {
            data_dir: PathBuf::from("./data"),
            block_rotation: BlockRotationPolicy::default(),
            compression: CompressionAlgorithm::default(),
            compression_level: 3,
            retention_days: 0,
            max_storage_size: 0,
            sync_writes: true,
        }
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Address to listen on for client and peer connections
    pub listen_addr: SocketAddr,
    /// List of peer node addresses in the cluster
    pub peers: Vec<SocketAddr>,
    /// Connection timeout
    pub timeout: Duration,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// TLS configuration (if enabled)
    pub tls: Option<TlsConfig>,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: PathBuf,
    /// Path to private key file
    pub key_path: PathBuf,
    /// Optional CA certificate path for mutual TLS
    pub ca_path: Option<PathBuf>,
}

/// Query engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// Maximum number of results to return
    pub max_results: usize,
    /// Query timeout
    pub timeout: Duration,
    /// Enable query cache
    pub enable_cache: bool,
    /// Cache size in entries
    pub cache_size: usize,
}

impl Default for QueryConfig {
    fn default() -> Self {
        QueryConfig {
            max_results: 10000,
            timeout: Duration::from_secs(30),
            enable_cache: true,
            cache_size: 1000,
        }
    }
}

/// Complete TimberDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Node configuration
    pub node: NodeConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Network configuration
    pub network: NetworkConfig,
    /// Query engine configuration
    pub query: QueryConfig,
    /// Log level
    pub log_level: String,
}

/// Default configuration values
impl Default for Config {
    fn default() -> Self {
        Config {
            node: NodeConfig {
                id: Uuid::new_v4().to_string(),
                role: NodeRole::default(),
            },
            storage: StorageConfig::default(),
            network: NetworkConfig {
                listen_addr: "127.0.0.1:7777".parse().unwrap(),
                peers: Vec::new(),
                timeout: Duration::from_secs(5),
                max_connections: 1000,
                tls: None,
            },
            query: QueryConfig::default(),
            log_level: "info".to_string(),
        }
    }
}

/// Create command line argument parser
pub fn create_cli() -> Command {
    Command::new("TimberDB")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TimberDB Team")
        .about("High-performance distributed log database with configurable compression")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to configuration file"),
        )
        .arg(
            Arg::new("data-dir")
                .long("data-dir")
                .value_name("DIRECTORY")
                .help("Data directory for TimberDB"),
        )
        .arg(
            Arg::new("listen")
                .long("listen")
                .value_name("ADDRESS")
                .help("Listen address for TimberDB (format: host:port)"),
        )
        .arg(
            Arg::new("peers")
                .long("peers")
                .value_name("ADDRESSES")
                .help("Comma-separated list of peer addresses (format: host:port)"),
        )
        .arg(
            Arg::new("node-id")
                .long("node-id")
                .value_name("ID")
                .help("Unique node ID"),
        )
}

/// Load configuration from command line arguments and/or config file
pub fn load_config(matches: &ArgMatches) -> Result<Config, Box<dyn Error>> {
    // Start with default configuration
    let mut config = Config::default();

    // If config file is specified, load it
    if let Some(config_path) = matches.get_one::<String>("config") {
        let config_file = fs::read_to_string(config_path)?;
        config = toml::from_str(&config_file)?;
    }

    // Override with command line arguments if provided
    if let Some(data_dir) = matches.get_one::<String>("data-dir") {
        config.storage.data_dir = PathBuf::from(data_dir);
    }

    if let Some(listen) = matches.get_one::<String>("listen") {
        config.network.listen_addr = listen.parse()?;
    }

    if let Some(peers) = matches.get_one::<String>("peers") {
        config.network.peers = peers
            .split(',')
            .map(|addr| addr.trim().parse())
            .collect::<Result<Vec<_>, _>>()?;
    }

    if let Some(node_id) = matches.get_one::<String>("node-id") {
        config.node.id = node_id.to_string();
    }

    // Ensure data directory exists
    fs::create_dir_all(&config.storage.data_dir)?;

    Ok(config)
}