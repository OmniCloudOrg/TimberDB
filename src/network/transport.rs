// TimberDB: A high-performance distributed log database
// network/transport.rs - Network transport implementation (simplified)

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

use crate::config::NetworkConfig;
use crate::network::consensus::RaftMessage;

// Transport errors
#[derive(Error, Debug)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Connection timeout")]
    Timeout,
    
    #[error("Connection closed")]
    Closed,
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Not connected to peer: {0}")]
    NotConnected(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

// Message types for network transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Raft consensus message
    Raft(RaftMessage),
    
    /// Direct query to execute
    Query {
        query_id: String,
        query_string: String,
    },
    
    /// Query response
    QueryResponse {
        query_id: String,
        success: bool,
        results: Vec<u8>,
        error: Option<String>,
    },
    
    /// Heartbeat to keep connection alive
    Heartbeat,
    
    /// Handshake message for connection establishment
    Handshake {
        node_id: String,
        cluster_id: String,
        timestamp: u64,
    },
}

// Command types for the transport module
#[derive(Debug)]
pub enum TransportCommand {
    /// Send message to a specific peer
    Send {
        peer_id: String,
        message: Message,
        response: oneshot::Sender<Result<(), TransportError>>,
    },
    
    /// Broadcast message to all peers
    Broadcast {
        message: Message,
        response: oneshot::Sender<HashMap<String, Result<(), TransportError>>>,
    },
    
    /// Connect to a new peer
    Connect {
        peer_id: String,
        addr: SocketAddr,
        response: oneshot::Sender<Result<(), TransportError>>,
    },
    
    /// Disconnect from a peer
    Disconnect {
        peer_id: String,
        response: oneshot::Sender<Result<(), TransportError>>,
    },
    
    /// Shutdown the transport
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

// Peer connection state
#[derive(Debug)]
struct PeerConnection {
    /// Peer ID
    id: String,
    /// Peer address
    addr: SocketAddr,
    /// Send channel for the connection
    tx: mpsc::Sender<Message>,
    /// Last activity timestamp
    last_activity: Arc<Mutex<std::time::Instant>>,
}

// Network transport implementation
#[derive(Debug, Clone)]
pub struct Transport {
    /// Node ID
    node_id: String,
    /// Cluster ID
    cluster_id: String,
    /// Network configuration
    config: Arc<NetworkConfig>,
    /// Connected peers
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    /// Message sender channel
    message_tx: mpsc::Sender<Message>,
    /// Message receiver channel
    message_rx: Arc<Mutex<Option<mpsc::Receiver<Message>>>>,
    /// Command channel
    command_tx: mpsc::Sender<TransportCommand>,
}

impl Transport {
    /// Create a new transport instance
    pub async fn new(
        node_id: String,
        cluster_id: String,
        config: NetworkConfig,
    ) -> Result<Self, TransportError> {
        // Create channels
        let (message_tx, message_rx) = mpsc::channel(1000);
        let (command_tx, command_rx) = mpsc::channel(100);
        
        // Create shared config
        let config_arc = Arc::new(config);
        
        // Create transport instance
        let transport = Transport {
            node_id: node_id.clone(),
            cluster_id: cluster_id.clone(),
            config: config_arc.clone(),
            peers: Arc::new(RwLock::new(HashMap::new())),
            message_tx: message_tx.clone(),
            command_tx: command_tx.clone(),
            message_rx: Arc::new(Mutex::new(Some(message_rx))),
        };
        
        // Start listener
        let listener_addr = config_arc.listen_addr;
        let listener_node_id = node_id.clone();
        let listener_cluster_id = cluster_id.clone();
        let listener_peers = transport.peers.clone();
        let listener_tx = message_tx.clone();
        
        let config_arc_listener = config_arc.clone();
        tokio::spawn(async move {
            if let Err(e) = listen(
                listener_addr,
                listener_node_id,
                listener_cluster_id,
                listener_peers,
                listener_tx,
                config_arc_listener,
            ).await {
                log::error!("Listener error: {}", e);
            }
        });
        
        // Start command processor
        let processor_peers = transport.peers.clone();
        let processor_node_id = node_id.clone();
        let processor_cluster_id = cluster_id.clone();
        let processor_config = config_arc.clone();
        let processor_message_rx = transport.take_receiver().unwrap();
        
        tokio::spawn(async move {
            process_commands(
                command_rx,
                processor_message_rx,
                processor_peers,
                processor_node_id,
                processor_cluster_id,
                processor_config,
            ).await;
        });
        
        Ok(transport)
    }

