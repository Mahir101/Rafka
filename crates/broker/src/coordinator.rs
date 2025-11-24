use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{SystemTime, Duration};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MemberMetadata {
    pub member_id: String,
    pub client_id: String,
    pub client_host: String,
    pub protocol_type: String,
    pub topics: Vec<String>,
    pub last_heartbeat: SystemTime,
    pub assignment: Vec<u8>,
}

#[derive(Debug)]
pub struct GroupMetadata {
    pub group_id: String,
    pub state: GroupState,
    pub members: HashMap<String, MemberMetadata>,
    pub leader_id: Option<String>,
    pub generation_id: i32,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupState {
    Empty,
    PreparingRebalance,
    CompletingRebalance,
    Stable,
    Dead,
}

pub struct GroupCoordinator {
    groups: Arc<RwLock<HashMap<String, Arc<RwLock<GroupMetadata>>>>>,
}

impl GroupCoordinator {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn handle_join_group(
        &self,
        group_id: &str,
        member_id: &str,
        client_id: &str,
        protocol_type: &str,
        topics: Vec<String>,
    ) -> Result<(String, String, i32, String), String> {
        let mut groups = self.groups.write().await;
        
        let group = groups
            .entry(group_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(GroupMetadata {
                group_id: group_id.to_string(),
                state: GroupState::Empty,
                members: HashMap::new(),
                leader_id: None,
                generation_id: 0,
                protocol: None,
            })));

        let mut group = group.write().await;

        // Generate member_id if empty
        let new_member_id = if member_id.is_empty() {
            format!("{}-{}", client_id, Uuid::new_v4())
        } else {
            member_id.to_string()
        };

        // Add or update member
        group.members.insert(new_member_id.clone(), MemberMetadata {
            member_id: new_member_id.clone(),
            client_id: client_id.to_string(),
            client_host: "127.0.0.1".to_string(), // TODO: Get from request
            protocol_type: protocol_type.to_string(),
            topics,
            last_heartbeat: SystemTime::now(),
            assignment: Vec::new(),
        });

        // Elect leader if none
        if group.leader_id.is_none() {
            group.leader_id = Some(new_member_id.clone());
        }

        // Transition to PreparingRebalance if not already
        if group.state == GroupState::Stable || group.state == GroupState::Empty {
            group.state = GroupState::PreparingRebalance;
            group.generation_id += 1;
        }

        Ok((
            new_member_id,
            group.leader_id.clone().unwrap(),
            group.generation_id,
            protocol_type.to_string(),
        ))
    }

    pub async fn handle_sync_group(
        &self,
        group_id: &str,
        member_id: &str,
        generation_id: i32,
        group_assignment: HashMap<String, Vec<u8>>,
    ) -> Result<Vec<u8>, String> {
        let groups = self.groups.read().await;
        if let Some(group) = groups.get(group_id) {
            let mut group = group.write().await;

            if group.generation_id != generation_id {
                return Err("Invalid generation id".to_string());
            }

            if group.state == GroupState::PreparingRebalance {
                // If leader, apply assignments
                if let Some(leader) = &group.leader_id {
                    if leader == member_id {
                        for (member, assignment) in group_assignment {
                            if let Some(m) = group.members.get_mut(&member) {
                                m.assignment = assignment;
                            }
                        }
                        group.state = GroupState::Stable;
                    }
                }
            }

            // Return assignment for this member
            if let Some(member) = group.members.get(member_id) {
                Ok(member.assignment.clone())
            } else {
                Err("Member not found".to_string())
            }
        } else {
            Err("Group not found".to_string())
        }
    }

    pub async fn handle_heartbeat(
        &self,
        group_id: &str,
        member_id: &str,
        generation_id: i32,
    ) -> Result<(), String> {
        let groups = self.groups.read().await;
        if let Some(group) = groups.get(group_id) {
            let mut group = group.write().await;

            if group.generation_id != generation_id {
                return Err("Rebalance in progress".to_string());
            }

            if let Some(member) = group.members.get_mut(member_id) {
                member.last_heartbeat = SystemTime::now();
                Ok(())
            } else {
                Err("Member not found".to_string())
            }
        } else {
            Err("Group not found".to_string())
        }
    }
}
