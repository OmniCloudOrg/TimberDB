use clap::{Parser, Subcommand};
use env_logger::Env;
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use chrono::{DateTime, Utc};
use tokio::time::Duration;

#[cfg(feature = "api")]
use timberdb::api::{ApiServer, server::ApiConfig};
use timberdb::{Config, TimberDB, LogEntry};

/// TimberDB - A high-performance embeddable log database
#[derive(Parser)]
#[command(name = "timberdb")]
#[command(about = "A high-performance embeddable log database with block-based storage")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Data directory (only used for standalone mode)
    #[arg(short, long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    
    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,
    
    /// Configuration file (only used for standalone mode)
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    
    /// API server URL (if not provided, uses standalone mode)
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    api_url: String,
    
    /// Force standalone mode (bypass API)
    #[arg(long)]
    standalone: bool,
    
    /// API timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Start the HTTP API server
    #[cfg(feature = "api")]
    Serve {
        /// Bind address for the API server
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
        
        /// Enable request logging
        #[arg(long)]
        no_logging: bool,
    },
    
    /// Create a new partition
    Create {
        /// Partition name
        name: String,
    },
    
    /// List all partitions
    List,
    
    /// Show partition information
    Info {
        /// Partition ID or name
        partition: String,
    },
    
    /// Append a log entry
    Append {
        /// Partition ID or name
        partition: String,
        
        /// Log source
        #[arg(short, long)]
        source: String,
        
        /// Log message
        #[arg(short, long)]
        message: String,
        
        /// Tags in key=value format
        #[arg(short, long)]
        tags: Vec<String>,
    },
    
    /// Query log entries
    Query {
        /// Partition ID or name (optional, searches all if not specified)
        #[arg(short, long)]
        partition: Option<String>,
        
        /// Source filter
        #[arg(short, long)]
        source: Option<String>,
        
        /// Message contains filter
        #[arg(short, long)]
        message: Option<String>,
        
        /// Tag filters in key=value format
        #[arg(short, long)]
        tags: Vec<String>,
        
        /// Maximum number of results
        #[arg(short, long, default_value = "100")]
        limit: usize,
        
        /// Show last N minutes of logs
        #[arg(long)]
        last_minutes: Option<u64>,
        
        /// Show last N hours of logs
        #[arg(long)]
        last_hours: Option<u64>,
        
        /// Show last N days of logs
        #[arg(long)]
        last_days: Option<u64>,
    },
    
    /// Database maintenance operations
    Maintenance {
        /// Force flush all partitions
        #[arg(long)]
        flush: bool,
        
        /// Compact storage (remove old entries)
        #[arg(long)]
        compact: bool,
        
        /// Show database statistics
        #[arg(long)]
        stats: bool,
    },
    
    /// Check API server status
    Status,
}

// API request/response types
#[derive(Serialize)]
struct CreatePartitionRequest {
    name: String,
}

#[derive(Deserialize)]
struct CreatePartitionResponse {
    partition_id: String,
}

#[derive(Deserialize)]
struct PartitionInfo {
    id: String,
    name: String,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
    total_entries: u64,
    total_size: u64,
    block_count: u64,
    active_block_id: Option<String>,
}