    pub fn take_receiver(&self) -> Option<mpsc::Receiver<Message>> {
        self.message_rx.lock().unwrap().take()
    }
    
    /// Send a message to a specific peer
    pub async fn send(
        &self,
        peer_id: &str,
        message: Message,
    ) -> Result<(), TransportError> {
        let (tx, rx) = oneshot::channel();
        
        self.command_tx
            .send(TransportCommand::Send {
                peer_id: peer_id.to_string(),
                message,
                response: tx,
            })
            .await
            .map_err(|_| {
                TransportError::Internal("Failed to send command".to_string())
            })?;
        
        rx.await.map_err(|_| {
            TransportError::Internal("Failed to receive response".to_string())
        })?
    }
    
    /// Broadcast a message to all peers
    pub async fn broadcast(
        &self,
        message: Message,
    ) -> HashMap<String, Result<(), TransportError>> {
        let (tx, rx) = oneshot::channel();
        
        if let Err(_) = self
            .command_tx
            .send(TransportCommand::Broadcast {
                message,
                response: tx,
            })
            .await
        {
            return HashMap::new();
        }
        
        rx.await.unwrap_or_default()
    }
    
    /// Connect to a new peer
    pub async fn connect(
        &self,
        peer_id: &str,
        addr: SocketAddr,
    ) -> Result<(), TransportError> {
        let (tx, rx) = oneshot::channel();
        
        self.command_tx
            .send(TransportCommand::Connect {
                peer_id: peer_id.to_string(),
                addr,
                response: tx,
            })
            .await
            .map_err(|_| {
                TransportError::Internal("Failed to send command".to_string())
            })?;
        
        rx.await.map_err(|_| {
            TransportError::Internal("Failed to receive response".to_string())
        })?
    }
    
    /// Disconnect from a peer
    pub async fn disconnect(&self, peer_id: &str) -> Result<(), TransportError> {
        let (tx, rx) = oneshot::channel();
        
        self.command_tx
            .send(TransportCommand::Disconnect {
                peer_id: peer_id.to_string(),
                response: tx,
            })
            .await
            .map_err(|_| {
                TransportError::Internal("Failed to send command".to_string())
            })?;
        
        rx.await.map_err(|_| {
            TransportError::Internal("Failed to receive response".to_string())
        })?
    }
    
    /// Shutdown the transport
    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        
        if let Err(_) = self
            .command_tx
            .send(TransportCommand::Shutdown { response: tx })
            .await
        {
            return;
        }
        
        let _ = rx.await;
    }
}

// Listen for incoming connections
async fn listen(
    addr: SocketAddr,
    node_id: String,
    cluster_id: String,
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    tx: mpsc::Sender<Message>,
    config: Arc<NetworkConfig>,
) -> Result<(), TransportError> {
    // Bind to the listen address
    let listener = TcpListener::bind(addr).await?;
    log::info!("Listening on {}", addr);
    
    // Accept connections
    while let Ok((stream, peer_addr)) = listener.accept().await {
        log::debug!("Accepted connection from {}", peer_addr);
        
        // Set TCP options
        if let Err(e) = stream.set_nodelay(true) {
            log::warn!("Failed to set TCP_NODELAY: {}", e);
        }
        
        // Handle the connection in a separate task
        let node_id = node_id.clone();
        let cluster_id = cluster_id.clone();
        let peers = peers.clone();
        let tx = tx.clone();
        let config = config.clone();
        
        tokio::spawn(async move {
            if let Err(e) = handle_connection(
                stream,
                peer_addr,
                node_id,
                cluster_id,
                peers,
                tx,
                config,
            ).await {
                log::error!("Connection error for {}: {}", peer_addr, e);
            }
        });
    }
    
    Ok(())
}

