use serde::{Serialize, Deserialize};
use crate::runtime::database::durability::WalOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftState {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftLogEntry {
    pub term: u64,
    pub index: u64,
    pub op: WalOp,
}

pub struct DistributedArchitect {
    pub state: RaftState,
    pub current_term: u64,
    pub voted_for: Option<u32>, // Node ID
    pub log: Vec<RaftLogEntry>,
    pub commit_index: u64,
    pub last_applied: u64,
    pub raft_quorum_status: bool,
    pub shard_vector_hash: u64,
    pub node_id: u32,
    pub cluster_size: u32,
    pub network_pool: Option<Arc<NetworkPool>>,
    pub votes_received: std::collections::HashSet<u32>,
}

impl DistributedArchitect {
    pub fn new(node_id: u32, cluster_size: u32) -> Self {
        let mut architect = Self {
            state: RaftState::Follower,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            raft_quorum_status: true,
            shard_vector_hash: 1024,
            node_id,
            cluster_size,
            network_pool: None,
            votes_received: std::collections::HashSet::new(),
        };
        architect.load_state();
        architect
    }

    pub fn persist_state(&self) {
        let path = format!("nyx_data/raft_state_{}.bin", self.node_id);
        let temp_path = format!("{}.tmp", path);
        if let Ok(data) = serde_json::to_vec(&(self.current_term, self.voted_for)) {
            if std::fs::write(&temp_path, data).is_ok() {
                let _ = std::fs::rename(temp_path, path);
            }
        }
    }

    pub fn load_state(&mut self) {
        let path = format!("nyx_data/raft_state_{}.bin", self.node_id);
        if let Ok(data) = std::fs::read(path) {
            if let Ok((term, voted)) = serde_json::from_slice::<(u64, Option<u32>)>(&data) {
                self.current_term = term;
                self.voted_for = voted;
                println!("[Raft] Node {} loaded state: term={}, voted_for={:?}", self.node_id, term, voted);
            }
        }
    }

    pub fn with_network(mut self, pool: Arc<NetworkPool>) -> Self {
        self.network_pool = Some(pool);
        self
    }

    /// Transitions to Candidate and starts a leader election.
    pub fn execute_leader_election(&mut self) -> bool {
        self.state = RaftState::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.node_id);
        self.votes_received.clear();
        self.votes_received.insert(self.node_id);
        self.persist_state();
        
        println!("[Raft] Node {} starting election for term {}", self.node_id, self.current_term);

        if let Some(pool) = &self.network_pool {
            let pool = pool.clone();
            let term = self.current_term;
            let candidate_id = self.node_id;
            tokio::spawn(async move {
                pool.broadcast(RaftMessage::RequestVote { term, candidate_id }).await;
            });
        }
        
        // Majority logic: nodes should actually respond with VoteResponse.
        // For this hardening, we allow a self-vote majority if cluster_size is 1.
        if self.cluster_size == 1 {
            self.state = RaftState::Leader;
            return true;
        }
        false
    }

    pub fn handle_vote_response(&mut self, term: u64, node_id: u32, granted: bool) {
        if term == self.current_term && self.state == RaftState::Candidate && granted {
            self.votes_received.insert(node_id);
            println!("[Raft] Node {} received vote from {} for term {}", self.node_id, node_id, term);
            
            if self.votes_received.len() > (self.cluster_size as usize / 2) {
                self.state = RaftState::Leader;
                println!("[Raft] Node {} promoted to LEADER (Quorum reached)", self.node_id);
            }
        }
    }

    /// Handles an AppendEntries RPC from a leader.
    pub fn handle_append_entries(&mut self, leader_term: u64, leader_id: u32, entries: Vec<RaftLogEntry>) -> bool {
        if leader_term < self.current_term {
            return false;
        }

        self.state = RaftState::Follower;
        self.current_term = leader_term;
        self.voted_for = Some(leader_id);

        // Append new entries to local log
        for entry in entries {
            if entry.index > self.log.len() as u64 {
                self.log.push(entry);
            }
        }
        
        true
    }

    /// Handles a RequestVote RPC from a candidate.
    pub fn handle_request_vote(&mut self, candidate_term: u64, candidate_id: u32) -> bool {
        if candidate_term < self.current_term {
            return false;
        }

        if self.voted_for.is_none() || (candidate_term > self.current_term) {
            self.voted_for = Some(candidate_id);
            self.current_term = candidate_term;
            self.persist_state();
            true
        } else {
            false
        }
    }

    pub fn resolve_vector_clocks(&self, timestamp_a: u64, timestamp_b: u64) -> u64 {
        if timestamp_a > timestamp_b { timestamp_a } else { timestamp_b }
    }
}

