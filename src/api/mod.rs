// TimberDB: A high-performance distributed log database
// api/mod.rs - API module for external interfaces

pub mod server;
pub mod client;

// Re-export main components
pub use server::{Server, ServerError};