// Handle a single connection
// Handle a single connection
async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    node_id: String,
    cluster_id: String,
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    tx: mpsc::Sender<Message>,
    config: Arc<NetworkConfig>,
) -> Result<(), TransportError> {
    // Split the stream into read and write halves
    let (mut read_half, mut write_half) = tokio::io::split(stream);
    
    // Send handshake
    let handshake = Message::Handshake {
        node_id: node_id.clone(),
        cluster_id: cluster_id.clone(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    // Serialize the handshake message
    let bytes = bincode::serialize(&handshake).map_err(|e| {
        TransportError::Serialization(e.to_string())
    })?;
    
    // Write message length and data
    let len = bytes.len() as u32;
    write_half.write_all(&len.to_be_bytes()).await?;
    write_half.write_all(&bytes).await?;
    write_half.flush().await?;
    
    // Receive handshake
    let peer_handshake = match timeout(Duration::from_secs(5), read_message(&mut read_half)).await {
        Ok(Ok(msg)) => msg,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(TransportError::Timeout),
    };
    
    let peer_id = match peer_handshake {
        Message::Handshake {
            node_id,
            cluster_id: peer_cluster_id,
            timestamp: _,
        } => {
            // Verify cluster ID
            if peer_cluster_id != cluster_id {
                log::warn!(
                    "Peer {} has different cluster ID: {} (expected {})",
                    peer_addr,
                    peer_cluster_id,
                    cluster_id
                );
                return Err(TransportError::Internal(
                    "Cluster ID mismatch".to_string(),
                ));
            }
            
            node_id
        }
        _ => {
            return Err(TransportError::Internal(
                "Invalid handshake message".to_string(),
            ));
        }
    };
    
    log::info!("Established connection with peer {} ({})", peer_id, peer_addr);
    
    // Create message channel for this peer
    let (peer_tx, mut peer_rx) = mpsc::channel::<Message>(100);
    
    // Add peer to the connection map
    let last_activity = Arc::new(Mutex::new(std::time::Instant::now()));
    let peer_conn = PeerConnection {
        id: peer_id.clone(),
        addr: peer_addr,
        tx: peer_tx.clone(),
        last_activity: last_activity.clone(),
    };
    
    peers.write().unwrap().insert(peer_id.clone(), peer_conn);
    
    // Spawn writer task
    let writer_last_activity = last_activity.clone();
    tokio::spawn(async move {
        while let Some(message) = peer_rx.recv().await {
            // Serialize the message
            let bytes = match bincode::serialize(&message) {
                Ok(bytes) => bytes,
                Err(e) => {
                    log::error!("Failed to serialize message: {}", e);
                    continue;
                }
            };
            
            // Write message length and data
            let len = bytes.len() as u32;
            if let Err(e) = write_half.write_all(&len.to_be_bytes()).await {
                log::error!("Failed to write message length: {}", e);
                break;
            }
            
            if let Err(e) = write_half.write_all(&bytes).await {
                log::error!("Failed to write message data: {}", e);
                break;
            }
            
            if let Err(e) = write_half.flush().await {
                log::error!("Failed to flush message: {}", e);
                break;
            }
            
            // Update last activity
            *writer_last_activity.lock().unwrap() = std::time::Instant::now();
        }
    });
    
    // Reader loop
    loop {
        // Read message with timeout
        let message = match timeout(
            Duration::from_secs(config.timeout.as_secs() * 2),
            read_message::<Message, _>(&mut read_half),
        ).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(e)) => {
                log::error!("Error reading message from peer {}: {}", peer_id, e);
                break;
            }
            Err(_) => {
                log::warn!("Connection timeout for peer {}", peer_id);
                break;
            }
        };
        
        // Update last activity
        *last_activity.lock().unwrap() = std::time::Instant::now();
        
        // Handle message
        match &message {
            Message::Heartbeat => {
                // Respond with heartbeat
                if let Err(e) = peer_tx.send(Message::Heartbeat).await {
                    log::error!("Failed to send heartbeat to peer {}: {}", peer_id, e);
                    break;
                }
            }
            _ => {
                // Forward message to handler
                if let Err(e) = tx.send(message).await {
                    log::error!("Failed to forward message: {}", e);
                    break;
                }
            }
        }
    }
    
    // Remove peer from the connection map
    peers.write().unwrap().remove(&peer_id);
    log::info!("Disconnected from peer {} ({})", peer_id, peer_addr);
    
    Ok(())
}

// Process transport commands
async fn process_commands(
    mut command_rx: mpsc::Receiver<TransportCommand>,
    mut _message_rx: mpsc::Receiver<Message>,
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    node_id: String,
    cluster_id: String,
    config: Arc<NetworkConfig>,
) {
    while let Some(command) = command_rx.recv().await {
        match command {
            TransportCommand::Send {
                peer_id,
                message,
                response,
            } => {
                let result = send_to_peer(&peers, &peer_id, message).await;
                let _ = response.send(result);
            }
            TransportCommand::Broadcast { message, response } => {
                let mut results = HashMap::new();
                let peer_ids: Vec<String> = {
                    peers.read().unwrap().keys().cloned().collect()
                };
                
                for peer_id in peer_ids {
                    let result = send_to_peer(&peers, &peer_id, message.clone()).await;
                    results.insert(peer_id, result);
                }
                
                let _ = response.send(results);
            }
            TransportCommand::Connect {
                peer_id,
                addr,
                response,
            } => {
                let result = connect_to_peer(
                    &peers,
                    peer_id.clone(),
                    addr,
                    node_id.clone(),
                    cluster_id.clone(),
                    config.clone(),
                ).await;
                let _ = response.send(result);
            }
            TransportCommand::Disconnect { peer_id, response } => {
                let mut peers_write = peers.write().unwrap();
                peers_write.remove(&peer_id);
                let _ = response.send(Ok(()));
            }
            TransportCommand::Shutdown { response } => {
                // Close all connections
                peers.write().unwrap().clear();
                let _ = response.send(());
                break;
            }
        }
    }
}

