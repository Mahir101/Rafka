use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::SystemTime;

/// Replica state for a partition
#[derive(Debug, Clone, PartialEq)]
pub enum ReplicaState {
    Leader,
    Follower,
    OutOfSync,
}

/// Metadata for a replica
#[derive(Debug, Clone)]
pub struct ReplicaInfo {
    pub broker_id: String,
    pub state: ReplicaState,
    pub last_fetch_offset: i64,
    pub last_updated: SystemTime,
    pub lag: i64, // How far behind the leader
}

/// In-Sync Replica (ISR) set for a partition
#[derive(Debug)]
pub struct ISRSet {
    pub partition_id: i32,
    pub leader_id: String,
    pub replicas: HashMap<String, ReplicaInfo>,
    pub min_isr: usize, // Minimum number of in-sync replicas required
    pub max_lag_ms: u64, // Maximum lag in milliseconds to be considered in-sync
}

impl ISRSet {
    pub fn new(partition_id: i32, leader_id: String, min_isr: usize) -> Self {
        let mut replicas = HashMap::new();
        replicas.insert(leader_id.clone(), ReplicaInfo {
            broker_id: leader_id.clone(),
            state: ReplicaState::Leader,
            last_fetch_offset: 0,
            last_updated: SystemTime::now(),
            lag: 0,
        });

        Self {
            partition_id,
            leader_id,
            replicas,
            min_isr,
            max_lag_ms: 10000, // 10 seconds default
        }
    }

    /// Add a follower replica
    pub fn add_follower(&mut self, broker_id: String) {
        self.replicas.insert(broker_id.clone(), ReplicaInfo {
            broker_id,
            state: ReplicaState::Follower,
            last_fetch_offset: 0,
            last_updated: SystemTime::now(),
            lag: 0,
        });
    }

    /// Update follower's fetch offset
    pub fn update_follower_offset(&mut self, broker_id: &str, offset: i64, leader_offset: i64) {
        if let Some(replica) = self.replicas.get_mut(broker_id) {
            replica.last_fetch_offset = offset;
            replica.last_updated = SystemTime::now();
            replica.lag = leader_offset - offset;

            // Update state based on lag
            if replica.lag <= 100 { // Within 100 messages
                replica.state = ReplicaState::Follower;
            } else {
                replica.state = ReplicaState::OutOfSync;
            }
        }
    }

    /// Get list of in-sync replicas (including leader)
    pub fn get_isr(&self) -> Vec<String> {
        self.replicas
            .iter()
            .filter(|(_, info)| {
                info.state == ReplicaState::Leader || info.state == ReplicaState::Follower
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check if we have minimum ISR
    pub fn has_min_isr(&self) -> bool {
        self.get_isr().len() >= self.min_isr
    }

    /// Remove a replica
    pub fn remove_replica(&mut self, broker_id: &str) {
        self.replicas.remove(broker_id);
    }
}

/// Replication Manager handles replication across brokers
pub struct ReplicationManager {
    /// ISR sets for each partition
    isr_sets: Arc<RwLock<HashMap<i32, ISRSet>>>,
    /// Replication factor (how many copies of each partition)
    replication_factor: usize,
}

impl ReplicationManager {
    pub fn new(replication_factor: usize) -> Self {
        Self {
            isr_sets: Arc::new(RwLock::new(HashMap::new())),
            replication_factor,
        }
    }

    /// Initialize ISR for a partition
    pub async fn init_partition(&self, partition_id: i32, leader_id: String) {
        let mut isr_sets = self.isr_sets.write().await;
        isr_sets.insert(
            partition_id,
            ISRSet::new(partition_id, leader_id, (self.replication_factor / 2) + 1),
        );
    }

    /// Add a follower to a partition
    pub async fn add_follower(&self, partition_id: i32, broker_id: String) -> Result<(), String> {
        let mut isr_sets = self.isr_sets.write().await;
        if let Some(isr) = isr_sets.get_mut(&partition_id) {
            isr.add_follower(broker_id);
            Ok(())
        } else {
            Err(format!("Partition {} not found", partition_id))
        }
    }

    /// Update follower offset
    pub async fn update_follower_offset(
        &self,
        partition_id: i32,
        broker_id: &str,
        offset: i64,
        leader_offset: i64,
    ) -> Result<(), String> {
        let mut isr_sets = self.isr_sets.write().await;
        if let Some(isr) = isr_sets.get_mut(&partition_id) {
            isr.update_follower_offset(broker_id, offset, leader_offset);
            Ok(())
        } else {
            Err(format!("Partition {} not found", partition_id))
        }
    }

    /// Get ISR for a partition
    pub async fn get_isr(&self, partition_id: i32) -> Result<Vec<String>, String> {
        let isr_sets = self.isr_sets.read().await;
        if let Some(isr) = isr_sets.get(&partition_id) {
            Ok(isr.get_isr())
        } else {
            Err(format!("Partition {} not found", partition_id))
        }
    }

    /// Check if partition has minimum ISR
    pub async fn has_min_isr(&self, partition_id: i32) -> bool {
        let isr_sets = self.isr_sets.read().await;
        isr_sets
            .get(&partition_id)
            .map(|isr| isr.has_min_isr())
            .unwrap_or(false)
    }

    /// Get leader for a partition
    pub async fn get_leader(&self, partition_id: i32) -> Option<String> {
        let isr_sets = self.isr_sets.read().await;
        isr_sets.get(&partition_id).map(|isr| isr.leader_id.clone())
    }

    /// Elect new leader for a partition (simple: pick first ISR member)
    pub async fn elect_leader(&self, partition_id: i32) -> Result<String, String> {
        let mut isr_sets = self.isr_sets.write().await;
        if let Some(isr) = isr_sets.get_mut(&partition_id) {
            let isr_members = isr.get_isr();
            if isr_members.is_empty() {
                return Err("No in-sync replicas available".to_string());
            }

            // Pick first ISR member as new leader
            let new_leader = isr_members[0].clone();
            
            // Update replica states
            for (broker_id, replica) in isr.replicas.iter_mut() {
                if broker_id == &new_leader {
                    replica.state = ReplicaState::Leader;
                } else if replica.state == ReplicaState::Leader {
                    replica.state = ReplicaState::Follower;
                }
            }
            
            isr.leader_id = new_leader.clone();
            Ok(new_leader)
        } else {
            Err(format!("Partition {} not found", partition_id))
        }
    }
}
