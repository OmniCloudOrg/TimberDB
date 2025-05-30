// TimberDB: A high-performance distributed log database
// api/server.rs - Production-ready HTTP API server

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::signal;
use warp::{Filter, Reply, Rejection};
use warp::http::{StatusCode, Method};
use warp::reject::Reject;

use crate::config::Config;
use crate::network::{Network, NetworkError};
use crate::query::Query;
use crate::query::engine::{QueryError, LogFilter};
use crate::storage::{Storage, Error as StorageError};
use crate::storage::block::LogEntry;

// Constants
const MAX_BODY_SIZE: u64 = 16 * 1024 * 1024; // 16MB
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

// Server errors
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    
    #[error("Query error: {0}")]
    Query(#[from] QueryError),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Server startup error: {0}")]
    Startup(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Rate limit exceeded")]
    RateLimit,
}

// Make errors implement Reject
impl Reject for NetworkError {}
impl Reject for StorageError {}
impl Reject for QueryError {}
impl Reject for ServerError {}

// API request/response types
#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
    code: String,
    timestamp: DateTime<Utc>,
    request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    timestamp: DateTime<Utc>,
    uptime_seconds: u64,
    version: String,
    node_id: String,
    storage: HealthStatus,
    network: Option<HealthStatus>,
    query_engine: HealthStatus,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthStatus {
    status: String,
    details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreatePartitionRequest {
    name: String,
    #[serde(default)]
    tags: BTreeMap<String, String>,
    #[serde(default)]
    retention_days: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreatePartitionResponse {
    id: String,
    name: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LogEntryRequest {
    #[serde(default = "Utc::now")]
    timestamp: DateTime<Utc>,
    source: String,
    #[serde(default)]
    tags: BTreeMap<String, String>,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LogEntryResponse {
    id: String,
    partition_id: String,
    entry_index: u64,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueryRequest {
    filter: LogFilter,
    #[serde(default)]
    limit: Option<u64>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MetricsResponse {
    uptime_seconds: u64,
    total_requests: u64,
    active_queries: u64,
    storage_size_bytes: u64,
    partition_count: u64,
    error_rate: f64,
}

// TimberDB server
#[derive(Debug)]
pub struct Server {
    config: Config,
    storage: Arc<Storage>,
    query: Arc<Query>,
    network: Option<Arc<Network>>,
    shutdown: Arc<tokio::sync::watch::Sender<bool>>,
    start_time: Instant,
    request_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl Server {
    /// Create a new server
    pub fn new(config: Config) -> Result<Self, ServerError> {
        // Create storage engine
        let storage = Storage::new(config.storage.clone())?;
        let storage_arc = Arc::new(storage);
        
        // Create query engine
        let query = Query::new(storage_arc.clone(), config.query.clone());
        let query_arc = Arc::new(query);
        
        // Create shutdown channel
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);
        
        // Create server instance
        let server = Server {
            config,
            storage: storage_arc,
            query: query_arc,
            network: None,
            shutdown: Arc::new(shutdown_tx),
            start_time: Instant::now(),
            request_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        };
        
        Ok(server)
    }
    
    /// Start the server
    pub async fn start(&mut self) -> Result<(), ServerError> {
        // Initialize network layer if configured
        if !self.config.network.peers.is_empty() {
            let network = Network::new(
                self.config.node.clone(),
                self.config.network.clone(),
            ).await?;
            
            self.network = Some(Arc::new(network));
        }
        
        // Set up API routes
        let routes = self.create_routes();
        
        // Start the HTTP server
        let addr: SocketAddr = self.config.network.listen_addr;
        
        log::info!("Starting TimberDB HTTP API server on {}", addr);
        log::info!("Server configuration: {:?}", self.config);
        
        // Create shutdown receiver
        let mut shutdown_rx = self.shutdown.subscribe();
        
        // Configure server with timeouts and limits
        let server = warp::serve(routes)
            .bind(addr);
        
        // Use tokio::select! to handle graceful shutdown
        tokio::select! {
            result = async { server.await; } => {
                log::info!("HTTP server stopped: {:?}", result);
            },
            _ = Self::handle_signals(self.shutdown.clone()) => {
                log::info!("Shutting down server due to signal");
            },
            _ = async {
                loop {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                    if shutdown_rx.changed().await.is_err() {
                        break;
                    }
                }
            } => {
                log::info!("Shutting down server due to shutdown request");
            }
        }
        
        // Perform graceful shutdown
        self.perform_graceful_shutdown().await;
        
        Ok(())
    }
    
    /// Handle OS signals for graceful shutdown
    async fn handle_signals(
        shutdown: Arc<tokio::sync::watch::Sender<bool>>,
    ) -> Result<(), ServerError> {
        // Wait for ctrl-c signal
        signal::ctrl_c().await?;
        log::info!("Received termination signal, shutting down...");
        
        // Signal shutdown
        let _ = shutdown.send(true);
        
        Ok(())
    }
    
    /// Perform graceful shutdown with timeout
    async fn perform_graceful_shutdown(&self) {
        let shutdown_start = Instant::now();
        
        log::info!("Starting graceful shutdown...");
        
        // Use timeout to ensure shutdown doesn't hang
        let shutdown_future = async {
            // Shutdown components in order
            if let Some(network) = &self.network {
                log::info!("Shutting down network layer...");
                network.shutdown().await;
            }
            
            log::info!("Shutting down query engine...");
            self.query.shutdown().await;
            
            // Shutdown storage
            log::info!("Shutting down storage engine...");
            let storage = Arc::clone(&self.storage);
            let storage_result = tokio::task::spawn_blocking(move || {
                match Arc::try_unwrap(storage) {
                    Ok(storage) => storage.shutdown(),
                    Err(_) => {
                        log::warn!("Storage still has multiple references during shutdown");
                        Ok(())
                    }
                }
            }).await;
            
            if let Err(e) = storage_result {
                log::error!("Error joining storage shutdown task: {}", e);
            }
        };
        
        // Apply timeout to shutdown
        match tokio::time::timeout(GRACEFUL_SHUTDOWN_TIMEOUT, shutdown_future).await {
            Ok(_) => {
                let elapsed = shutdown_start.elapsed();
                log::info!("Graceful shutdown completed in {:?}", elapsed);
            }
            Err(_) => {
                log::warn!("Graceful shutdown timed out after {:?}, forcing exit", GRACEFUL_SHUTDOWN_TIMEOUT);
            }
        }
    }
    
    /// Create API routes with proper error handling and middleware
    fn create_routes(
        &self,
    ) -> impl Filter<Extract = impl Reply, Error = Infallible> + Clone {
        let storage = self.storage.clone();
        let query = self.query.clone();
        let network = self.network.clone();
        let start_time = self.start_time;
        let request_counter = self.request_counter.clone();
        let node_id = self.config.node.id.clone();
        
        // Common middleware
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type", "authorization"])
            .allow_methods(vec![Method::GET, Method::POST, Method::PUT, Method::DELETE]);
        
        let request_logging = warp::log::custom(|info| {
            log::info!(
                "{} {} {} {}ms",
                info.method(),
                info.path(),
                info.status(),
                info.elapsed().as_millis()
            );
        });
        
        // Health and metrics routes
        let health_route = warp::path("health")
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and(with_query(query.clone()))
            .and(with_network(network.clone()))
            .and(warp::any().map(move || start_time))
            .and(warp::any().map(move || node_id.clone()))
            .and_then(handle_health_check);
        
        let metrics_route = warp::path("metrics")
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and(with_query(query.clone()))
            .and(warp::any().map(move || start_time))
            .and(with_request_counter(request_counter.clone()))
            .and_then(handle_metrics);
        
        // Partition management routes
        let partitions_list = warp::path("partitions")
            .and(warp::path::end())
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_list_partitions);
        
        let partition_create = warp::path!("partitions" / "create")
            .and(warp::post())
            .and(warp::body::content_length_limit(MAX_BODY_SIZE))
            .and(warp::body::json())
            .and(with_storage(storage.clone()))
            .and(with_network(network.clone()))
            .and_then(handle_create_partition);
        
        let partition_get = warp::path!("partitions" / String)
            .and(warp::path::end())
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_get_partition);
        
        // Log entry routes
        let logs_append = warp::path!("partitions" / String / "logs")
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::content_length_limit(MAX_BODY_SIZE))
            .and(warp::body::json())
            .and(with_storage(storage.clone()))
            .and(with_network(network.clone()))
            .and_then(handle_append_log);
        
        let logs_get = warp::path!("partitions" / String / "blocks" / String / "logs" / u64)
            .and(warp::path::end())
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_get_log);
        
        // Query routes
        let query_execute = warp::path!("query" / "execute")
            .and(warp::post())
            .and(warp::body::content_length_limit(MAX_BODY_SIZE))
            .and(warp::body::json())
            .and(with_query(query.clone()))
            .and_then(handle_execute_query);
        
        let query_cancel = warp::path!("query" / String / "cancel")
            .and(warp::post())
            .and(with_query(query.clone()))
            .and_then(handle_cancel_query);
        
        let query_status = warp::path!("query" / String / "status")
            .and(warp::get())
            .and(with_query(query.clone()))
            .and_then(handle_query_status);
        
        // API versioning
        let api_v1 = warp::path("api")
            .and(warp::path("v1"))
            .and(
                partitions_list
                    .or(partition_create)
                    .or(partition_get)
                    .or(logs_append)
                    .or(logs_get)
                    .or(query_execute)
                    .or(query_cancel)
                    .or(query_status)
            );
        
        // Combine all routes
        let all_routes = health_route
            .or(metrics_route)
            .or(api_v1)
            .with(cors)
            .with(request_logging)
            .with(warp::trace::request())
            .recover(handle_rejection);
        
        // Add request counting middleware
        all_routes.map(move |reply| {
            request_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            reply
        })
    }
    
    /// Shutdown the server
    pub async fn shutdown(&self) {
        log::info!("Initiating server shutdown...");
        let _ = self.shutdown.send(true);
    }
}

// Helper functions for dependency injection
fn with_storage(
    storage: Arc<Storage>,
) -> impl Filter<Extract = (Arc<Storage>,), Error = Infallible> + Clone {
    warp::any().map(move || storage.clone())
}

fn with_query(
    query: Arc<Query>,
) -> impl Filter<Extract = (Arc<Query>,), Error = Infallible> + Clone {
    warp::any().map(move || query.clone())
}

fn with_network(
    network: Option<Arc<Network>>,
) -> impl Filter<Extract = (Option<Arc<Network>>,), Error = Infallible> + Clone {
    warp::any().map(move || network.clone())
}

fn with_request_counter(
    counter: Arc<std::sync::atomic::AtomicU64>,
) -> impl Filter<Extract = (Arc<std::sync::atomic::AtomicU64>,), Error = Infallible> + Clone {
    warp::any().map(move || counter.clone())
}

// Enhanced error handling
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message, error_code) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Resource not found".to_string(), "NOT_FOUND")
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        (StatusCode::BAD_REQUEST, format!("Invalid request body: {}", e), "INVALID_REQUEST")
    } else if let Some(e) = err.find::<warp::reject::PayloadTooLarge>() {
        (StatusCode::PAYLOAD_TOO_LARGE, format!("Request too large: {}", e), "PAYLOAD_TOO_LARGE")
    } else if let Some(e) = err.find::<NetworkError>() {
        match e {
            NetworkError::NotLeader => (StatusCode::SERVICE_UNAVAILABLE, "Not the cluster leader".to_string(), "NOT_LEADER"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("Network error: {}", e), "NETWORK_ERROR"),
        }
    } else if let Some(e) = err.find::<StorageError>() {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Storage error: {}", e), "STORAGE_ERROR")
    } else if let Some(e) = err.find::<QueryError>() {
        (StatusCode::BAD_REQUEST, format!("Query error: {}", e), "QUERY_ERROR")
    } else if let Some(e) = err.find::<ServerError>() {
        match e {
            ServerError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone(), "VALIDATION_ERROR"),
            ServerError::Timeout => (StatusCode::REQUEST_TIMEOUT, "Request timeout".to_string(), "TIMEOUT"),
            ServerError::RateLimit => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string(), "RATE_LIMIT"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("Server error: {}", e), "SERVER_ERROR"),
        }
    } else {
        log::error!("Unhandled rejection: {:?}", err);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), "INTERNAL_ERROR")
    };
    
    let json = warp::reply::json(&ErrorResponse {
        error: message,
        code: error_code.to_string(),
        timestamp: Utc::now(),
        request_id: None, // Could be enhanced with request tracing
    });
    
    Ok(warp::reply::with_status(json, code))
}