// Send a message to a specific peer
async fn send_to_peer(
    peers: &Arc<RwLock<HashMap<String, PeerConnection>>>,
    peer_id: &str,
    message: Message,
) -> Result<(), TransportError> {
    let tx = {
        let peers_read = peers.read().unwrap();
        match peers_read.get(peer_id) {
            Some(peer) => peer.tx.clone(),
            None => return Err(TransportError::NotConnected(peer_id.to_string())),
        }
    };
    
    tx.send(message).await.map_err(|_| {
        TransportError::Closed
    })
}

// Connect to a peer
async fn connect_to_peer(
    peers: &Arc<RwLock<HashMap<String, PeerConnection>>>,
    peer_id: String,
    addr: SocketAddr,
    node_id: String,
    cluster_id: String,
    config: Arc<NetworkConfig>,
) -> Result<(), TransportError> {
    // Check if already connected
    {
        let peers_read = peers.read().unwrap();
        if peers_read.contains_key(&peer_id) {
            return Ok(());
        }
    }
    
    // Connect to peer
    let mut stream = match timeout(
        config.timeout,
        TcpStream::connect(addr),
    ).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(TransportError::Io(e)),
        Err(_) => return Err(TransportError::Timeout),
    };
    
    // Set TCP options
    if let Err(e) = stream.set_nodelay(true) {
        log::warn!("Failed to set TCP_NODELAY: {}", e);
    }
    
    // Send handshake
    let handshake = Message::Handshake {
        node_id: node_id.clone(),
        cluster_id: cluster_id.clone(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    // Serialize and send handshake
    let bytes = bincode::serialize(&handshake).map_err(|e| {
        TransportError::Serialization(e.to_string())
    })?;
    
    
    // Write message length and data
    let len = bytes.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    
    // Receive handshake
    let peer_handshake = match timeout(
        Duration::from_secs(5),
        read_message::<Message, _>(&mut stream),
    ).await {
        Ok(Ok(msg)) => msg,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(TransportError::Timeout),
    };
    
    let verified_peer_id = match peer_handshake {
        Message::Handshake {
            node_id,
            cluster_id: peer_cluster_id,
            timestamp: _,
        } => {
            // Verify cluster ID
            if peer_cluster_id != cluster_id {
                log::warn!(
                    "Peer {} has different cluster ID: {} (expected {})",
                    addr,
                    peer_cluster_id,
                    cluster_id
                );
                return Err(TransportError::Internal(
                    "Cluster ID mismatch".to_string(),
                ));
            }
            
            // Verify node ID
            if node_id != peer_id {
                log::warn!(
                    "Peer {} has different node ID: {} (expected {})",
                    addr,
                    node_id,
                    peer_id
                );
                return Err(TransportError::Internal(
                    "Node ID mismatch".to_string(),
                ));
            }
            
            node_id
        }
        _ => {
            return Err(TransportError::Internal(
                "Invalid handshake message".to_string(),
            ));
        }
    };
    
    log::info!("Connected to peer {} ({})", verified_peer_id, addr);
    
    // Create message channel for this peer
    let (peer_tx, _peer_rx) = mpsc::channel::<Message>(100);
    
    // Add peer to the connection map
    let last_activity = Arc::new(Mutex::new(std::time::Instant::now()));
    let peer_conn = PeerConnection {
        id: verified_peer_id.clone(),
        addr,
        tx: peer_tx,
        last_activity,
    };
    
    peers.write().unwrap().insert(verified_peer_id.clone(), peer_conn);
    
    // Note: We're not starting reader and writer tasks here since that would be
    // handled by the listener when the peer connects back to us
    
    Ok(())
}

// Read a message from a stream
use tokio::io::AsyncRead;

async fn read_message<T, R>(
    reader: &mut R,
) -> Result<T, TransportError>
where
    T: for<'de> Deserialize<'de>,
    R: AsyncRead + Unpin,
{
    // Read message length
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    
    // Read message data
    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer).await?;
    
    // Deserialize the message
    bincode::deserialize(&buffer).map_err(|e| {
        TransportError::Serialization(e.to_string())
    })
}