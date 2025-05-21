// TimberDB: A high-performance distributed log database
// network/consensus.rs - Raft consensus implementation

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio::time;
use uuid::Uuid;

use crate::config::NodeConfig;
use crate::storage::block::LogEntry;

// Consensus errors
#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Not the leader")]
    NotLeader,
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
}

// Node state in Raft
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// Follower node, receives updates from leader
    Follower,
    /// Candidate node, currently running for election
    Candidate,
    /// Leader node, coordinates the cluster
    Leader,
    /// Observer node, doesn't participate in consensus
    Observer,
}

// Log replication command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Append a log entry to a partition
    Append {
        partition_id: String,
        entry: LogEntry,
    },
    /// Create a new partition
    CreatePartition {
        name: String,
    },
    /// No-op command for leader election
    Noop,
}

// Raft RPC message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    /// Request vote from other nodes
    RequestVote {
        term: u64,
        candidate_id: String,
        last_log_index: u64,
        last_log_term: u64,
    },
    /// Response to vote request
    RequestVoteResponse {
        term: u64,
        vote_granted: bool,
    },
    /// Append entries to follower logs
    AppendEntries {
        term: u64,
        leader_id: String,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    },
    /// Response to append entries
    AppendEntriesResponse {
        term: u64,
        success: bool,
        match_index: u64,
    },
    /// Install snapshot on follower
    InstallSnapshot {
        term: u64,
        leader_id: String,
        last_included_index: u64,
        last_included_term: u64,
        data: Vec<u8>,
        done: bool,
    },
    /// Response to install snapshot
    InstallSnapshotResponse {
        term: u64,
        success: bool,
    },
}

// Log entry in the Raft log
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RaftLogEntry {
    /// Term when entry was received by leader
    pub term: u64,
    /// Command to execute
    pub command: Command,
    /// Unique entry ID
    pub id: String,
}

// Configuration for the consensus module
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Node configuration
    pub node: NodeConfig,
    /// List of peer addresses
    pub peers: Vec<SocketAddr>,
    /// Election timeout range (min, max) in milliseconds
    pub election_timeout: (u64, u64),
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval: u64,
    /// Maximum log entries per append
    pub max_entries_per_append: usize,
    /// Snapshot interval in log entries
    pub snapshot_interval: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        ConsensusConfig {
            node: NodeConfig {
                id: Uuid::new_v4().to_string(),
                role: crate::config::NodeRole::Follower,
            },
            peers: Vec::new(),
            election_timeout: (150, 300),
            heartbeat_interval: 50,
            max_entries_per_append: 100,
            snapshot_interval: 1000,
        }
    }
}

// Consensus module command types
#[derive(Debug)]
pub enum ConsensusCommand {
    /// Apply a command to the state machine
    Apply {
        command: Command,
        response: oneshot::Sender<Result<(), ConsensusError>>,
    },
    /// Get current leader
    GetLeader {
        response: oneshot::Sender<Option<String>>,
    },
    /// Get current term
    GetTerm {
        response: oneshot::Sender<u64>,
    },
    /// Get current state
    GetState {
        response: oneshot::Sender<NodeState>,
    },
    /// Shutdown the consensus module
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

// Consensus implementation
#[derive(Debug)]
pub struct Consensus {
    /// Node ID
    node_id: String,
    /// Current term
    current_term: Arc<RwLock<u64>>,
    /// Current state
    state: Arc<RwLock<NodeState>>,
    /// Current leader ID
    leader_id: Arc<RwLock<Option<String>>>,
    /// Command channel
    command_tx: mpsc::Sender<ConsensusCommand>,
}

impl Consensus {
    /// Create a new consensus module
    pub async fn new(config: ConsensusConfig) -> Result<Self, ConsensusError> {
        let node_id = config.node.id.clone();
        let current_term = Arc::new(RwLock::new(0));
        let state = Arc::new(RwLock::new(match config.node.role {
            crate::config::NodeRole::Leader => NodeState::Leader,
            crate::config::NodeRole::Follower => NodeState::Follower,
            crate::config::NodeRole::Observer => NodeState::Observer,
        }));
        let leader_id = Arc::new(RwLock::new(None));
        
        // Create command channel
        let (command_tx, command_rx) = mpsc::channel(100);
        
        // Create consensus instance
        let consensus = Consensus {
            node_id: node_id.clone(),
            current_term: current_term.clone(),
            state: state.clone(),
            leader_id: leader_id.clone(),
            command_tx: command_tx.clone(),
        };
        
        // Start consensus loop in the background
        tokio::spawn(consensus_loop(
            config,
            node_id,
            current_term.clone(),
            state.clone(),
            leader_id.clone(),
            command_rx,
        ));
        
        Ok(consensus)
    }
    
    /// Apply a command to the state machine
    pub async fn apply(&self, command: Command) -> Result<(), ConsensusError> {
        let (tx, rx) = oneshot::channel();
        
        self.command_tx
            .send(ConsensusCommand::Apply {
                command,
                response: tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::Internal("Failed to send command".to_string())
            })?;
        
        rx.await.map_err(|_| {
            ConsensusError::Internal("Failed to receive response".to_string())
        })?
    }
    