#[derive(Serialize)]
struct AppendRequest {
    source: String,
    message: String,
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct AppendResponse {
    entry_id: String,
}

#[derive(Serialize)]
struct QueryRequest {
    partition_id: Option<String>,
    source: Option<String>,
    message_contains: Option<String>,
    tags: HashMap<String, String>,
    limit: usize,
    time_range_seconds: Option<u64>,
}

#[derive(Deserialize)]
struct QueryResponse {
    entries: Vec<LogEntryResponse>,
    total_count: u64,
    returned_count: u64,
    execution_time_ms: u64,
    partitions_searched: usize,
}

#[derive(Deserialize)]
struct LogEntryResponse {
    id: String,
    timestamp: DateTime<Utc>,
    source: String,
    message: String,
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct StatsResponse {
    total_partitions: usize,
    total_entries: u64,
    total_size: u64,
}

#[derive(Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    details: Option<String>,
}

struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    fn new(base_url: String, timeout_secs: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            client,
            base_url,
        }
    }
    
    async fn is_available(&self) -> bool {
        match self.client
            .get(&format!("{}/api/v1/health", self.base_url))
            .send()
            .await 
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
    
    async fn get_health(&self) -> Result<HealthResponse, Box<dyn std::error::Error>> {
        let response = self.client
            .get(&format!("{}/api/v1/health", self.base_url))
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        let health: HealthResponse = response.json().await?;
        Ok(health)
    }
    
    async fn create_partition(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let request = CreatePartitionRequest {
            name: name.to_string(),
        };
        
        let response = self.client
            .post(&format!("{}/api/v1/partitions", self.base_url))
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        let result: CreatePartitionResponse = response.json().await?;
        Ok(result.partition_id)
    }
    
    async fn list_partitions(&self) -> Result<Vec<PartitionInfo>, Box<dyn std::error::Error>> {
        let response = self.client
            .get(&format!("{}/api/v1/partitions", self.base_url))
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        let partitions: Vec<PartitionInfo> = response.json().await?;
        Ok(partitions)
    }
    
    async fn get_partition(&self, partition: &str) -> Result<PartitionInfo, Box<dyn std::error::Error>> {
        let response = self.client
            .get(&format!("{}/api/v1/partitions/{}", self.base_url, partition))
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        let partition_info: PartitionInfo = response.json().await?;
        Ok(partition_info)
    }
    
    async fn append(&self, partition_id: &str, source: String, message: String, tags: HashMap<String, String>) -> Result<String, Box<dyn std::error::Error>> {
        let request = AppendRequest {
            source,
            message,
            tags,
        };
        
        let response = self.client
            .post(&format!("{}/api/v1/partitions/{}/append", self.base_url, partition_id))
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        let result: AppendResponse = response.json().await?;
        Ok(result.entry_id)
    }
    
    async fn query(&self, request: QueryRequest) -> Result<QueryResponse, Box<dyn std::error::Error>> {
        let response = self.client
            .post(&format!("{}/api/v1/query", self.base_url))
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        let result: QueryResponse = response.json().await?;
        Ok(result)
    }
    
    async fn flush(&self) -> Result<(), Box<dyn std::error::Error>> {
        let response = self.client
            .post(&format!("{}/api/v1/maintenance/flush", self.base_url))
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        Ok(())
    }
    
    async fn compact(&self) -> Result<(), Box<dyn std::error::Error>> {
        let response = self.client
            .post(&format!("{}/api/v1/maintenance/compact", self.base_url))
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<StatsResponse, Box<dyn std::error::Error>> {
        let response = self.client
            .get(&format!("{}/api/v1/stats", self.base_url))
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                error: format!("HTTP {}", status),
                details: None,
            });
            return Err(format!("API error: {}", error.error).into());
        }
        
        let stats: StatsResponse = response.json().await?;
        Ok(stats)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Initialize logging
    env_logger::Builder::from_env(Env::default().default_filter_or(&cli.log_level))
        .init();
    
    // Handle serve command (always uses standalone mode)
    #[cfg(feature = "api")]
    if let Commands::Serve { bind, no_logging } = &cli.command.clone() {
        return serve_api(cli, bind.clone(), *no_logging).await;
    }
    
    // Determine if we should use API mode or standalone mode
    let use_api = !cli.standalone && {
        let client = ApiClient::new(cli.api_url.clone(), cli.timeout);
        client.is_available().await
    };
    
    if use_api {
        log::info!("Using API mode (server: {})", cli.api_url);
        execute_api_command(cli).await
    } else {
        if !cli.standalone {
            log::warn!("API server not available at {}, falling back to standalone mode", cli.api_url);
        } else {
            log::info!("Using standalone mode (forced)");
        }
        execute_standalone_command(cli).await
    }
}

#[cfg(feature = "api")]
async fn serve_api(cli: Cli, bind: String, no_logging: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let mut config = if let Some(config_path) = cli.config {
        let config_str = tokio::fs::read_to_string(config_path).await
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        toml::from_str(&config_str)
            .map_err(|e| format!("Failed to parse config file: {}", e))?
    } else {
        Config::default()
    };
    
    // Override data directory if specified
    if let Some(data_dir) = cli.data_dir {
        config.data_dir = data_dir;
    }
    
    // Open database
    let db = TimberDB::open(config).await
        .map_err(|e| format!("Failed to open database: {}", e))?;
    
    let mut api_config = ApiConfig::default();
    api_config.bind_address = bind.parse()
        .map_err(|e| format!("Invalid bind address '{}': {}", bind, e))?;
    api_config.enable_logging = !no_logging;
    
    println!("Starting TimberDB API server on {}", api_config.bind_address);
    
    let server = ApiServer::new(db.clone(), api_config);
    server.start().await?;
    
    Ok(())
}

