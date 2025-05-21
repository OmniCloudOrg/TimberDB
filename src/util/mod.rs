// TimberDB: A high-performance distributed log database
// util/mod.rs - Utility functions and modules

pub mod logging;

// Re-export main utilities
pub use logging::init_logging;