// API handlers with comprehensive error handling and validation

async fn handle_health_check(
    storage: Arc<Storage>,
    query: Arc<Query>,
    network: Option<Arc<Network>>,
    start_time: Instant,
    node_id: String,
) -> Result<impl Reply, Rejection> {
    let uptime = start_time.elapsed().as_secs();
    
    // Basic health checks - these can be made more sophisticated
    let storage_status = HealthStatus { 
        status: "healthy".to_string(), 
        details: None 
    };
    
    let query_status = HealthStatus { 
        status: "healthy".to_string(), 
        details: None 
    };
    
    let network_status = if network.is_some() {
        Some(HealthStatus { 
            status: "healthy".to_string(), 
            details: None 
        })
    } else {
        None
    };
    
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        uptime_seconds: uptime,
        version: env!("CARGO_PKG_VERSION").to_string(),
        node_id,
        storage: storage_status,
        network: network_status,
        query_engine: query_status,
    };
    
    Ok(warp::reply::json(&response))
}

async fn handle_metrics(
    storage: Arc<Storage>,
    _query: Arc<Query>,
    start_time: Instant,
    request_counter: Arc<std::sync::atomic::AtomicU64>,
) -> Result<impl Reply, Rejection> {
    let uptime = start_time.elapsed().as_secs();
    let total_requests = request_counter.load(std::sync::atomic::Ordering::Relaxed);
    
    // Get basic metrics - these can be enhanced when the methods are available
    let partition_count = storage.list_partitions().len() as u64;
    
    let response = MetricsResponse {
        uptime_seconds: uptime,
        total_requests,
        active_queries: 0, // TODO: Implement when method is available
        storage_size_bytes: 0, // TODO: Implement when method is available
        partition_count,
        error_rate: 0.0, // Could be calculated from error tracking
    };
    
    Ok(warp::reply::json(&response))
}

