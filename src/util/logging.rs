// TimberDB: A high-performance distributed log database
// util/logging.rs - Logging configuration

use std::io::{self, Write};
use std::path::Path;

use chrono::Local;
use env_logger::Builder;
use log::LevelFilter;

use crate::config::Config;

/// Initialize logging with configuration
pub fn init_logging(config: &Config) -> Result<(), io::Error> {
    // Parse log level
    let level = match config.log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    };
    
    // Create log directory if it doesn't exist
    let log_dir = config.storage.data_dir.join("logs");
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)?;
    }
    
    // Configure environment logger
    let mut builder = Builder::new();
    builder
        .filter(None, level)
        .format(|buf, record| {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                timestamp,
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        });
    
    // Add file logger if log directory exists
    if log_dir.exists() {
        let log_file_path = log_dir.join(format!(
            "timberdb_{}.log",
            Local::now().format("%Y%m%d_%H%M%S")
        ));
        
        let log_file = std::fs::File::create(log_file_path)?;
        builder.target(env_logger::Target::Pipe(Box::new(log_file)));
    }
    
    // Initialize the logger
    builder.init();
    
    // Log the initialization
    log::info!("Logging initialized at level: {}", level);
    
    Ok(())
}

/// Configure logging to a specific file
pub fn configure_file_logging(
    log_path: &Path,
    level: LevelFilter,
) -> Result<(), io::Error> {
    // Create log directory if it doesn't exist
    if let Some(parent) = log_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    
    // Configure environment logger
    let mut builder = Builder::new();
    builder
        .filter(None, level)
        .format(|buf, record| {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                timestamp,
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        });
    
    // Add file logger
    let log_file = std::fs::File::create(log_path)?;
    builder.target(env_logger::Target::Pipe(Box::new(log_file)));
    
    // Initialize the logger
    builder.init();
    
    Ok(())
}