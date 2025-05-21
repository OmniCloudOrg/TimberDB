// TimberDB: A high-performance distributed log database
// api/client.rs - HTTP client for TimberDB

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::storage::block::LogEntry;
use crate::query::engine::{QueryResult, QueryStatistics, LogFilter};

// Client errors
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Not the leader")]
    NotLeader,
}

// API error response
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

// LogEntry request
#[derive(Debug, Serialize)]
struct LogEntryRequest {
    timestamp: Option<DateTime<Utc>>,
    source: String,
    tags: BTreeMap<String, String>,
    message: String,
}

// TimberDB client
#[derive(Debug, Clone)]
pub struct Client {
    /// Base URL for the TimberDB server
    base_url: Url,
    /// HTTP client
    client: HttpClient,
}

impl Client {
    /// Create a new client
    pub fn new(base_url: &str) -> Result<Self, ClientError> {
        // Parse base URL and ensure it ends with "/"
        let mut base_url = Url::parse(base_url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        
        // Create HTTP client
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        Ok(Client {
            base_url,
            client,
        })
    }
    
    /// Create a new client with custom options
    pub fn with_options(
        base_url: &str,
        timeout: Duration,
        user_agent: Option<&str>,
    ) -> Result<Self, ClientError> {
        // Parse base URL and ensure it ends with "/"
        let mut base_url = Url::parse(base_url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        
        // Create HTTP client with options
        let mut builder = HttpClient::builder().timeout(timeout);
        
        if let Some(user_agent) = user_agent {
            builder = builder.user_agent(user_agent);
        }
        
        let client = builder.build()?;
        
        Ok(Client {
            base_url,
            client,
        })
    }
    
    /// Health check
    pub async fn health_check(&self) -> Result<bool, ClientError> {
        let url = self.base_url.join("health")?;
        
        let response = self.client.get(url).send().await?;
        
        if response.status().is_success() {
            Ok(true)
        } else {
            Err(ClientError::Api(
                response.text().await?.to_string(),
            ))
        }
    }
    
    /// List all partitions
    pub async fn list_partitions(&self) -> Result<Vec<String>, ClientError> {
        let url = self.base_url.join("partitions")?;
        
        let response = self.client.get(url).send().await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
    
    /// Create a new partition
    pub async fn create_partition(&self, name: &str) -> Result<String, ClientError> {
        let url = self.base_url.join("partitions")?;
        
        let response = self.client.post(url).json(&name).send().await?;
        
        if response.status().is_success() {
            Ok(response.text().await?)
        } else if response.status().as_u16() == 403 {
            Err(ClientError::NotLeader)
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
    
    /// Get partition info
    pub async fn get_partition(&self, id: &str) -> Result<serde_json::Value, ClientError> {
        let url = self.base_url.join(&format!("partitions/{}", id))?;
        
        let response = self.client.get(url).send().await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
    
    /// Append a log entry to a partition
    pub async fn append_log(
        &self,
        partition_id: &str,
        source: &str,
        message: &str,
        tags: BTreeMap<String, String>,
        timestamp: Option<DateTime<Utc>>,
    ) -> Result<u64, ClientError> {
        let url = self.base_url.join(&format!("partitions/{}/logs", partition_id))?;
        
        let request = LogEntryRequest {
            timestamp,
            source: source.to_string(),
            tags,
            message: message.to_string(),
        };
        
        let response = self.client.post(url).json(&request).send().await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else if response.status().as_u16() == 403 {
            Err(ClientError::NotLeader)
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
    
    /// Get a log entry
    pub async fn get_log(
        &self,
        partition_id: &str,
        block_id: &str,
        entry_id: u64,
    ) -> Result<LogEntry, ClientError> {
        let url = self.base_url.join(
            &format!("partitions/{}/blocks/{}/logs/{}", 
                partition_id, block_id, entry_id)
        )?;
        
        let response = self.client.get(url).send().await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
    
    /// Execute a query with a filter
    pub async fn query(
        &self,
        filter: &LogFilter,
        timeout_ms: Option<u64>,
    ) -> Result<QueryResult, ClientError> {
        let url = self.base_url.join("query")?;
        
        let mut request_builder = self.client.post(url).json(filter);
        
        if let Some(timeout) = timeout_ms {
            request_builder = request_builder.timeout(Duration::from_millis(timeout));
        }
        
        let response = request_builder.send().await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
    
    /// Cancel a running query
    pub async fn cancel_query(&self, query_id: &str) -> Result<(), ClientError> {
        let url = self.base_url.join(&format!("query/{}/cancel", query_id))?;
        
        let response = self.client.post(url).send().await?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
    
    /// Get query status
    pub async fn query_status(&self, query_id: &str) -> Result<QueryStatistics, ClientError> {
        let url = self.base_url.join(&format!("query/{}", query_id))?;
        
        let response = self.client.get(url).send().await?;
        
        if response.status().is_success() {
            let stats: QueryStatistics = response.json().await?;
            Ok(stats)
        } else {
            let error: ErrorResponse = response.json().await?;
            Err(ClientError::Api(error.error))
        }
    }
}