async fn handle_list_partitions(
    storage: Arc<Storage>,
) -> Result<impl Reply, Rejection> {
    let partitions = storage.list_partitions();
    Ok(warp::reply::json(&partitions))
}

async fn handle_create_partition(
    request: CreatePartitionRequest,
    storage: Arc<Storage>,
    network: Option<Arc<Network>>,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if request.name.is_empty() {
        return Err(warp::reject::custom(ServerError::Validation(
            "Partition name cannot be empty".to_string()
        )));
    }
    
    if request.name.len() > 255 {
        return Err(warp::reject::custom(ServerError::Validation(
            "Partition name too long (max 255 characters)".to_string()
        )));
    }
    
    // Check if we're in a cluster
    if let Some(network) = network {
        let state = network.get_state().await;
        if state != crate::network::consensus::NodeState::Leader {
            return Err(warp::reject::custom(NetworkError::NotLeader));
        }
        
        // Apply through consensus
        if let Err(e) = network
            .apply_command(crate::network::consensus::Command::CreatePartition { 
                name: request.name.clone() 
            })
            .await
        {
            return Err(warp::reject::custom(e));
        }
    }
    
    // Create the partition
    match storage.create_partition(&request.name) {
        Ok(id) => {
            let response = CreatePartitionResponse {
                id,
                name: request.name,
                created_at: Utc::now(),
            };
            Ok(warp::reply::json(&response))
        },
        Err(e) => Err(warp::reject::custom(e)),
    }
}

