// TimberDB: A high-performance distributed log database
// main.rs - Main entry point for the TimberDB application

use clap::{Command, Arg};
use std::process;

mod api;
mod config;
mod network;
mod query;
mod storage;
mod util;

use crate::api::server::Server;

#[tokio::main]
async fn main() {
    // Set up command line argument parsing
    let matches = Command::new("TimberDB")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TimberDB Team")
        .about("High-performance distributed log database with configurable compression")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to configuration file"),
        )
        .arg(
            Arg::new("data-dir")
                .long("data-dir")
                .value_name("DIRECTORY")
                .help("Data directory for TimberDB"),
        )
        .arg(
            Arg::new("listen")
                .long("listen")
                .value_name("ADDRESS")
                .help("Listen address for TimberDB (format: host:port)"),
        )
        .arg(
            Arg::new("peers")
                .long("peers")
                .value_name("ADDRESSES")
                .help("Comma-separated list of peer addresses (format: host:port)"),
        )
        .arg(
            Arg::new("node-id")
                .long("node-id")
                .value_name("ID")
                .help("Unique node ID"),
        )
        .get_matches();

    // Load configuration
    let config = match config::load_config(&matches) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Failed to load configuration: {}", err);
            process::exit(1);
        }
    };

    // Initialize logging
    if let Err(err) = util::logging::init_logging(&config) {
        eprintln!("Failed to initialize logging: {}", err);
        process::exit(1);
    }

    // Log startup information
    log::info!("Starting TimberDB v{}", env!("CARGO_PKG_VERSION"));
    log::info!("Node ID: {}", config.node.id);
    log::info!("Data directory: {}", config.storage.data_dir.display());
    log::info!("Listening on: {}", config.network.listen_addr);
    
    // Create and start the server
    match Server::new(config) {
        Ok(mut server) => {
            if let Err(err) = server.start().await {
                log::error!("Server error: {}", err);
                process::exit(1);
            }
        }
        Err(err) => {
            log::error!("Failed to create server: {}", err);
            process::exit(1);
        }
    }
}