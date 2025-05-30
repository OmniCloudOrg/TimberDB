use super::models::*;
use crate::{TimberDB, Error, LogEntry};
use chrono::Utc;
use std::sync::Arc;
use warp::{http::StatusCode, reject, reply, Rejection, Reply};

/// Health check handler
pub async fn health_check() -> Result<impl Reply, Rejection> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    
    Ok(reply::json(&response))
}

/// List all partitions
pub async fn list_partitions(db: Arc<TimberDB>) -> Result<impl Reply, Rejection> {
    match db.list_partitions().await {
        Ok(partitions) => Ok(reply::json(&partitions)),
        Err(e) => {
            log::error!("Failed to list partitions: {}", e);
            Err(reject::custom(ApiError::from(e)))
        }
    }
}

/// Create a new partition
pub async fn create_partition(
    request: CreatePartitionRequest,
    db: Arc<TimberDB>,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if request.name.is_empty() {
        return Err(reject::custom(ApiError::from(Error::InvalidFormat(
            "Partition name cannot be empty".to_string(),
        ))));
    }
    
    if request.name.len() > 255 {
        return Err(reject::custom(ApiError::from(Error::InvalidFormat(
            "Partition name too long (max 255 characters)".to_string(),
        ))));
    }
    
    match db.create_partition(&request.name).await {
        Ok(partition_id) => {
            let response = CreatePartitionResponse {
                id: partition_id,
                name: request.name,
                created_at: Utc::now(),
            };
            
            Ok(reply::with_status(
                reply::json(&response),
                StatusCode::CREATED,
            ))
        }
        Err(e) => {
            log::error!("Failed to create partition '{}': {}", request.name, e);
            Err(reject::custom(ApiError::from(e)))
        }
    }
}

/// Get partition information
pub async fn get_partition(
    partition_id: String,
    db: Arc<TimberDB>,
) -> Result<impl Reply, Rejection> {
    match db.get_partition(&partition_id).await {
        Ok(partition) => Ok(reply::json(&partition)),
        Err(e) => {
            log::error!("Failed to get partition '{}': {}", partition_id, e);
            Err(reject::custom(ApiError::from(e)))
        }
    }
}

/// Append a log entry to a partition
pub async fn append_log(
    partition_id: String,
    request: AppendLogRequest,
    db: Arc<TimberDB>,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if request.source.is_empty() {
        return Err(reject::custom(ApiError::from(Error::InvalidFormat(
            "Source cannot be empty".to_string(),
        ))));
    }
    
    if request.message.is_empty() {
        return Err(reject::custom(ApiError::from(Error::InvalidFormat(
            "Message cannot be empty".to_string(),
        ))));
    }
    
    let entry = LogEntry::from(&request);
    
    // Validate entry
    if let Err(validation_error) = entry.validate() {
        return Err(reject::custom(ApiError::from(Error::InvalidFormat(
            validation_error,
        ))));
    }
    
    match db.append(&partition_id, entry).await {
        Ok(entry_id) => {
            let response = AppendLogResponse {
                entry_id,
                partition_id,
                timestamp: request.timestamp,
            };
            
            Ok(reply::with_status(
                reply::json(&response),
                StatusCode::CREATED,
            ))
        }
        Err(e) => {
            log::error!("Failed to append log to partition '{}': {}", partition_id, e);
            Err(reject::custom(ApiError::from(e)))
        }
    }
}

/// Get a specific log entry
pub async fn get_log(
    partition_id: String,
    entry_id: u64,
    db: Arc<TimberDB>,
) -> Result<impl Reply, Rejection> {
    match db.get_entry(&partition_id, entry_id).await {
        Ok(entry) => Ok(reply::json(&entry)),
        Err(e) => {
            log::error!(
                "Failed to get entry {} from partition '{}': {}",
                entry_id, partition_id, e
            );
            Err(reject::custom(ApiError::from(e)))
        }
    }
}

/// Query log entries
pub async fn query_logs(
    request: QueryRequest,
    db: Arc<TimberDB>,
) -> Result<impl Reply, Rejection> {
    // Validate request
    if let Some(limit) = request.limit {
        if limit > 10_000 {
            return Err(reject::custom(ApiError::from(Error::InvalidFormat(
                "Limit cannot exceed 10,000".to_string(),
            ))));
        }
    }
    
    if let Some(timeout_seconds) = request.timeout_seconds {
        if timeout_seconds > 300 {
            return Err(reject::custom(ApiError::from(Error::InvalidFormat(
                "Timeout cannot exceed 300 seconds".to_string(),
            ))));
        }
    }
    
    // Convert request to internal query
    let query = crate::query::Query::from(&request);
    
    match execute_query_direct(&db, query).await {
        Ok(result) => {
            let response = QueryResponse::from(result);
            Ok(reply::json(&response))
        }
        Err(e) => {
            log::error!("Query failed: {}", e);
            Err(reject::custom(ApiError::from(e)))
        }
    }
}

