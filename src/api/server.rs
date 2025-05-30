use crate::{TimberDB, Result, Error};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use warp::Filter;

/// API server configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// Address to bind the server to
    pub bind_address: SocketAddr,
    
    /// Enable request logging
    pub enable_logging: bool,
    
    /// Maximum request body size in bytes
    pub max_body_size: u64,
    
    /// Request timeout in seconds
    pub request_timeout: u64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        ApiConfig {
            bind_address: "127.0.0.1:8080".parse().unwrap(),
            enable_logging: true,
            max_body_size: 16 * 1024 * 1024, // 16MB
            request_timeout: 30,
        }
    }
}

/// TimberDB API server
pub struct ApiServer {
    config: ApiConfig,
    db: Arc<TimberDB>,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(db: TimberDB, config: ApiConfig) -> Self {
        ApiServer {
            config,
            db: Arc::new(db),
        }
    }
    
    /// Start the API server
    pub async fn start(self) -> Result<()> {
        let routes = super::create_routes(self.db.clone())
            .with(warp::trace::request());

        log::info!("Starting TimberDB API server on {}", self.config.bind_address);
        
        // Start server with graceful shutdown
        let (_, server) = warp::serve(routes)
            .bind_with_graceful_shutdown(self.config.bind_address, async {
                // Wait for shutdown signal
                signal::ctrl_c().await.ok();
                log::info!("Received shutdown signal, stopping API server...");
            });
        
        server.await;
        
        log::info!("API server stopped");
        Ok(())
    }
    
    /// Start the server in the background
    pub async fn start_background(self) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let handle = tokio::spawn(async move {
            self.start().await
        });
        
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, LogEntry};
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::time::{timeout, Duration};
    
    async fn create_test_db() -> (TimberDB, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.data_dir = temp_dir.path().to_path_buf();
        
        let db = TimberDB::open(config).await.unwrap();
        (db, temp_dir)
    }
    
    #[tokio::test]
    async fn test_server_creation() {
        let (db, _temp_dir) = create_test_db().await;
        let config = ApiConfig::default();
        
        let server = ApiServer::new(db, config);
        assert!(server.db.list_partitions().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_server_startup() {
        let (db, _temp_dir) = create_test_db().await;
        let mut config = ApiConfig::default();
        config.bind_address = "127.0.0.1:0".parse().unwrap(); // Use any available port
        
        let server = ApiServer::new(db, config);
        
        // Start server in background and immediately shutdown
        let handle = server.start_background().await.unwrap();
        
        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Send shutdown signal
        handle.abort();
        
        // Should complete without error (ignore the abort error)
        let _ = handle.await;
    }
}