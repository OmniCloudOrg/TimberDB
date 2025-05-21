// TimberDB: A high-performance distributed log database
// lib.rs - Main library entry point

//! # TimberDB
//!
//! TimberDB is a high-performance distributed log database with configurable compression
//! and block-based partitioning for storing and querying log data at scale.
//!
//! ## Features
//!
//! - Block-based storage with configurable rotation policies
//! - Multiple compression algorithms (LZ4, Zstd, Snappy, Gzip)
//! - Distributed architecture with Raft consensus
//! - SQL-like query language
//! - RESTful HTTP API
//! - High performance for both writing and querying
//! - Configurable retention policies
//!
//! ## Example
//!
//! ```rust,no_run
//! use timberdb::config::Config;
//! use timberdb::api::server::Server;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load configuration
//!     let config = Config::default();
//!
//!     // Create and start the server
//!     let mut server = Server::new(config)?;
//!     server.start().await?;
//!
//!     Ok(())
//! }
//! ```

pub mod api;
pub mod config;
pub mod network;
pub mod query;
pub mod storage;
pub mod util;

// Re-export main components for convenience
pub use api::server::Server;
pub use api::client::Client;
pub use config::Config;
pub use storage::block::LogEntry;
pub use query::engine::QueryResult;