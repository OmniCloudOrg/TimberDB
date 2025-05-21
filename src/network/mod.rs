// TimberDB: A high-performance distributed log database
// network/mod.rs - Network module for distributed communication

pub mod consensus;
pub mod transport;

use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::config::{NetworkConfig, NodeConfig};
use crate::storage::block::LogEntry;
use self::consensus::{Command, Consensus, ConsensusConfig, ConsensusError, NodeState};
use self::transport::{Message, Transport, TransportError};

// Network errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    
    #[error("Consensus error: {0}")]
    Consensus(#[from] ConsensusError),
    
    #[error("Not the leader")]
    NotLeader,
    
    #[error("Internal error: {0}")]
    Internal(String),
}

// Network service for distributed communication
#[derive(Debug)]
pub struct Network {
    /// Node ID
    node_id: String,
    /// Cluster ID
    cluster_id: String,
    /// Network transport
    transport: Arc<Transport>,
    /// Consensus module
    consensus: Arc<Consensus>,
    /// Message receiver
    message_rx: Arc<Mutex<mpsc::Receiver<Message>>>,
}

impl Network {
    /// Create a new network service
    pub async fn new(
        node_config: NodeConfig,
        network_config: NetworkConfig,
    ) -> Result<Self, NetworkError> {
        let node_id = node_config.id.clone();
        let cluster_id = Uuid::new_v4().to_string();
        
        // Create consensus module
        let consensus_config = ConsensusConfig {
            node: node_config.clone(),
            peers: network_config.peers.clone(),
            ..Default::default()
        };
        
        let consensus = Consensus::new(consensus_config).await
            .map_err(NetworkError::Consensus)?;
        
        // Create transport module
        let transport = Transport::new(node_id.clone(), cluster_id.clone(), network_config.clone())
            .await
            .map_err(NetworkError::Transport)?;

        // Get message receiver (take ownership, not a reference)
        let (network_message_tx, network_message_rx) = mpsc::channel(1000);
        let message_rx = Arc::new(Mutex::new(network_message_rx));
        let message_rx_clone = message_rx.clone();

        // Create network instance
        let network = Network {
            node_id,
            cluster_id,
            transport: Arc::new(transport),
            consensus: Arc::new(consensus),
            message_rx,
        };
        
        // Start message handler
        let transport_clone = network.transport.clone();
        let consensus_clone = network.consensus.clone();

        tokio::spawn(async move {
            let mut rx = message_rx_clone.lock().await;
            while let Some(message) = rx.recv().await {
                handle_message(
                    &transport_clone,
                    &consensus_clone,
                    message,
                ).await;
            }
        });
        
        // Connect to peers
        for (i, addr) in network_config.peers.iter().enumerate() {
            let peer_id = format!("peer-{}", i);
            if let Err(e) = network.transport.connect(&peer_id, *addr).await {
                log::warn!("Failed to connect to peer {}: {}", peer_id, e);
            }
        }
        
        Ok(network)
    }
    
    /// Apply a command to the distributed state machine
    pub async fn apply_command(&self, command: Command) -> Result<(), NetworkError> {
        // Check if we're the leader
        if self.consensus.get_state().await != NodeState::Leader {
            // Forward to leader if known
            if let Some(leader_id) = self.consensus.get_leader().await {
                // TODO: Implement forwarding to leader
                return Err(NetworkError::NotLeader);
            } else {
                return Err(NetworkError::NotLeader);
            }
        }
        
        // Apply command through consensus
        self.consensus.apply(command).await
            .map_err(NetworkError::Consensus)
    }
    
    /// Get the current node state
    pub async fn get_state(&self) -> NodeState {
        self.consensus.get_state().await
    }
    
    /// Get the current leader ID
    pub async fn get_leader(&self) -> Option<String> {
        self.consensus.get_leader().await
    }
    
    /// Shutdown the network service
    pub async fn shutdown(&self) {
        self.consensus.shutdown().await;
        self.transport.shutdown().await;
    }
}

// Handle incoming network messages
async fn handle_message(
    transport: &Arc<Transport>,
    consensus: &Arc<Consensus>,
    message: Message,
) {
    match message {
        Message::Raft(raft_message) => {
            // Handle Raft consensus messages
            // TODO: Process raft messages through consensus module
        }
        Message::Query { query_id, query_string } => {
            // Handle direct queries
            // TODO: Implement query handling
        }
        Message::QueryResponse { query_id, success, results, error } => {
            // Handle query responses
            // TODO: Implement query response handling
        }
        Message::Heartbeat => {
            // Heartbeat messages are handled by the transport layer
        }
        Message::Handshake { node_id, cluster_id, timestamp } => {
            // Handshake messages are handled by the transport layer
        }
    }
}