// TimberDB: A high-performance distributed log database
// api/server.rs - HTTP API server

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::signal;
use warp::{Filter, Reply, Rejection};
use warp::http::StatusCode;
use warp::reject::Reject;

use crate::config::Config;
use crate::network::{Network, NetworkError};
use crate::query::{Query, self};
use crate::query::engine::{QueryError, LogFilter};
use crate::storage::{Storage, Error as StorageError};
use crate::storage::block::LogEntry;

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
}

// Make NetworkError, StorageError, and QueryError implement Reject
impl Reject for NetworkError {}
impl Reject for StorageError {}
impl Reject for QueryError {}

// API error response
#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
}

// LogEntry request
#[derive(Debug, Serialize, Deserialize)]
struct LogEntryRequest {
    timestamp: Option<DateTime<Utc>>,
    source: String,
    tags: BTreeMap<String, String>,
    message: String,
}

// TimberDB server
#[derive(Debug)]
pub struct Server {
    /// Server configuration
    config: Config,
    /// Storage engine
    storage: Arc<Storage>,
    /// Query engine
    query: Arc<Query>,
    /// Network layer
    network: Option<Arc<Network>>,
    /// Server shutdown flag
    shutdown: Arc<tokio::sync::watch::Sender<bool>>,
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
            network: None, // Will be initialized in start()
            shutdown: Arc::new(shutdown_tx),
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
        
        log::info!("Starting HTTP API server on {}", addr);
        
        // Create shutdown receiver
        let mut shutdown_rx = self.shutdown.subscribe();
        
        // Spawn the server task
        let server = warp::serve(routes).bind(addr);
        
        // Use tokio::select! to handle graceful shutdown
        tokio::select! {
            _ = async { server.await; } => {
                log::info!("Server stopped");
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
        
        Ok(())
    }
    
    /// Handle OS signals for graceful shutdown in a platform-agnostic way
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
    
    /// Create API routes
    fn create_routes(
        &self,
    ) -> impl Filter<Extract = impl Reply, Error = Infallible> + Clone {
        // Create application state
        let storage = self.storage.clone();
        let query = self.query.clone();
        let network = self.network.clone();
        
        // Health check route
        let health_route = warp::path("health")
            .and(warp::get())
            .map(|| "OK");
        
        // Partitions routes
        let partitions_list = warp::path("partitions")
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_list_partitions);
        
        let partition_create = warp::path("partitions")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_storage(storage.clone()))
            .and(with_network(network.clone()))
            .and_then(handle_create_partition);
        
        let partition_get = warp::path!("partitions" / String)
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_get_partition);
        
        // Logs routes
        let logs_append = warp::path!("partitions" / String / "logs")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_storage(storage.clone()))
            .and(with_network(network.clone()))
            .and_then(handle_append_log);
        
        let logs_get = warp::path!("partitions" / String / "blocks" / String / "logs" / u64)
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_get_log);
        
        // Query routes
        let query_execute = warp::path("query")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_query(query.clone()))
            .and_then(handle_execute_query);
        
        let query_cancel = warp::path!("query" / String / "cancel")
            .and(warp::post())
            .and(with_query(query.clone()))
            .and_then(handle_cancel_query);
        
        let query_status = warp::path!("query" / String)
            .and(warp::get())
            .and(with_query(query.clone()))
            .and_then(handle_query_status);
        
        // Combine routes with error handling
        health_route
            .or(partitions_list)
            .or(partition_create)
            .or(partition_get)
            .or(logs_append)
            .or(logs_get)
            .or(query_execute)
            .or(query_cancel)
            .or(query_status)
            .recover(handle_rejection)
    }
    
    /// Shutdown the server
    pub async fn shutdown(&self) {
        // Signal server shutdown
        let _ = self.shutdown.send(true);
        
        // Shutdown components
        if let Some(network) = &self.network {
            network.shutdown().await;
        }
        
        self.query.shutdown().await;
        
        // Shutdown storage
        let storage = Arc::clone(&self.storage);
        let storage_result = tokio::task::spawn_blocking(move || {
            // Consume the Arc and drop the Storage instance
            match Arc::try_unwrap(storage) {
                Ok(storage) => storage.shutdown(),
                Err(_) => {
                    log::warn!("Storage still has multiple references");
                    Ok(())
                }
            }
        }).await;
        
        if let Err(e) = storage_result {
            log::error!("Error joining storage shutdown task: {}", e);
        }
        
        log::info!("Server shutdown complete");
    }
}

// Warp filters to provide components to handlers
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

// Handle API rejections
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message: String;
    
    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "Not found".to_string();
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        code = StatusCode::BAD_REQUEST;
        message = format!("Invalid request: {}", e);
    } else if let Some(e) = err.find::<NetworkError>() {
        code = StatusCode::FORBIDDEN;
        message = format!("Network error: {}", e);
    } else if let Some(e) = err.find::<StorageError>() {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = format!("Storage error: {}", e);
    } else if let Some(e) = err.find::<QueryError>() {
        code = StatusCode::BAD_REQUEST;
        message = format!("Query error: {}", e);
    } else {
        // Internal server error
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal server error".to_string();
        log::error!("Unhandled rejection: {:?}", err);
    }
    
    let json = warp::reply::json(&ErrorResponse {
        error: message,
    });
    
    Ok(warp::reply::with_status(json, code))
}

// API handlers
async fn handle_list_partitions(
    storage: Arc<Storage>,
) -> Result<impl Reply, Rejection> {
    let partitions = storage.list_partitions();
    
    // Extract just the IDs for a cleaner API
    let partition_ids: Vec<String> = partitions.iter()
        .map(|p| p.id.clone())
        .collect();
    
    Ok(warp::reply::json(&partition_ids))
}

async fn handle_create_partition(
    name: String,
    storage: Arc<Storage>,
    network: Option<Arc<Network>>,
) -> Result<impl Reply, Rejection> {
    // Check if we're in a cluster
    if let Some(network) = network {
        // Check if we're the leader
        let state = network.get_state().await;
        if state != crate::network::consensus::NodeState::Leader {
            // Forward to leader if known
            if let Some(_) = network.get_leader().await {
                return Err(warp::reject::custom(NetworkError::NotLeader));
            } else {
                return Err(warp::reject::custom(NetworkError::NotLeader));
            }
        }
        
        // Apply through consensus
        if let Err(e) = network
            .apply_command(crate::network::consensus::Command::CreatePartition { name: name.clone() })
            .await
        {
            return Err(warp::reject::custom(e));
        }
    }
    
    // Create the partition
    match storage.create_partition(&name) {
        Ok(id) => Ok(warp::reply::json(&id)),
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
    log_entry: LogEntryRequest,
    storage: Arc<Storage>,
    network: Option<Arc<Network>>,
) -> Result<impl Reply, Rejection> {
    // Create the log entry
    let entry = LogEntry {
        timestamp: log_entry.timestamp.unwrap_or_else(Utc::now),
        source: log_entry.source,
        tags: log_entry.tags,
        message: log_entry.message,
    };
    
    // Check if we're in a cluster
    if let Some(network) = network {
        // Check if we're the leader
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
    
    // Append to storage
    match storage.append(&partition_id, entry) {
        Ok(entry_id) => Ok(warp::reply::json(&entry_id)),
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
    filter: LogFilter,
    query: Arc<Query>,
) -> Result<impl Reply, Rejection> {
    match query.execute(filter).await {
        Ok(result) => Ok(warp::reply::json(&result)),
        Err(e) => Err(warp::reject::custom(e)),
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