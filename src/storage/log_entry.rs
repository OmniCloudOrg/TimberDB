use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single log entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    /// Timestamp when the log entry was created
    pub timestamp: DateTime<Utc>,
    
    /// Source identifier (e.g., service name, component)
    pub source: String,
    
    /// Log message content
    pub message: String,
    
    /// Key-value tags for metadata
    pub tags: HashMap<String, String>,
    
    /// Internal entry ID (set when stored)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_id: Option<u64>,
}

impl LogEntry {
    /// Create a new log entry with current timestamp
    pub fn new(source: impl Into<String>, message: impl Into<String>, tags: HashMap<String, String>) -> Self {
        LogEntry {
            timestamp: Utc::now(),
            source: source.into(),
            message: message.into(),
            tags,
            entry_id: None,
        }
    }
    
    /// Create a new log entry with specified timestamp
    pub fn with_timestamp(
        timestamp: DateTime<Utc>,
        source: impl Into<String>,
        message: impl Into<String>,
        tags: HashMap<String, String>,
    ) -> Self {
        LogEntry {
            timestamp,
            source: source.into(),
            message: message.into(),
            tags,
            entry_id: None,
        }
    }
    
    /// Create a builder for constructing log entries
    pub fn builder() -> LogEntryBuilder {
        LogEntryBuilder::new()
    }
    
    /// Get a tag value by key
    pub fn get_tag(&self, key: &str) -> Option<&String> {
        self.tags.get(key)
    }
    
    /// Check if entry has a specific tag
    pub fn has_tag(&self, key: &str) -> bool {
        self.tags.contains_key(key)
    }
    
    /// Check if entry has a tag with specific value
    pub fn has_tag_value(&self, key: &str, value: &str) -> bool {
        self.tags.get(key).map_or(false, |v| v == value)
    }
    
    /// Set the entry ID (internal use)
    pub(crate) fn set_entry_id(&mut self, id: u64) {
        self.entry_id = Some(id);
    }
    
    /// Validate the log entry
    pub fn validate(&self) -> Result<(), String> {
        if self.source.is_empty() {
            return Err("Source cannot be empty".to_string());
        }
        
        if self.message.is_empty() {
            return Err("Message cannot be empty".to_string());
        }
        
        // Check for reasonable size limits
        if self.source.len() > 1024 {
            return Err("Source is too long (max 1024 characters)".to_string());
        }
        
        if self.message.len() > 1024 * 1024 {
            return Err("Message is too long (max 1MB)".to_string());
        }
        
        // Validate tags
        if self.tags.len() > 100 {
            return Err("Too many tags (max 100)".to_string());
        }
        
        for (key, value) in &self.tags {
            if key.is_empty() {
                return Err("Tag key cannot be empty".to_string());
            }
            
            if key.len() > 256 {
                return Err("Tag key is too long (max 256 characters)".to_string());
            }
            
            if value.len() > 1024 {
                return Err("Tag value is too long (max 1024 characters)".to_string());
            }
        }
        
        Ok(())
    }
}

/// Builder for constructing log entries
#[derive(Debug, Default)]
pub struct LogEntryBuilder {
    timestamp: Option<DateTime<Utc>>,
    source: Option<String>,
    message: Option<String>,
    tags: HashMap<String, String>,
}

impl LogEntryBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the timestamp
    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
    
    /// Set the source
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
    
    /// Set the message
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
    
    /// Add a tag
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }
    
    /// Add multiple tags
    pub fn tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags.extend(tags);
        self
    }
    
    /// Build the log entry
    pub fn build(self) -> Result<LogEntry, String> {
        let source = self.source.ok_or("Source is required")?;
        let message = self.message.ok_or("Message is required")?;
        let timestamp = self.timestamp.unwrap_or_else(Utc::now);
        
        let entry = LogEntry {
            timestamp,
            source,
            message,
            tags: self.tags,
            entry_id: None,
        };
        
        entry.validate()?;
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    
    #[test]
    fn test_new_log_entry() {
        let mut tags = HashMap::new();
        tags.insert("level".to_string(), "info".to_string());
        
        let entry = LogEntry::new("test-service", "Test message", tags.clone());
        
        assert_eq!(entry.source, "test-service");
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.tags, tags);
        assert!(entry.entry_id.is_none());
    }
    
    #[test]
    fn test_builder() {
        let timestamp = Utc.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap();
        
        let entry = LogEntry::builder()
            .timestamp(timestamp)
            .source("test-service")
            .message("Test message")
            .tag("level", "info")
            .tag("component", "auth")
            .build()
            .unwrap();
        
        assert_eq!(entry.timestamp, timestamp);
        assert_eq!(entry.source, "test-service");
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.get_tag("level"), Some(&"info".to_string()));
        assert_eq!(entry.get_tag("component"), Some(&"auth".to_string()));
    }
    
    #[test]
    fn test_validation() {
        // Valid entry
        let entry = LogEntry::new("service", "message", HashMap::new());
        assert!(entry.validate().is_ok());
        
        // Empty source
        let mut entry = LogEntry::new("", "message", HashMap::new());
        assert!(entry.validate().is_err());
        
        // Empty message
        entry = LogEntry::new("service", "", HashMap::new());
        assert!(entry.validate().is_err());
        
        // Too many tags
        let mut tags = HashMap::new();
        for i in 0..101 {
            tags.insert(format!("key{}", i), "value".to_string());
        }
        entry = LogEntry::new("service", "message", tags);
        assert!(entry.validate().is_err());
    }
}