    /// Get current leader
    pub async fn get_leader(&self) -> Option<String> {
        let (tx, rx) = oneshot::channel();
        
        if let Err(_) = self
            .command_tx
            .send(ConsensusCommand::GetLeader { response: tx })
            .await
        {
            return None;
        }
        
        rx.await.unwrap_or(None)
    }
    
    /// Get current term
    pub async fn get_term(&self) -> u64 {
        let (tx, rx) = oneshot::channel();
        
        if let Err(_) = self
            .command_tx
            .send(ConsensusCommand::GetTerm { response: tx })
            .await
        {
            return 0;
        }
        
        rx.await.unwrap_or(0)
    }
    
    /// Get current state
    pub async fn get_state(&self) -> NodeState {
        let (tx, rx) = oneshot::channel();
        
        if let Err(_) = self
            .command_tx
            .send(ConsensusCommand::GetState { response: tx })
            .await
        {
            return NodeState::Follower;
        }
        
        rx.await.unwrap_or(NodeState::Follower)
    }
    
    /// Shutdown the consensus module
    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        
        if let Err(_) = self
            .command_tx
            .send(ConsensusCommand::Shutdown { response: tx })
            .await
        {
            return;
        }
        
        let _ = rx.await;
    }
}

// Main consensus loop
async fn consensus_loop(
    config: ConsensusConfig,
    node_id: String,
    current_term: Arc<RwLock<u64>>,
    state: Arc<RwLock<NodeState>>,
    leader_id: Arc<RwLock<Option<String>>>,
    mut command_rx: mpsc::Receiver<ConsensusCommand>,
) {
    // Initialize Raft state
    let mut voted_for: Option<String> = None;
    let mut log: Vec<RaftLogEntry> = Vec::new();
    let mut commit_index: u64 = 0;
    let mut last_applied: u64 = 0;
    let mut next_index: HashMap<String, u64> = HashMap::new();
    let mut match_index: HashMap<String, u64> = HashMap::new();
    
    // Initialize peer connections
    let mut peers: HashMap<String, SocketAddr> = HashMap::new();
    for (i, addr) in config.peers.iter().enumerate() {
        peers.insert(format!("peer-{}", i), *addr);
    }
    
    // Set initial election timeout
    let mut election_timeout = random_election_timeout(config.election_timeout);
    let mut last_heartbeat = Instant::now();
    
    // Main loop
    loop {
        // Handle commands
        while let Ok(command) = command_rx.try_recv() {
            match command {
                ConsensusCommand::Apply { command, response } => {
                    let result = if *state.read().unwrap() == NodeState::Leader {
                        // Leader can apply commands directly
                        let entry = RaftLogEntry {
                            term: *current_term.read().unwrap(),
                            command,
                            id: Uuid::new_v4().to_string(),
                        };
                        
                        log.push(entry);
                        
                        // TODO: Replicate to followers and wait for majority
                        
                        Ok(())
                    } else {
                        // Non-leaders redirect to leader
                        Err(ConsensusError::NotLeader)
                    };
                    
                    let _ = response.send(result);
                }
                ConsensusCommand::GetLeader { response } => {
                    let _ = response.send(leader_id.read().unwrap().clone());
                }
                ConsensusCommand::GetTerm { response } => {
                    let _ = response.send(*current_term.read().unwrap());
                }
                ConsensusCommand::GetState { response } => {
                    let _ = response.send(*state.read().unwrap());
                }
                ConsensusCommand::Shutdown { response } => {
                    let _ = response.send(());
                    return;
                }
            }
        }
        
        // State machine based on Raft
        match *state.read().unwrap() {
            NodeState::Follower => {
                // Check if election timeout has elapsed
                if last_heartbeat.elapsed() > election_timeout {
                    // Convert to candidate and start election
                    *state.write().unwrap() = NodeState::Candidate;
                    *current_term.write().unwrap() += 1;
                    voted_for = Some(node_id.clone());
                    
                    // Reset election timeout
                    election_timeout = random_election_timeout(config.election_timeout);
                    last_heartbeat = Instant::now();
                    
                    // TODO: Send RequestVote RPCs to all peers
                }
            }
            NodeState::Candidate => {
                // Check if election timeout has elapsed
                if last_heartbeat.elapsed() > election_timeout {
                    // Start new election round
                    *current_term.write().unwrap() += 1;
                    voted_for = Some(node_id.clone());
                    
                    // Reset election timeout
                    election_timeout = random_election_timeout(config.election_timeout);
                    last_heartbeat = Instant::now();
                    
                    // TODO: Send RequestVote RPCs to all peers
                }
            }
            NodeState::Leader => {
                // Send heartbeats to followers
                if last_heartbeat.elapsed() > Duration::from_millis(config.heartbeat_interval) {
                    // Reset heartbeat timer
                    last_heartbeat = Instant::now();
                    
                    // TODO: Send AppendEntries RPCs to all peers
                }
            }
            NodeState::Observer => {
                // Observers don't participate in leader election
                // Just check for heartbeats to track leader
                if last_heartbeat.elapsed() > Duration::from_millis(config.heartbeat_interval * 2) {
                    // Haven't heard from leader, clear leader ID
                    *leader_id.write().unwrap() = None;
                }
            }
        }
        
        // Sleep briefly to avoid busy-waiting
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// Generate a random election timeout
fn random_election_timeout(range: (u64, u64)) -> Duration {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let timeout = rng.gen_range(range.0..=range.1);
    Duration::from_millis(timeout)
}