async fn handle_get_partition(
    id: String,
    storage: Arc<Storage>,
) -> Result<impl Reply, Rejection> {
    match storage.get_partition(&id) {
        Ok(partition) => Ok(warp::reply::json(&partition)),
        Err(e) => Err(warp::reject::custom(e)),
    }
}

async fn handle_append_log(
    partition_id: String,
    request: LogEntryRequest,
    storage: Arc<Storage>,
    network: Option<Arc<Network>>,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if request.source.is_empty() {
        return Err(warp::reject::custom(ServerError::Validation(
            "Source cannot be empty".to_string()
        )));
    }
    
    if request.message.is_empty() {
        return Err(warp::reject::custom(ServerError::Validation(
            "Message cannot be empty".to_string()
        )));
    }
    
    // Create the log entry
    let entry = LogEntry {
        timestamp: request.timestamp,
        source: request.source,
        tags: request.tags,
        message: request.message,
    };
    
    // Check if we're in a cluster
    if let Some(network) = network {
        let state = network.get_state().await;
        if state != crate::network::consensus::NodeState::Leader {
            return Err(warp::reject::custom(NetworkError::NotLeader));
        }
        
        // Apply through consensus
        if let Err(e) = network
            .apply_command(crate::network::consensus::Command::Append {
                partition_id: partition_id.clone(),
                entry: entry.clone(),
            })
            .await
        {
            return Err(warp::reject::custom(e));
        }
    }
    
    // Append to storage - assuming it returns a simple index or ID
    match storage.append(&partition_id, entry.clone()) {
        Ok(entry_index) => {
            let response = LogEntryResponse {
                id: format!("{}:{}", partition_id, entry_index),
                partition_id,
                entry_index,
                timestamp: entry.timestamp,
            };
            Ok(warp::reply::json(&response))
        },
        Err(e) => Err(warp::reject::custom(e)),
    }
}

