// TimberDB: A high-performance distributed log database
// query/mod.rs - Query module for search and filtering

use std::sync::Arc;
use std::time::Duration;

use crate::config::QueryConfig;
use crate::storage::Storage;
use self::engine::{QueryEngine, QueryError, QueryResult, QueryStatistics, LogFilter};

// Public API exports
pub mod engine;

// Query interface for TimberDB
#[derive(Debug)]
pub struct Query {
    /// Query engine
    engine: Arc<QueryEngine>,
}

impl Query {
    /// Create a new query interface
    pub fn new(storage: Arc<Storage>, config: QueryConfig) -> Self {
        let engine = QueryEngine::new(storage, config);
        
        Query {
            engine: Arc::new(engine),
        }
    }
    
    /// Execute a query using a filter
    pub async fn execute(&self, filter: LogFilter) -> Result<QueryResult, QueryError> {
        self.engine.execute(filter).await
    }
    
    /// Cancel a running query
    pub async fn cancel(&self, query_id: &str) -> Result<(), QueryError> {
        self.engine.cancel(query_id).await
    }
    
    /// Get statistics for a query
    pub async fn get_statistics(&self, query_id: &str) -> Result<QueryStatistics, QueryError> {
        self.engine.get_statistics(query_id).await
    }
    
    /// Get the query engine
    pub fn engine(&self) -> Arc<QueryEngine> {
        self.engine.clone()
    }
    
    /// Shutdown the query interface
    pub async fn shutdown(&self) {
        self.engine.shutdown().await;
    }
}