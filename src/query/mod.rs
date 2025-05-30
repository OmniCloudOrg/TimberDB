use crate::{Error, Result, LogEntry};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Query filter conditions
#[derive(Debug, Clone)]
pub struct Query {
    /// Partition IDs to search (empty = search all)
    pub partitions: Vec<String>,
    
    /// Time range filter
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    
    /// Source filter (exact match)
    pub source: Option<String>,
    
    /// Source prefix filter
    pub source_prefix: Option<String>,
    
    /// Message contains filter (case-insensitive)
    pub message_contains: Option<String>,
    
    /// Tag filters (all must match)
    pub tags: HashMap<String, String>,
    
    /// Tag exists filters (tag key must exist)
    pub tag_exists: Vec<String>,
    
    /// Maximum number of results
    pub limit: Option<usize>,
    
    /// Skip this many results
    pub offset: Option<usize>,
    
    /// Sort order (newest first by default)
    pub sort_desc: bool,
    
    /// Query timeout
    pub timeout: Option<Duration>,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            partitions: Vec::new(),
            time_range: None,
            source: None,
            source_prefix: None,
            message_contains: None,
            tags: HashMap::new(),
            tag_exists: Vec::new(),
            limit: Some(1000),
            offset: None,
            sort_desc: true,
            timeout: Some(Duration::from_secs(30)),
        }
    }
}

impl Query {
    /// Create a new empty query
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if a log entry matches this query
    pub fn matches(&self, entry: &LogEntry) -> bool {
        // Check time range
        if let Some((start, end)) = &self.time_range {
            if entry.timestamp < *start || entry.timestamp > *end {
                return false;
            }
        }
        
        // Check source
        if let Some(source) = &self.source {
            if entry.source != *source {
                return false;
            }
        }
        
        // Check source prefix
        if let Some(prefix) = &self.source_prefix {
            if !entry.source.starts_with(prefix) {
                return false;
            }
        }
        
        // Check message contains
        if let Some(contains) = &self.message_contains {
            if !entry.message.to_lowercase().contains(&contains.to_lowercase()) {
                return false;
            }
        }
        
        // Check tag filters
        for (key, value) in &self.tags {
            if !entry.has_tag_value(key, value) {
                return false;
            }
        }
        
        // Check tag exists filters
        for key in &self.tag_exists {
            if !entry.has_tag(key) {
                return false;
            }
        }
        
        true
    }
}

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Matching log entries
    pub entries: Vec<LogEntry>,
    
    /// Total number of entries found (before limit/offset)
    pub total_count: u64,
    
    /// Number of entries returned
    pub returned_count: usize,
    
    /// Query execution time
    pub execution_time: Duration,
    
    /// Number of partitions searched
    pub partitions_searched: usize,
    
    /// Whether the query was truncated due to limits
    pub truncated: bool,
}

/// Query builder for fluent API
pub struct QueryBuilder {
    pub storage: Arc<crate::storage::Storage>,
    pub query: Query,
}

impl QueryBuilder {
    /// Create a new query builder
    pub fn new(storage: Arc<crate::storage::Storage>) -> Self {
        QueryBuilder {
            storage,
            query: Query::new(),
        }
    }
    
    /// Filter by partition ID
    pub fn partition(mut self, partition_id: impl Into<String>) -> Self {
        self.query.partitions.push(partition_id.into());
        self
    }
    
    /// Filter by multiple partition IDs
    pub fn partitions(mut self, partition_ids: Vec<String>) -> Self {
        self.query.partitions.extend(partition_ids);
        self
    }
    