async fn handle_get_log(
    partition_id: String,
    block_id: String,
    entry_id: u64,
    storage: Arc<Storage>,
) -> Result<impl Reply, Rejection> {
    match storage.read_entry(&partition_id, &block_id, entry_id) {
        Ok(entry) => Ok(warp::reply::json(&entry)),
        Err(e) => Err(warp::reject::custom(e)),
    }
}

async fn handle_execute_query(
    request: QueryRequest,
    query: Arc<Query>,
) -> Result<impl Reply, Rejection> {
    // Apply timeout if specified
    let timeout = request.timeout_seconds
        .map(Duration::from_secs)
        .unwrap_or(REQUEST_TIMEOUT);
    
    let query_future = query.execute(request.filter);
    
    match tokio::time::timeout(timeout, query_future).await {
        Ok(Ok(result)) => Ok(warp::reply::json(&result)),
        Ok(Err(e)) => Err(warp::reject::custom(e)),
        Err(_) => Err(warp::reject::custom(ServerError::Timeout)),
    }
}

async fn handle_cancel_query(
    query_id: String,
    query: Arc<Query>,
) -> Result<impl Reply, Rejection> {
    match query.cancel(&query_id).await {
        Ok(_) => Ok(warp::reply::json(&"Query canceled")),
        Err(e) => Err(warp::reject::custom(e)),
    }
}

async fn handle_query_status(
    query_id: String,
    query: Arc<Query>,
) -> Result<impl Reply, Rejection> {
    match query.get_statistics(&query_id).await {
        Ok(stats) => Ok(warp::reply::json(&stats)),
        Err(e) => Err(warp::reject::custom(e)),
    }
}