async fn execute_api_command(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let client = ApiClient::new(cli.api_url, cli.timeout);
    
    match cli.command {
        Commands::Status => {
            match client.get_health().await {
                Ok(health) => {
                    println!("✓ API server is running at {}", client.base_url);
                    println!("  Status: {}", health.status);
                    println!("  Version: {}", health.version);
                    println!("  Uptime: {}s", health.uptime_seconds);
                }
                Err(e) => {
                    eprintln!("✗ API server is not available at {}", client.base_url);
                    eprintln!("  Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::Create { name } => {
            let partition_id = client.create_partition(&name).await?;
            println!("Created partition '{}' with ID: {}", name, partition_id);
        }
        
        Commands::List => {
            let partitions = client.list_partitions().await?;
            
            if partitions.is_empty() {
                println!("No partitions found.");
            } else {
                println!("Partitions:");
                println!("{:<36} {:<20} {:<15} {:<15}", "ID", "Name", "Entries", "Size");
                println!("{}", "-".repeat(86));
                
                for partition in partitions {
                    println!(
                        "{:<36} {:<20} {:<15} {:<15}",
                        partition.id,
                        partition.name,
                        partition.total_entries,
                        format_bytes(partition.total_size)
                    );
                }
            }
        }
        
        Commands::Info { partition } => {
            let partition_info = client.get_partition(&partition).await?;
            
            println!("Partition Information:");
            println!("  ID: {}", partition_info.id);
            println!("  Name: {}", partition_info.name);
            println!("  Created: {}", partition_info.created_at);
            println!("  Modified: {}", partition_info.modified_at);
            println!("  Total Entries: {}", partition_info.total_entries);
            println!("  Total Size: {}", format_bytes(partition_info.total_size));
            println!("  Block Count: {}", partition_info.block_count);
            if let Some(active_block) = &partition_info.active_block_id {
                println!("  Active Block: {}", active_block);
            }
        }
        
        Commands::Append {
            partition,
            source,
            message,
            tags,
        } => {
            // Parse tags
            let mut tag_map = HashMap::new();
            for tag in tags {
                if let Some((key, value)) = tag.split_once('=') {
                    tag_map.insert(key.to_string(), value.to_string());
                } else {
                    return Err(format!("Invalid tag format: '{}' (expected key=value)", tag).into());
                }
            }
            
            let entry_id = client.append(&partition, source, message, tag_map).await?;
            println!("Appended log entry with ID: {}", entry_id);
        }
        
        Commands::Query {
            partition,
            source,
            message,
            tags,
            limit,
            last_minutes,
            last_hours,
            last_days,
        } => {
            // Calculate time range
            let time_range_seconds = if let Some(minutes) = last_minutes {
                Some(minutes * 60)
            } else if let Some(hours) = last_hours {
                Some(hours * 3600)
            } else if let Some(days) = last_days {
                Some(days * 86400)
            } else {
                Some(3600) // Default to last hour
            };
            
            // Parse tag filters
            let mut tag_map = HashMap::new();
            for tag in tags {
                if let Some((key, value)) = tag.split_once('=') {
                    tag_map.insert(key.to_string(), value.to_string());
                } else {
                    return Err(format!("Invalid tag format: '{}' (expected key=value)", tag).into());
                }
            }
            
            let request = QueryRequest {
                partition_id: partition,
                source,
                message_contains: message,
                tags: tag_map,
                limit,
                time_range_seconds,
            };
            
            let result = client.query(request).await?;
            
            if result.entries.is_empty() {
                println!("No matching log entries found.");
            } else {
                println!(
                    "Found {} entries (showing {}):",
                    result.total_count, result.returned_count
                );
                println!();
                
                for entry in &result.entries {
                    println!("[{}] {} | {}", 
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                        entry.source,
                        entry.message
                    );
                    
                    if !entry.tags.is_empty() {
                        print!("  Tags: ");
                        for (i, (key, value)) in entry.tags.iter().enumerate() {
                            if i > 0 { print!(", "); }
                            print!("{}={}", key, value);
                        }
                        println!();
                    }
                    println!();
                }
                
                println!(
                    "Query executed in {}ms, searched {} partitions",
                    result.execution_time_ms,
                    result.partitions_searched
                );
            }
        }
        
        Commands::Maintenance { flush, compact, stats } => {
            if !flush && !compact && !stats {
                eprintln!("Error: At least one maintenance operation must be specified");
                std::process::exit(1);
            }
            
            if flush {
                println!("Flushing all partitions...");
                client.flush().await?;
                println!("Flush completed.");
            }
            
            if compact {
                println!("Compacting storage...");
                client.compact().await?;
                println!("Compaction completed.");
            }
            
            if stats {
                let stats = client.get_stats().await?;
                
                println!("Database Statistics:");
                println!("  Total Partitions: {}", stats.total_partitions);
                println!("  Total Entries: {}", stats.total_entries);
                println!("  Total Size: {}", format_bytes(stats.total_size));
                println!("  Average Entries per Partition: {:.1}", 
                    if stats.total_partitions == 0 { 0.0 } else { stats.total_entries as f64 / stats.total_partitions as f64 });
            }
        }
        
        #[cfg(feature = "api")]
        Commands::Serve { .. } => {
            unreachable!("Serve command should be handled earlier");
        }
    }
    
    Ok(())
}

async fn execute_standalone_command(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let mut config = if let Some(config_path) = &cli.config {
        let config_str = tokio::fs::read_to_string(config_path).await
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        toml::from_str(&config_str)
            .map_err(|e| format!("Failed to parse config file: {}", e))?
    } else {
        Config::default()
    };
    
    // Override data directory if specified
    if let Some(data_dir) = cli.data_dir {
        config.data_dir = data_dir;
    }
    
    // Open database
    let db = TimberDB::open(config).await
        .map_err(|e| format!("Failed to open database: {}", e))?;
    
    // Execute command
    match cli.command {
        Commands::Status => {
            println!("Running in standalone mode (no API server)");
            println!("Database directory: {:?}", db.get_config().data_dir);
        }
        
        Commands::Create { name } => {
            let partition_id = db.create_partition(&name).await?;
            println!("Created partition '{}' with ID: {}", name, partition_id);
        }
        
        Commands::List => {
            let partitions = db.list_partitions().await?;
            
            if partitions.is_empty() {
                println!("No partitions found.");
            } else {
                println!("Partitions:");
                println!("{:<36} {:<20} {:<15} {:<15}", "ID", "Name", "Entries", "Size");
                println!("{}", "-".repeat(86));
                
                for partition in partitions {
                    println!(
                        "{:<36} {:<20} {:<15} {:<15}",
                        partition.id,
                        partition.name,
                        partition.total_entries,
                        format_bytes(partition.total_size)
                    );
                }
            }
        }
        
        Commands::Info { partition } => {
            let partitions = db.list_partitions().await?;
            let partition_info = partitions
                .iter()
                .find(|p| p.id == partition || p.name == partition)
                .ok_or_else(|| format!("Partition '{}' not found", partition))?;
            
            println!("Partition Information:");
            println!("  ID: {}", partition_info.id);
            println!("  Name: {}", partition_info.name);
            println!("  Created: {}", partition_info.created_at);
            println!("  Modified: {}", partition_info.modified_at);
            println!("  Total Entries: {}", partition_info.total_entries);
            println!("  Total Size: {}", format_bytes(partition_info.total_size));
            println!("  Block Count: {}", partition_info.block_count);
            if let Some(active_block) = &partition_info.active_block_id {
                println!("  Active Block: {}", active_block);
            }
        }
        
        Commands::Append {
            partition,
            source,
            message,
            tags,
        } => {
            // Find partition
            let partitions = db.list_partitions().await?;
            let partition_info = partitions
                .iter()
                .find(|p| p.id == partition || p.name == partition)
                .ok_or_else(|| format!("Partition '{}' not found", partition))?;
            
            // Parse tags
            let mut tag_map = HashMap::new();
            for tag in tags {
                if let Some((key, value)) = tag.split_once('=') {
                    tag_map.insert(key.to_string(), value.to_string());
                } else {
                    return Err(format!("Invalid tag format: '{}' (expected key=value)", tag).into());
                }
            }
            
            // Create log entry
            let entry = LogEntry::new(source, message, tag_map);
            
            // Append to database
            let entry_id = db.append(&partition_info.id, entry).await?;
            println!("Appended log entry with ID: {}", entry_id);
        }
        
        Commands::Query {
            partition,
            source,
            message,
            tags,
            limit,
            last_minutes,
            last_hours,
            last_days,
        } => {
            let mut query_builder = db.query();
            
            // Set partition filter
            if let Some(partition_name) = partition {
                let partitions = db.list_partitions().await?;
                let partition_info = partitions
                    .iter()
                    .find(|p| p.id == partition_name || p.name == partition_name)
                    .ok_or_else(|| format!("Partition '{}' not found", partition_name))?;
                
                query_builder = query_builder.partition(&partition_info.id);
            }
            
            // Set time filter
            if let Some(minutes) = last_minutes {
                query_builder = query_builder.last(std::time::Duration::from_secs(minutes * 60));
            } else if let Some(hours) = last_hours {
                query_builder = query_builder.last(std::time::Duration::from_secs(hours * 3600));
            } else if let Some(days) = last_days {
                query_builder = query_builder.last(std::time::Duration::from_secs(days * 86400));
            } else {
                // Default to last hour
                query_builder = query_builder.last(std::time::Duration::from_secs(3600));
            }
            
            // Set filters
            if let Some(source_filter) = source {
                query_builder = query_builder.source(source_filter);
            }
            
            if let Some(message_filter) = message {
                query_builder = query_builder.message_contains(message_filter);
            }
            
            // Parse tag filters
            for tag in tags {
                if let Some((key, value)) = tag.split_once('=') {
                    query_builder = query_builder.tag(key, value);
                } else {
                    return Err(format!("Invalid tag format: '{}' (expected key=value)", tag).into());
                }
            }
            
            query_builder = query_builder.limit(limit);
            
            // Execute query
            let result = query_builder.execute().await?;
            
            if result.entries.is_empty() {
                println!("No matching log entries found.");
            } else {
                println!(
                    "Found {} entries (showing {}):",
                    result.total_count, result.returned_count
                );
                println!();
                
                for entry in &result.entries {
                    println!("[{}] {} | {}", 
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                        entry.source,
                        entry.message
                    );
                    
                    if !entry.tags.is_empty() {
                        print!("  Tags: ");
                        for (i, (key, value)) in entry.tags.iter().enumerate() {
                            if i > 0 { print!(", "); }
                            print!("{}={}", key, value);
                        }
                        println!();
                    }
                    println!();
                }
                
                println!(
                    "Query executed in {}ms, searched {} partitions",
                    result.execution_time.as_millis(),
                    result.partitions_searched
                );
            }
        }
        
        Commands::Maintenance { flush, compact, stats } => {
            if !flush && !compact && !stats {
                eprintln!("Error: At least one maintenance operation must be specified");
                std::process::exit(1);
            }
            
            if flush {
                println!("Flushing all partitions...");
                db.flush().await?;
                println!("Flush completed.");
            }
            
            if compact {
                println!("Compacting storage...");
                // TODO: Implement compaction
                println!("Compaction not yet implemented in standalone mode.");
            }
            
            if stats {
                let partitions = db.list_partitions().await?;
                let mut total_entries = 0u64;
                let mut total_size = 0u64;
                
                for partition in &partitions {
                    total_entries += partition.total_entries;
                    total_size += partition.total_size;
                }
                
                println!("Database Statistics:");
                println!("  Total Partitions: {}", partitions.len());
                println!("  Total Entries: {}", total_entries);
                println!("  Total Size: {}", format_bytes(total_size));
                println!("  Average Entries per Partition: {:.1}", 
                    if partitions.is_empty() { 0.0 } else { total_entries as f64 / partitions.len() as f64 });
            }
        }
        
        #[cfg(feature = "api")]
        Commands::Serve { .. } => {
            unreachable!("Serve command should be handled earlier");
        }
    }
    
    // Close database
    db.close().await?;
    
    Ok(())
}

/// Format bytes in human-readable format
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}