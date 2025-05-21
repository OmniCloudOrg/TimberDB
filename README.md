# TimberDB

A high-performance distributed log database with configurable compression and block-based storage.

![TimberDB](https://via.placeholder.com/800x200?text=TimberDB)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- **High-performance**: Optimized for fast writes and efficient queries
- **Block-based storage**: Data is organized in blocks for efficient storage and retrieval
- **Configurable compression**: Support for multiple compression algorithms (LZ4, Zstd, Snappy, Gzip)
- **Distributed architecture**: Built with scalability in mind using Raft consensus
- **Flexible partitioning**: Organize logs into logical partitions
- **Query engine**: SQL-like query language for filtering and retrieving logs
- **RESTful HTTP API**: Simple and powerful interface for integration
- **Configurable retention**: Manage data lifecycle with flexible retention policies
- **Clustering**: Built-in support for distributed operation

## Table of Contents

- [Installation](#installation)
- [Configuration](#configuration)
- [Command Line Interface](#command-line-interface)
- [HTTP API](#http-api)
- [Clustering](#clustering)
- [Storage Architecture](#storage-architecture)
- [Query Engine](#query-engine)
- [Performance Tuning](#performance-tuning)
- [Monitoring](#monitoring)
- [Development](#development)
- [License](#license)

## Installation

### Prerequisites

- Rust 1.70 or later
- Cargo package manager
- Linux, macOS, or Windows operating system

### Building from Source

Clone the repository and build the project:

```bash
git clone https://github.com/OmniCloudOrg/timberdb.git
cd timberdb
cargo build --release
```

The binary will be available at `target/release/timberdb`.

### Using Docker

```bash
docker pull timberdb/timberdb:latest
docker run -p 7777:7777 -v /path/to/data:/data timberdb/timberdb:latest
```

## Configuration

TimberDB uses a TOML configuration file with the following sections:

### Sample Configuration

```toml
# Node configuration
[node]
id = "node-1"  # Unique node identifier
role = "Leader"  # Leader, Follower, or Observer

# Storage configuration
[storage]
data_dir = "./data"  # Directory where TimberDB will store its data
block_rotation = { type = "time", value = 3600 }  # Rotate blocks every hour
compression = "LZ4"  # Compression algorithm (None, LZ4, Zstd, Snappy, Gzip)
compression_level = 3  # Compression level (algorithm specific)
retention_days = 30  # Keep logs for 30 days (0 = keep forever)
max_storage_size = 107374182400  # Max storage size in bytes (100GB, 0 = unlimited)
sync_writes = true  # Sync writes to disk for durability (may impact performance)

# Network configuration
[network]
listen_addr = "127.0.0.1:7777"  # Address to listen on for client and peer connections
peers = ["192.168.1.2:7777", "192.168.1.3:7777"]  # List of peer node addresses
timeout = 5  # Connection timeout in seconds
max_connections = 1000  # Maximum concurrent connections

# Optional TLS configuration
[network.tls]
cert_path = "/path/to/cert.pem"  # Path to certificate file
key_path = "/path/to/key.pem"  # Path to private key file
ca_path = "/path/to/ca.pem"  # Optional CA certificate path for mutual TLS

# Query engine configuration
[query]
max_results = 10000  # Maximum number of results to return
timeout = 30  # Query timeout in seconds
enable_cache = true  # Enable query cache
cache_size = 1000  # Cache size in entries

# Logging configuration
log_level = "info"  # Logging level (trace, debug, info, warn, error)
```

### Block Rotation Policies

TimberDB supports multiple block rotation policies:

- **Time**: Rotate blocks based on time (seconds)
  ```toml
  block_rotation = { type = "time", value = 3600 }  # 1 hour
  ```

- **Size**: Rotate blocks based on size (bytes)
  ```toml
  block_rotation = { type = "size", value = 1073741824 }  # 1GB
  ```

- **Count**: Rotate blocks based on entry count
  ```toml
  block_rotation = { type = "count", value = 1000000 }  # 1 million entries
  ```

### Compression Algorithms

TimberDB supports multiple compression algorithms:

- **None**: No compression
- **LZ4**: Fast compression/decompression with good compression ratio
- **Zstd**: Higher compression ratio but slower than LZ4
- **Snappy**: Optimized for speed rather than compression ratio
- **Gzip**: High compression ratio but slower compression/decompression

## Command Line Interface

TimberDB provides a command-line interface for managing and interacting with the database:

```
USAGE:
    timberdb [OPTIONS]

OPTIONS:
    -c, --config <FILE>          Path to configuration file
        --data-dir <DIRECTORY>   Data directory for TimberDB
        --listen <ADDRESS>       Listen address for TimberDB (format: host:port)
        --peers <ADDRESSES>      Comma-separated list of peer addresses (format: host:port)
        --node-id <ID>           Unique node ID
    -h, --help                   Print help information
    -V, --version                Print version information
```

### Examples

Start TimberDB with a configuration file:

```bash
timberdb --config config.toml
```

Start TimberDB with command-line options:

```bash
timberdb --data-dir ./data --listen 127.0.0.1:7777 --node-id node-1
```

## HTTP API

TimberDB provides a RESTful HTTP API for interacting with the database.

### Health Check

```
GET /health
```

Response:
```
OK
```

### Partition Management

#### List Partitions

```
GET /partitions
```

Response:
```json
["partition-1", "partition-2", "partition-3"]
```

#### Create Partition

```
POST /partitions
Content-Type: application/json

"my_partition"
```

Response:
```json
"1a2b3c4d-5e6f-7g8h-9i0j-1k2l3m4n5o6p"
```

#### Get Partition Info

```
GET /partitions/{id}
```

Response:
```json
{
  "id": "1a2b3c4d-5e6f-7g8h-9i0j-1k2l3m4n5o6p",
  "name": "my_partition",
  "created_at": "2023-06-15T12:34:56Z",
  "blocks": ["block-1", "block-2"],
  "active_block": "block-2",
  "entry_count": 12345,
  "size_bytes": 67890123
}
```

### Log Operations

#### Append Log Entry

```
POST /partitions/{id}/logs
Content-Type: application/json

{
  "timestamp": "2023-06-15T12:34:56Z",
  "source": "app-server-1",
  "tags": {
    "level": "error",
    "component": "auth",
    "environment": "production"
  },
  "message": "Authentication failed: invalid credentials"
}
```

Response:
```json
42
```

#### Get Log Entry

```
GET /partitions/{id}/blocks/{block_id}/logs/{entry_id}
```

Response:
```json
{
  "timestamp": "2023-06-15T12:34:56Z",
  "source": "app-server-1",
  "tags": {
    "level": "error",
    "component": "auth",
    "environment": "production"
  },
  "message": "Authentication failed: invalid credentials"
}
```

### Query Operations

#### Execute Query

```
POST /query
Content-Type: application/json

{
  "time_range": ["2023-06-15T00:00:00Z", "2023-06-15T23:59:59Z"],
  "source": "app-server-1",
  "tags": {
    "level": "error"
  },
  "message_contains": "Authentication failed",
  "limit": 100
}
```

Response:
```json
{
  "id": "query-1a2b3c",
  "execution_time": 0.123,
  "scanned_entries": 5000,
  "matched_entries": 42,
  "entries": [
    {
      "timestamp": "2023-06-15T12:34:56Z",
      "source": "app-server-1",
      "tags": {
        "level": "error",
        "component": "auth",
        "environment": "production"
      },
      "message": "Authentication failed: invalid credentials"
    },
    // ... more entries
  ],
  "error": null
}
```

#### Cancel Query

```
POST /query/{id}/cancel
```

Response:
```json
"Query canceled"
```

#### Get Query Status

```
GET /query/{id}
```

Response:
```json
{
  "id": "query-1a2b3c",
  "query_string": "...",
  "status": "Running",
  "start_time": "2023-06-15T12:34:56Z",
  "end_time": null,
  "execution_time": null,
  "scanned_entries": 3000,
  "matched_entries": 25,
  "error": null
}
```

## Client Libraries

### Rust

```rust
use timberdb::Client;
use std::collections::BTreeMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client
    let client = Client::new("http://localhost:7777")?;
    
    // Create a partition
    let partition_id = client.create_partition("my_partition").await?;
    
    // Append a log entry
    let mut tags = BTreeMap::new();
    tags.insert("level".to_string(), "info".to_string());
    tags.insert("component".to_string(), "auth".to_string());
    
    let entry_id = client.append_log(
        &partition_id,
        "app-server-1",
        "User logged in successfully",
        tags,
        None  // Use current timestamp
    ).await?;
    
    println!("Appended log entry with ID: {}", entry_id);
    
    // Query logs
    let mut filter = timberdb::query::engine::LogFilter::default();
    filter.source = Some("app-server-1".to_string());
    filter.limit = Some(10);
    
    let result = client.query(&filter, Some(5000)).await?;
    
    println!("Found {} matching entries", result.matched_entries);
    for entry in result.entries {
        println!("{} [{}] {}", entry.timestamp, entry.source, entry.message);
    }
    
    Ok(())
}
```

## Clustering

TimberDB uses the Raft consensus algorithm for distributed operation. This enables high availability and data consistency across multiple nodes.

### Node Roles

- **Leader**: Handles write operations and coordinates the cluster
- **Follower**: Replicates data from the leader
- **Observer**: Read-only node that doesn't participate in voting

### Setting Up a Cluster

1. Create a configuration file for each node:

   ```toml
   # node1.toml
   [node]
   id = "node-1"
   role = "Follower"  # Will become leader if elected
   
   [network]
   listen_addr = "192.168.1.1:7777"
   peers = ["192.168.1.2:7777", "192.168.1.3:7777"]
   ```

   ```toml
   # node2.toml
   [node]
   id = "node-2"
   role = "Follower"
   
   [network]
   listen_addr = "192.168.1.2:7777"
   peers = ["192.168.1.1:7777", "192.168.1.3:7777"]
   ```

   ```toml
   # node3.toml
   [node]
   id = "node-3"
   role = "Follower"
   
   [network]
   listen_addr = "192.168.1.3:7777"
   peers = ["192.168.1.1:7777", "192.168.1.2:7777"]
   ```

2. Start each node with its configuration:

   ```bash
   # On machine 1
   timberdb --config node1.toml
   
   # On machine 2
   timberdb --config node2.toml
   
   # On machine 3
   timberdb --config node3.toml
   ```

3. The nodes will automatically form a cluster and elect a leader.

### Cluster Operation

- Write operations are automatically forwarded to the leader node
- Read operations can be served by any node
- If the leader fails, a new leader is automatically elected
- The cluster can continue operating as long as a majority of nodes are available (quorum)

### Monitoring Cluster Status

You can check the cluster status via the HTTP API:

```
GET /cluster/status
```

Response:
```json
{
  "leader_id": "node-1",
  "term": 3,
  "nodes": [
    {"id": "node-1", "address": "192.168.1.1:7777", "state": "Leader", "last_seen": "2023-06-15T12:34:56Z"},
    {"id": "node-2", "address": "192.168.1.2:7777", "state": "Follower", "last_seen": "2023-06-15T12:34:55Z"},
    {"id": "node-3", "address": "192.168.1.3:7777", "state": "Follower", "last_seen": "2023-06-15T12:34:54Z"}
  ]
}
```

## Storage Architecture

TimberDB organizes data in a hierarchical structure:

- **Partitions**: Logical containers for logs
- **Blocks**: Physical storage units within partitions
- **Log Entries**: Individual log records within blocks

### Block Management

Blocks are the fundamental storage unit in TimberDB. Each block has:

- A unique identifier
- Metadata (creation time, entry count, size, etc.)
- An index for fast lookups
- The actual log data

Blocks can be in one of three states:

- **Active**: Currently accepting writes
- **Sealed**: No longer accepting writes but not yet compressed
- **Archived**: Compressed and optimized for storage

Block rotation happens automatically based on the configured policy (time, size, or entry count).

### Compression

Older blocks are automatically compressed using the configured algorithm. This provides a good balance between storage efficiency and query performance.

### Retention

TimberDB supports automatic data retention based on:

- Age (delete logs older than a specified number of days)
- Storage limit (delete oldest logs when storage limit is reached)

## Query Engine

TimberDB includes a powerful query engine for filtering and retrieving logs.

### Query Language

TimberDB supports a SQL-like query language:

```sql
SELECT * FROM my_partition
WHERE source = 'app-server-1'
  AND tags.level = 'error'
  AND message CONTAINS 'Authentication failed'
TIME RANGE ('2023-06-15T00:00:00Z', '2023-06-15T23:59:59Z')
LIMIT 100
```

### Query Filter

For programmatic access, you can use the LogFilter structure:

```rust
let mut filter = LogFilter {
    time_range: Some((start_time, end_time)),
    source: Some("app-server-1".to_string()),
    tags: HashMap::from([
        ("level".to_string(), "error".to_string()),
    ]),
    message_contains: Some("Authentication failed".to_string()),
    limit: Some(100),
};
```

### Query Execution

Queries are executed efficiently by:

1. Filtering partitions by time range
2. Using indexes to narrow down candidate blocks
3. Scanning matching blocks for entries that match the filter
4. Aggregating and returning results

Queries can be cancelled if they take too long or are no longer needed.

## Performance Tuning

### Block Rotation

Block size and rotation policy have a significant impact on performance:

- Smaller blocks provide faster queries but more overhead
- Larger blocks are more efficient for storage but slower for queries
- Time-based rotation is good for consistent query performance

### Compression Settings

Different compression algorithms offer different trade-offs:

- **LZ4**: Good balance of speed and compression ratio
- **Zstd**: Better compression ratio, slightly slower
- **Snappy**: Fastest, but lower compression ratio
- **Gzip**: Best compression ratio, but slowest

### Write Performance

To optimize write performance:

- Set `sync_writes = false` (reduces durability but improves throughput)
- Use size-based block rotation with larger blocks
- Use LZ4 or Snappy compression
- Run in a cluster to distribute write load

### Query Performance

To optimize query performance:

- Use partition filtering effectively
- Add useful tags for common query patterns
- Enable query cache for repetitive queries
- Consider using an observer node dedicated to queries

## Monitoring

TimberDB provides several ways to monitor its operation:

### Logging

TimberDB logs important events to help with monitoring and troubleshooting:

```
[2023-06-15 12:34:56.123 INFO server.rs:42] Starting TimberDB v0.1.0
[2023-06-15 12:34:56.234 INFO server.rs:43] Node ID: node-1
[2023-06-15 12:34:56.345 INFO server.rs:44] Data directory: /data
[2023-06-15 12:34:56.456 INFO server.rs:45] Listening on: 0.0.0.0:7777
```

### Metrics

TimberDB exposes metrics via the HTTP API:

```
GET /metrics
```

Response:
```json
{
  "uptime": 3600,
  "node_state": "Leader",
  "memory_usage": 1073741824,
  "storage": {
    "total_size": 10737418240,
    "used_size": 5368709120,
    "partitions": 10,
    "blocks": {
      "active": 10,
      "sealed": 20,
      "archived": 100
    }
  },
  "operations": {
    "writes": {
      "total": 1000000,
      "rate": 1000,
      "avg_latency": 0.005
    },
    "reads": {
      "total": 500000,
      "rate": 500,
      "avg_latency": 0.010
    },
    "queries": {
      "total": 100000,
      "rate": 100,
      "avg_latency": 0.050,
      "active": 5
    }
  }
}
```

### Health Checks

TimberDB provides a simple health check endpoint:

```
GET /health
```

Response:
```
OK
```

## Development

### Project Structure

```
timberdb/
├── src/
│   ├── api/          # HTTP API and client
│   ├── config/       # Configuration handling
│   ├── network/      # Network and clustering
│   ├── query/        # Query engine
│   ├── storage/      # Storage engine
│   └── util/         # Utility functions
├── tests/            # Integration tests
├── examples/         # Example code
├── benches/          # Benchmarks
└── docs/             # Documentation
```

### Building and Testing

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench

# Build documentation
cargo doc
```

### Contributing

We welcome contributions! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

TimberDB is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- The Raft consensus algorithm
- Rust and its amazing ecosystem
- All the open-source projects that made this possible