/// Get system metrics
pub async fn get_metrics(db: Arc<TimberDB>) -> Result<impl Reply, Rejection> {
    match collect_metrics(&db).await {
        Ok(metrics) => Ok(reply::json(&metrics)),
        Err(e) => {
            log::error!("Failed to collect metrics: {}", e);
            Err(reject::custom(ApiError::from(e)))
        }
    }
}

/// Collect system metrics
async fn collect_metrics(db: &TimberDB) -> Result<MetricsResponse, Error> {
    let partitions = db.list_partitions().await?;
    
    let mut total_entries = 0u64;
    let mut total_size = 0u64;
    
    for partition in &partitions {
        total_entries += partition.total_entries;
        total_size += partition.total_size;
    }
    
    let response = MetricsResponse {
        timestamp: Utc::now(),
        uptime_seconds: 0, // TODO: Track actual uptime
        partitions: PartitionMetrics {
            total_count: partitions.len(),
            total_entries,
            total_size_bytes: total_size,
        },
        storage: StorageMetrics {
            total_size_bytes: total_size,
            active_blocks: 0, // TODO: Track active blocks
            sealed_blocks: 0, // TODO: Track sealed blocks
        },
        queries: QueryMetrics {
            total_queries: 0,    // TODO: Track query stats
            avg_execution_time_ms: 0.0,
            error_rate: 0.0,
        },
    };
    
    Ok(response)
}

/// Execute a query directly (workaround for API usage)
async fn execute_query_direct(
    db: &TimberDB,
    query: crate::query::Query,
) -> crate::Result<crate::query::QueryResult> {
    let _start_time = std::time::Instant::now();
    
    // Create a new builder with the same storage
    let mut builder = db.query();
    
    // Apply query parameters
    if !query.partitions.is_empty() {
        builder = builder.partitions(query.partitions);
    }
    
    if let Some((start, end)) = query.time_range {
        builder = builder.time_range(start, end);
    }
    
    if let Some(source) = query.source {
        builder = builder.source(source);
    }
    
    if let Some(prefix) = query.source_prefix {
        builder = builder.source_prefix(prefix);
    }
    
    if let Some(contains) = query.message_contains {
        builder = builder.message_contains(contains);
    }
    
    for (key, value) in query.tags {
        builder = builder.tag(key, value);
    }
    
    for key in query.tag_exists {
        builder = builder.tag_exists(key);
    }
    
    if let Some(limit) = query.limit {
        builder = builder.limit(limit);
    }
    
    if let Some(offset) = query.offset {
        builder = builder.offset(offset);
    }
    
    if query.sort_desc {
        builder = builder.sort_desc();
    } else {
        builder = builder.sort_asc();
    }
    
    if let Some(timeout) = query.timeout {
        builder = builder.timeout(timeout);
    }
    
    builder.execute().await
}

/// Custom error type for API
#[derive(Debug)]
pub struct ApiError {
    error: Error,
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        ApiError { error }
    }
}

impl reject::Reject for ApiError {}

/// Handle rejections and convert them to JSON responses
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    let (code, error_response) = if err.is_not_found() {
        (
            StatusCode::NOT_FOUND,
            ErrorResponse {
                error: "Resource not found".to_string(),
                error_type: "not_found".to_string(),
                timestamp: Utc::now(),
                request_id: None,
            },
        )
    } else if let Some(api_error) = err.find::<ApiError>() {
        let status_code = match &api_error.error {
            Error::PartitionNotFound(_) => StatusCode::NOT_FOUND,
            Error::EntryNotFound(_, _) => StatusCode::NOT_FOUND,
            Error::BlockNotFound(_) => StatusCode::NOT_FOUND,
            Error::InvalidFormat(_) => StatusCode::BAD_REQUEST,
            Error::Config(_) => StatusCode::BAD_REQUEST,
            Error::Query(_) => StatusCode::BAD_REQUEST,
            Error::ResourceLimit(_) => StatusCode::BAD_REQUEST,
            Error::DatabaseInUse => StatusCode::CONFLICT,
            Error::Corruption(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Compression(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Serialization(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        
        (status_code, ErrorResponse::from(api_error.error.clone()))
    } else if err.find::<warp::filters::body::BodyDeserializeError>().is_some() {
        (
            StatusCode::BAD_REQUEST,
            ErrorResponse {
                error: "Invalid request body".to_string(),
                error_type: "invalid_request_body".to_string(),
                timestamp: Utc::now(),
                request_id: None,
            },
        )
    } else if err.find::<warp::reject::PayloadTooLarge>().is_some() {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorResponse {
                error: "Request payload too large".to_string(),
                error_type: "payload_too_large".to_string(),
                timestamp: Utc::now(),
                request_id: None,
            },
        )
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            ErrorResponse {
                error: "Method not allowed".to_string(),
                error_type: "method_not_allowed".to_string(),
                timestamp: Utc::now(),
                request_id: None,
            },
        )
    } else {
        log::error!("Unhandled rejection: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorResponse {
                error: "Internal server error".to_string(),
                error_type: "internal_server_error".to_string(),
                timestamp: Utc::now(),
                request_id: None,
            },
        )
    };
    
    Ok(reply::with_status(reply::json(&error_response), code))
}