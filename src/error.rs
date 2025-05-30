use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Partition not found: {0}")]
    PartitionNotFound(String),
    
    #[error("Entry not found: partition={0}, entry={1}")]
    EntryNotFound(String, u64),
    
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    #[error("Database is currently in use and cannot be closed")]
    DatabaseInUse,
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Query error: {0}")]
    Query(String),
    
    #[error("Compression error: {0}")]
    Compression(String),
    
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    
    #[error("Data corruption detected: {0}")]
    Corruption(String),
    
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
}

// Manual Clone implementation to handle the non-cloneable error types
impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Error::Io(e) => Error::Io(std::io::Error::new(e.kind(), e.to_string())),
            Error::Serialization(_) => Error::Serialization(bincode::Error::from(bincode::ErrorKind::Custom("Cloned serialization error".into()))),
            Error::Config(s) => Error::Config(s.clone()),
            Error::PartitionNotFound(s) => Error::PartitionNotFound(s.clone()),
            Error::EntryNotFound(s, id) => Error::EntryNotFound(s.clone(), *id),
            Error::BlockNotFound(s) => Error::BlockNotFound(s.clone()),
            Error::DatabaseInUse => Error::DatabaseInUse,
            Error::Storage(s) => Error::Storage(s.clone()),
            Error::Query(s) => Error::Query(s.clone()),
            Error::Compression(s) => Error::Compression(s.clone()),
            Error::InvalidFormat(s) => Error::InvalidFormat(s.clone()),
            Error::Corruption(s) => Error::Corruption(s.clone()),
            Error::ResourceLimit(s) => Error::ResourceLimit(s.clone()),
        }
    }
}