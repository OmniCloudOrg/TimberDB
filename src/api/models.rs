use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub version: String,
}

/// Create partition request
#[derive(Debug, Deserialize)]
pub struct CreatePartitionRequest {
    pub name: String,
}

/// Create partition response
#[derive(Debug, Serialize)]
pub struct CreatePartitionResponse {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Append log request
#[derive(Debug, Deserialize)]
pub struct AppendLogRequest {
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub message: String,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

/// Append log response
#[derive(Debug, Serialize)]
pub struct AppendLogResponse {
    pub entry_id: u64,
    pub partition_id: String,
    pub timestamp: DateTime<Utc>,
}

/// Query request
#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    /// Partition IDs to search (optional, searches all if empty)
    #[serde(default)]
    pub partitions: Vec<String>,
    
    /// Time range filter
    pub time_range: Option<TimeRange>,
    
    /// Source filter (exact match)
    pub source: Option<String>,
    
    /// Source prefix filter
    pub source_prefix: Option<String>,
    
    /// Message contains filter (case-insensitive)
    pub message_contains: Option<String>,
    
    /// Tag filters (all must match)
    #[serde(default)]
    pub tags: HashMap<String, String>,
    
    /// Tag exists filters (tag key must exist)
    #[serde(default)]
    pub tag_exists: Vec<String>,
    
    /// Maximum number of results
    pub limit: Option<usize>,
    
    /// Skip this many results
    pub offset: Option<usize>,
    
    /// Sort order (newest first by default)
    #[serde(default = "default_sort_desc")]
    pub sort_desc: bool,
    
    /// Query timeout in seconds
    pub timeout_seconds: Option<u64>,
}

fn default_sort_desc() -> bool {
    true
}

/// Time range for queries
#[derive(Debug, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Query response
#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub entries: Vec<crate::LogEntry>,
    pub total_count: u64,
    pub returned_count: usize,
    pub execution_time_ms: u64,
    pub partitions_searched: usize,
    pub truncated: bool,
}

/// Metrics response
#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub timestamp: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub partitions: PartitionMetrics,
    pub storage: StorageMetrics,
    pub queries: QueryMetrics,
}

#[derive(Debug, Serialize)]
pub struct PartitionMetrics {
    pub total_count: usize,
    pub total_entries: u64,
    pub total_size_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct StorageMetrics {
    pub total_size_bytes: u64,
    pub active_blocks: usize,
    pub sealed_blocks: usize,
}

#[derive(Debug, Serialize)]
pub struct QueryMetrics {
    pub total_queries: u64,
    pub avg_execution_time_ms: f64,
    pub error_rate: f64,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_type: String,
    pub timestamp: DateTime<Utc>,
    pub request_id: Option<String>,
}

impl From<crate::Error> for ErrorResponse {
    fn from(error: crate::Error) -> Self {
        let error_type = match &error {
            crate::Error::Io(_) => "io_error",
            crate::Error::Serialization(_) => "serialization_error",
            crate::Error::Config(_) => "config_error",
            crate::Error::PartitionNotFound(_) => "partition_not_found",
            crate::Error::EntryNotFound(_, _) => "entry_not_found",
            crate::Error::BlockNotFound(_) => "block_not_found",
            crate::Error::DatabaseInUse => "database_in_use",
            crate::Error::Storage(_) => "storage_error",
            crate::Error::Query(_) => "query_error",
            crate::Error::Compression(_) => "compression_error",
            crate::Error::InvalidFormat(_) => "invalid_format",
            crate::Error::Corruption(_) => "data_corruption",
            crate::Error::ResourceLimit(_) => "resource_limit",
        };
        
        ErrorResponse {
            error: error.to_string(),
            error_type: error_type.to_string(),
            timestamp: Utc::now(),
            request_id: None,
        }
    }
}

impl From<&QueryRequest> for crate::query::Query {
    fn from(req: &QueryRequest) -> Self {
        let mut query = crate::query::Query::new();
        
        query.partitions = req.partitions.clone();
        
        if let Some(time_range) = &req.time_range {
            query.time_range = Some((time_range.start, time_range.end));
        }
        
        query.source = req.source.clone();
        query.source_prefix = req.source_prefix.clone();
        query.message_contains = req.message_contains.clone();
        query.tags = req.tags.clone();
        query.tag_exists = req.tag_exists.clone();
        query.limit = req.limit;
        query.offset = req.offset;
        query.sort_desc = req.sort_desc;
        
        if let Some(timeout_seconds) = req.timeout_seconds {
            query.timeout = Some(Duration::from_secs(timeout_seconds));
        }
        
        query
    }
}

impl From<crate::query::QueryResult> for QueryResponse {
    fn from(result: crate::query::QueryResult) -> Self {
        QueryResponse {
            entries: result.entries,
            total_count: result.total_count,
            returned_count: result.returned_count,
            execution_time_ms: result.execution_time.as_millis() as u64,
            partitions_searched: result.partitions_searched,
            truncated: result.truncated,
        }
    }
}

impl From<&AppendLogRequest> for crate::LogEntry {
    fn from(req: &AppendLogRequest) -> Self {
        crate::LogEntry::with_timestamp(
            req.timestamp,
            req.source.clone(),
            req.message.clone(),
            req.tags.clone(),
        )
    }
}