    /// Filter by time range
    pub fn time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.query.time_range = Some((start, end));
        self
    }
    
    /// Filter by time since (from time until now)
    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.query.time_range = Some((since, Utc::now()));
        self
    }
    
    /// Filter by last duration (e.g., last hour)
    pub fn last(mut self, duration: Duration) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::from_std(duration).unwrap_or_default();
        self.query.time_range = Some((start, end));
        self
    }
    
    /// Filter by exact source match
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.query.source = Some(source.into());
        self
    }
    
    /// Filter by source prefix
    pub fn source_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.query.source_prefix = Some(prefix.into());
        self
    }
    
    /// Filter by message content (case-insensitive)
    pub fn message_contains(mut self, text: impl Into<String>) -> Self {
        self.query.message_contains = Some(text.into());
        self
    }
    
    /// Filter by tag value
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.tags.insert(key.into(), value.into());
        self
    }
    
    /// Filter by tag existence
    pub fn tag_exists(mut self, key: impl Into<String>) -> Self {
        self.query.tag_exists.push(key.into());
        self
    }
    
    /// Set maximum number of results
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }
    
    /// Set number of results to skip
    pub fn offset(mut self, offset: usize) -> Self {
        self.query.offset = Some(offset);
        self
    }
    
    /// Sort results in ascending order (oldest first)
    pub fn sort_asc(mut self) -> Self {
        self.query.sort_desc = false;
        self
    }
    
    /// Sort results in descending order (newest first) - default
    pub fn sort_desc(mut self) -> Self {
        self.query.sort_desc = true;
        self
    }
    
    /// Set query timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.query.timeout = Some(timeout);
        self
    }
    
    /// Execute the query
    pub async fn execute(self) -> Result<QueryResult> {
        let start_time = std::time::Instant::now();
        
        // Execute query with timeout
        let result = if let Some(timeout) = self.query.timeout {
            tokio::time::timeout(timeout, self.execute_query()).await
                .map_err(|_| Error::Query("Query timeout".to_string()))?
        } else {
            self.execute_query().await
        }?;
        
        let execution_time = start_time.elapsed();
        
        Ok(QueryResult {
            entries: result.0.clone(),
            total_count: result.1,
            returned_count: result.0.len(),
            execution_time,
            partitions_searched: result.2,
            truncated: result.3,
        })
    }
    
    /// Internal query execution
    async fn execute_query(self) -> Result<(Vec<LogEntry>, u64, usize, bool)> {
        let mut all_entries = Vec::new();
        let mut total_count = 0u64;
        let mut partitions_searched = 0;
        
        // Determine which partitions to search
        let partitions_to_search = if self.query.partitions.is_empty() {
            // Search all partitions
            self.storage.get_all_partitions_for_query().await
        } else {
            // Search specified partitions
            let mut partitions = Vec::new();
            for partition_id in &self.query.partitions {
                if let Some(partition) = self.storage.get_partition_for_query(partition_id).await {
                    partitions.push(partition);
                }
            }
            partitions
        };
        
        // Search each partition
        for partition in partitions_to_search {
            partitions_searched += 1;
            
            let entries = if let Some((start, end)) = self.query.time_range {
                // Time range query
                partition.get_entries_in_range(start, end, self.query.limit).await?
            } else {
                // No time range specified, this would need a different implementation
                // For now, we'll search recent entries
                let end = Utc::now();
                let start = end - chrono::Duration::days(1); // Default to last day
                partition.get_entries_in_range(start, end, self.query.limit).await?
            };
            
            // Filter entries
            for entry in entries {
                if self.query.matches(&entry) {
                    all_entries.push(entry);
                    total_count += 1;
                }
            }
        }
        
        // Sort results
        if self.query.sort_desc {
            all_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        } else {
            all_entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        }
        
        // Apply offset and limit
        let total_before_pagination = all_entries.len();
        let offset = self.query.offset.unwrap_or(0);
        let limit = self.query.limit.unwrap_or(usize::MAX);
        
        if offset >= all_entries.len() {
            all_entries.clear();
        } else {
            let end = std::cmp::min(offset + limit, all_entries.len());
            all_entries = all_entries[offset..end].to_vec();
        }
        
        let truncated = total_before_pagination > all_entries.len();
        
        Ok((all_entries, total_count, partitions_searched, truncated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::HashMap;
    
    #[test]
    fn test_query_matches() {
        let mut tags = HashMap::new();
        tags.insert("level".to_string(), "error".to_string());
        tags.insert("component".to_string(), "auth".to_string());
        
        let entry = LogEntry::new("user-service", "Authentication failed", tags);
        
        // Test source filter
        let mut query = Query::new();
        query.source = Some("user-service".to_string());
        assert!(query.matches(&entry));
        
        query.source = Some("other-service".to_string());
        assert!(!query.matches(&entry));
        
        // Test source prefix filter
        query = Query::new();
        query.source_prefix = Some("user-".to_string());
        assert!(query.matches(&entry));
        
        query.source_prefix = Some("admin-".to_string());
        assert!(!query.matches(&entry));
        
        // Test message contains filter
        query = Query::new();
        query.message_contains = Some("authentication".to_string()); // case insensitive
        assert!(query.matches(&entry));
        
        query.message_contains = Some("authorization".to_string());
        assert!(!query.matches(&entry));
        
        // Test tag filters
        query = Query::new();
        query.tags.insert("level".to_string(), "error".to_string());
        assert!(query.matches(&entry));
        
        query.tags.insert("level".to_string(), "info".to_string());
        assert!(!query.matches(&entry));
        
        // Test tag exists filter
        query = Query::new();
        query.tag_exists.push("level".to_string());
        assert!(query.matches(&entry));
        
        query.tag_exists.push("nonexistent".to_string());
        assert!(!query.matches(&entry));
    }
    
    #[test]
    fn test_time_range_filter() {
        let timestamp = Utc.with_ymd_and_hms(2023, 6, 15, 12, 0, 0).unwrap();
        let entry = LogEntry::with_timestamp(
            timestamp,
            "service",
            "message",
            HashMap::new(),
        );
        
        let mut query = Query::new();
        
        // Entry should match if within range
        let start = Utc.with_ymd_and_hms(2023, 6, 15, 11, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2023, 6, 15, 13, 0, 0).unwrap();
        query.time_range = Some((start, end));
        assert!(query.matches(&entry));
        
        // Entry should not match if outside range
        let start = Utc.with_ymd_and_hms(2023, 6, 15, 13, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2023, 6, 15, 14, 0, 0).unwrap();
        query.time_range = Some((start, end));
        assert!(!query.matches(&entry));
    }
}