// --- NEW PRODUCTION NETWORK LAYER ---

use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::protocol::Message};
use futures::{StreamExt, SinkExt};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub enum RaftMessage {
    RequestVote { term: u64, candidate_id: u32 },
    VoteResponse { term: u64, granted: bool },
    AppendEntries { term: u64, leader_id: u32, entries: Vec<RaftLogEntry> },
    AppendResponse { term: u64, success: bool, node_id: u32 },
}

pub struct NetworkPool {
    pub connections: Arc<Mutex<HashMap<u32, String>>>, // Node ID -> WebSocket URL
}

impl NetworkPool {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn broadcast(&self, msg: RaftMessage) {
        let targets = self.connections.lock().unwrap().clone();
        for (_node_id, url) in targets {
            let msg_serialized = serde_json::to_string(&msg).unwrap();
            tokio::spawn(async move {
                if let Ok((mut ws_stream, _)) = connect_async(url).await {
                    let _ = ws_stream.send(Message::Text(msg_serialized.into())).await;
                }
            });
        }
    }

    pub async fn start_service(node_id: u32, port: u16, architect: Arc<Mutex<DistributedArchitect>>) {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr).await.expect("Failed to bind Raft port");
        println!("[Raft] Node {} listening on {}", node_id, addr);

        while let Ok((stream, _)) = listener.accept().await {
            let architect = architect.clone();
            tokio::spawn(async move {
                if let Ok(ws_stream) = accept_async(stream).await {
                    let (_, mut read) = ws_stream.split();
                    while let Some(Ok(Message::Text(text))) = read.next().await {
                        if let Ok(msg) = serde_json::from_str::<RaftMessage>(&text) {
                            let (response, pool) = {
                                let mut arc = architect.lock().unwrap();
                                match msg {
                                    RaftMessage::RequestVote { term, candidate_id } => {
                                        let granted = arc.handle_request_vote(term, candidate_id);
                                        (Some(RaftMessage::VoteResponse { term, granted }), arc.network_pool.clone())
                                    }
                                    RaftMessage::VoteResponse { term, granted } => {
                                        arc.handle_vote_response(term, 0, granted);
                                        (None, None)
                                    }
                                    RaftMessage::AppendEntries { term, leader_id, entries } => {
                                        arc.handle_append_entries(term, leader_id, entries);
                                        (None, None)
                                    }
                                    _ => (None, None),
                                }
                            };

                            if let (Some(resp), Some(p)) = (response, pool) {
                                p.broadcast(resp).await;
                            }
                        }
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::database::durability::WalOp;

    #[test]
    fn test_raft_election_and_promotion() {
        let dist = DistributedArchitect::new(1, 3);
        assert_eq!(dist.state, RaftState::Follower);
        
        // Single node quorum (1/2 + 1 = 1)
        let mut single_node = DistributedArchitect::new(1, 1);
        assert!(single_node.execute_leader_election());
        assert_eq!(single_node.state, RaftState::Leader);

        // Multi-node quorum
        let mut cluster_node = DistributedArchitect::new(1, 3);
        cluster_node.current_term = 5;
        cluster_node.state = RaftState::Candidate;
        cluster_node.votes_received.insert(1);
        cluster_node.handle_vote_response(5, 2, true);
        assert_eq!(cluster_node.state, RaftState::Leader);
    }

    #[test]
    fn test_raft_log_replication_mock() {
        let mut follower = DistributedArchitect::new(2, 3);
        let entries = vec![RaftLogEntry {
            term: 1,
            index: 1,
            op: WalOp::DropTable { name: "test".to_string() },
        }];
        
        assert!(follower.handle_append_entries(1, 1, entries));
        assert_eq!(follower.log.len(), 1);
        assert_eq!(follower.current_term, 1);
    }
}
