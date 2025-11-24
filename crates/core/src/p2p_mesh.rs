use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, mpsc, broadcast};
use tokio::time::interval;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::net::SocketAddr;
use tonic::Request;
use crate::proto::rafka::{
    broker_service_client::BrokerServiceClient,
    GetMeshTopologyRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
    
    pub fn from_string(id: String) -> Self {
        Self(id)
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: SocketAddr,
    pub partition_id: u32,
    pub total_partitions: u32,
    pub last_seen: u64, // Unix timestamp instead of Instant
    pub is_alive: bool,
    pub metadata: HashMap<String, String>,
}

impl NodeInfo {
    pub fn new(address: SocketAddr, partition_id: u32, total_partitions: u32) -> Self {
        Self {
            id: NodeId::new(),
            address,
            partition_id,
            total_partitions,
            last_seen: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            is_alive: true,
            metadata: HashMap::new(),
        }
    }
    
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.is_alive = true;
    }
    
    pub fn mark_dead(&mut self) {
        self.is_alive = false;
    }
    
    pub fn is_stale(&self, timeout: Duration) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_seen > timeout.as_secs()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipMessage {
    pub from_node: NodeId,
    pub message_type: GossipMessageType,
    pub timestamp: SystemTime,
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessageType {
    NodeJoin(NodeInfo),
    NodeLeave(NodeId),
    NodeUpdate(NodeInfo),
    Heartbeat(NodeId),
    PartitionUpdate { partition_id: u32, owner: NodeId },
    ClusterState(Vec<NodeInfo>),
}

#[derive(Debug, Clone)]
pub struct MeshTopology {
    pub nodes: HashMap<NodeId, NodeInfo>,
    pub partitions: HashMap<u32, NodeId>,
    pub connections: HashMap<NodeId, HashSet<NodeId>>,
    pub last_updated: u64, // Unix timestamp instead of Instant
}

impl MeshTopology {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            partitions: HashMap::new(),
            connections: HashMap::new(),
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    pub fn add_node(&mut self, node: NodeInfo) {
        self.nodes.insert(node.id.clone(), node.clone());
        self.partitions.insert(node.partition_id, node.id.clone());
        self.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    
    pub fn remove_node(&mut self, node_id: &NodeId) {
        if let Some(node) = self.nodes.remove(node_id) {
            self.partitions.remove(&node.partition_id);
            self.connections.remove(node_id);
            // Remove connections to this node from other nodes
            for connections in self.connections.values_mut() {
                connections.remove(node_id);
            }
            self.last_updated = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }
    
    pub fn update_node(&mut self, node: NodeInfo) {
        if self.nodes.contains_key(&node.id) {
            self.nodes.insert(node.id.clone(), node.clone());
            self.partitions.insert(node.partition_id, node.id.clone());
            self.last_updated = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }
    
    pub fn get_node_for_partition(&self, partition_id: u32) -> Option<&NodeInfo> {
        self.partitions.get(&partition_id)
            .and_then(|node_id| self.nodes.get(node_id))
    }
    
    pub fn get_alive_nodes(&self) -> Vec<&NodeInfo> {
        self.nodes.values()
            .filter(|node| node.is_alive)
            .collect()
    }
    
    pub fn get_neighbors(&self, node_id: &NodeId) -> Vec<&NodeInfo> {
        self.connections.get(node_id)
            .map(|neighbors| {
                neighbors.iter()
                    .filter_map(|id| self.nodes.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub struct P2PMesh {
    pub node_id: NodeId,
    pub node_info: NodeInfo,
    pub topology: Arc<RwLock<MeshTopology>>,
    pub gossip_tx: broadcast::Sender<GossipMessage>,
    pub gossip_rx: broadcast::Receiver<GossipMessage>,
    pub mesh_tx: mpsc::Sender<GossipMessage>,
    pub mesh_rx: mpsc::Receiver<GossipMessage>,
    pub heartbeat_interval: Duration,
    pub gossip_interval: Duration,
    pub node_timeout: Duration,
    pub max_neighbors: usize,
}

impl P2PMesh {
    pub fn new(
        address: SocketAddr,
        partition_id: u32,
        total_partitions: u32,
        heartbeat_interval: Duration,
        gossip_interval: Duration,
        node_timeout: Duration,
        max_neighbors: usize,
    ) -> Self {
        let node_id = NodeId::new();
        let node_info = NodeInfo::new(address, partition_id, total_partitions);
        
        let (gossip_tx, gossip_rx) = broadcast::channel(1000);
        let (mesh_tx, mesh_rx) = mpsc::channel(1000);
        
        Self {
            node_id,
            node_info,
            topology: Arc::new(RwLock::new(MeshTopology::new())),
            gossip_tx,
            gossip_rx,
            mesh_tx,
            mesh_rx,
            heartbeat_interval,
            gossip_interval,
            node_timeout,
            max_neighbors,
        }
    }
    
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🌐 Starting P2P mesh for node {}", self.node_id.0);
        
        // Add self to topology
        {
            let mut topology = self.topology.write().await;
            topology.add_node(self.node_info.clone());
            // Immediately announce our partition ownership
            topology.partitions.insert(self.node_info.partition_id, self.node_id.clone());
        }
        
        // Start background tasks
        let _topology = self.topology.clone();
        let gossip_tx = self.gossip_tx.clone();
        let node_id = self.node_id.clone();
        let heartbeat_interval = self.heartbeat_interval;
        
        // Heartbeat task
        let node_id_heartbeat = node_id.clone();
        tokio::spawn(async move {
            let mut interval = interval(heartbeat_interval);
            loop {
                interval.tick().await;
                
                let heartbeat = GossipMessage {
                    from_node: node_id_heartbeat.clone(),
                    message_type: GossipMessageType::Heartbeat(node_id_heartbeat.clone()),
                    timestamp: SystemTime::now(),
                    ttl: 3,
                };
                
                let _ = gossip_tx.send(heartbeat);
            }
        });
        
        // Topology synchronization task - periodically exchange topology with known nodes
        let topology = self.topology.clone();
        let gossip_interval = self.gossip_interval;
        let node_id_gossip = node_id.clone();

        tokio::spawn(async move {
            let mut interval = interval(gossip_interval);
            loop {
                interval.tick().await;

                // Get current topology
                let known_nodes: Vec<NodeInfo> = {
                    let topology_read = topology.read().await;
                    topology_read.nodes.values().cloned().collect()
                };

                // Send our topology to a few known nodes (simple gossip)
                for node in known_nodes.iter().take(2) {
                    if node.id != node_id_gossip {
                        println!("🔄 Syncing topology to node {} at {}", node.id.0, node.address);
                        // Send topology sync via gRPC
                        let endpoint = format!("http://{}", node.address);
                        if let Ok(mut client) = BrokerServiceClient::connect(endpoint).await {
                            let request = GetMeshTopologyRequest {
                                requesting_node_id: node_id_gossip.0.clone(),
                            };

                            if let Ok(response) = client.get_mesh_topology(Request::new(request)).await {
                                let remote_topology = response.into_inner();
                                println!("📥 Received topology sync from {}: {} nodes, {} partitions",
                                    node.address, remote_topology.nodes.len(), remote_topology.partition_owners.len());

                                // Debug: print what we're receiving
                                for (pid, owner) in &remote_topology.partition_owners {
                                    println!("  📍 Remote partition {} owned by {}", pid, owner);
                                }
                                for node_info in &remote_topology.nodes {
                                    println!("  🖥️ Remote node {} at {}:{}", node_info.node_id, node_info.address, node_info.port);
                                }

                                // Update our topology with remote information
                                let mut topology_write = topology.write().await;

                                // Add partition ownerships from remote node
                                for (partition_id, owner_id) in remote_topology.partition_owners {
                                    let was_updated = !topology_write.partitions.contains_key(&(partition_id as u32));
                                    let owner_id_clone = owner_id.clone();
                                    topology_write.partitions.insert(partition_id as u32, NodeId::from_string(owner_id));
                                    if was_updated {
                                        println!("📍 Updated partition {} ownership to {}", partition_id, owner_id_clone);
                                    }
                                }

                                // Add ALL nodes from remote topology (including ourselves - this helps with address resolution)
                                for remote_node in remote_topology.nodes {
                                    // Skip if we already know about this node
                                    if !topology_write.nodes.contains_key(&NodeId::from_string(remote_node.node_id.clone())) {
                                        let node_info = NodeInfo {
                                            id: NodeId::from_string(remote_node.node_id),
                                            address: format!("{}:{}", remote_node.address, remote_node.port).parse().unwrap_or_else(|_| "127.0.0.1:50051".parse().unwrap()),
                                            partition_id: remote_node.partition_id as u32,
                                            total_partitions: remote_node.total_partitions as u32,
                                            last_seen: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                                            is_alive: remote_node.is_alive,
                                            metadata: remote_node.metadata,
                                        };
                                        let node_id = node_info.id.0.clone();
                                        let node_addr = node_info.address;
                                        let partition_id = node_info.partition_id;
                                        topology_write.add_node(node_info);
                                        println!("🆕 Discovered new node: {} at {} (partition {})",
                                            node_id, node_addr, partition_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        
        // Partition announcement task
        let gossip_tx = self.gossip_tx.clone();
        let node_id_partition = node_id.clone();
        let partition_id = self.node_info.partition_id;
        let partition_interval = Duration::from_secs(15); // Announce every 15 seconds
        
        tokio::spawn(async move {
            let mut interval = interval(partition_interval);
            loop {
                interval.tick().await;
                
                let partition_message = GossipMessage {
                    from_node: node_id_partition.clone(),
                    message_type: GossipMessageType::PartitionUpdate {
                        partition_id,
                        owner: node_id_partition.clone(),
                    },
                    timestamp: SystemTime::now(),
                    ttl: 3,
                };
                
                let _ = gossip_tx.send(partition_message);
            }
        });
        
        // Node cleanup task
        let topology = self.topology.clone();
        let node_timeout = self.node_timeout;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                
                let mut topology = topology.write().await;
                let stale_nodes: Vec<NodeId> = topology.nodes.values()
                    .filter(|node| node.is_stale(node_timeout))
                    .map(|node| node.id.clone())
                    .collect();
                
                for node_id in stale_nodes {
                    println!("🗑️ Removing stale node: {}", node_id.0);
                    topology.remove_node(&node_id);
                }
            }
        });
        
        // Simple gossip forwarding task - forward basic messages
        let topology_for_gossip = self.topology.clone();
        let node_id_for_forward = self.node_id.clone();
        let gossip_rx = self.gossip_tx.subscribe();

        tokio::spawn(async move {
            let mut gossip_recv = gossip_rx;

            while let Ok(message) = gossip_recv.recv().await {
                // Skip if message is from us
                if message.from_node == node_id_for_forward {
                    continue;
                }

                // Skip if TTL expired
                if message.ttl == 0 {
                    continue;
                }

                // For now, just forward to all known nodes (simple flooding)
                // In a real implementation, we'd use more sophisticated routing
                let topology_read = topology_for_gossip.read().await;
                let all_nodes: Vec<_> = topology_read.nodes.values().collect();

                for neighbor in all_nodes.iter().take(2) { // Limit to prevent excessive forwarding
                    if neighbor.id == message.from_node || neighbor.id == node_id_for_forward {
                        continue;
                    }

                    let endpoint = format!("http://{}", neighbor.address);
                    if let Ok(mut client) = BrokerServiceClient::connect(endpoint).await {
                        // For complex messages, just trigger topology sync instead
                        let request = GetMeshTopologyRequest {
                            requesting_node_id: node_id_for_forward.0.clone(),
                        };
                        let _ = client.get_mesh_topology(Request::new(request)).await;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    pub async fn join_cluster(&mut self, bootstrap_nodes: Vec<SocketAddr>) -> Result<(), Box<dyn std::error::Error>> {
        println!("🤝 Joining cluster with bootstrap nodes: {:?}", bootstrap_nodes);
        
        // Send join message to bootstrap nodes
        let join_message = GossipMessage {
            from_node: self.node_id.clone(),
            message_type: GossipMessageType::NodeJoin(self.node_info.clone()),
            timestamp: SystemTime::now(),
            ttl: 5,
        };
        
        // Send join message via gossip
        let _ = self.gossip_tx.send(join_message);
        
        // Send partition ownership announcement
        let partition_message = GossipMessage {
            from_node: self.node_id.clone(),
            message_type: GossipMessageType::PartitionUpdate {
                partition_id: self.node_info.partition_id,
                owner: self.node_id.clone(),
            },
            timestamp: SystemTime::now(),
            ttl: 5,
        };
        
        let _ = self.gossip_tx.send(partition_message);
        
        // Request cluster state from bootstrap nodes
        for bootstrap_addr in bootstrap_nodes {
            self.request_cluster_state(bootstrap_addr).await?;
        }
        
        Ok(())
    }
    
    async fn request_cluster_state(&self, bootstrap_addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        // Create gRPC client to request cluster state
        let endpoint = format!("http://{}", bootstrap_addr);
        let mut client = match BrokerServiceClient::connect(endpoint).await {
            Ok(client) => client,
            Err(e) => {
                println!("Failed to connect to bootstrap node {}: {}", bootstrap_addr, e);
                return Ok(()); // Don't fail the entire join process
            }
        };
        
        let request = GetMeshTopologyRequest {
            requesting_node_id: self.node_id.0.clone(),
        };
        
        match client.get_mesh_topology(Request::new(request)).await {
            Ok(response) => {
                let topology = response.into_inner();
                println!("📊 Received cluster state from {}: {} nodes", bootstrap_addr, topology.cluster_size);

                // Process the received topology
                let mut local_topology = self.topology.write().await;

                // Add nodes from bootstrap node
                for remote_node in topology.nodes {
                    if !local_topology.nodes.contains_key(&NodeId::from_string(remote_node.node_id.clone())) {
                        let node_info = NodeInfo {
                            id: NodeId::from_string(remote_node.node_id.clone()),
                            address: format!("{}:{}", remote_node.address, remote_node.port).parse().unwrap_or_else(|_| bootstrap_addr),
                            partition_id: remote_node.partition_id as u32,
                            total_partitions: remote_node.total_partitions as u32,
                            last_seen: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            is_alive: remote_node.is_alive,
                            metadata: remote_node.metadata,
                        };
                        let node_addr = node_info.address;
                        local_topology.add_node(node_info);
                        println!("🆕 Bootstrapped node: {} at {} (partition {})",
                            remote_node.node_id, node_addr, remote_node.partition_id);
                    }
                }

                for (partition_id, owner_id) in topology.partition_owners {
                    println!("📍 Learned partition {} is owned by {}", partition_id, owner_id);
                    local_topology.partitions.insert(partition_id as u32, NodeId::from_string(owner_id));
                }
            }
            Err(e) => {
                println!("Failed to get cluster state from {}: {}", bootstrap_addr, e);
            }
        }
        
        Ok(())
    }
    
    pub async fn leave_cluster(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("👋 Leaving cluster: {}", self.node_id.0);
        
        let leave_message = GossipMessage {
            from_node: self.node_id.clone(),
            message_type: GossipMessageType::NodeLeave(self.node_id.clone()),
            timestamp: SystemTime::now(),
            ttl: 5,
        };
        
        let _ = self.gossip_tx.send(leave_message);
        
        // Remove self from topology
        {
            let mut topology = self.topology.write().await;
            topology.remove_node(&self.node_id);
        }
        
        Ok(())
    }
    
    pub async fn get_cluster_topology(&self) -> MeshTopology {
        self.topology.read().await.clone()
    }
    
    pub async fn get_node_for_partition(&self, partition_id: u32) -> Option<NodeInfo> {
        let topology = self.topology.read().await;
        topology.get_node_for_partition(partition_id).cloned()
    }
    
    pub async fn broadcast_message(&self, message: GossipMessage) -> Result<(), Box<dyn std::error::Error>> {
        let _ = self.gossip_tx.send(message);
        Ok(())
    }
    
    pub async fn process_gossip_message(&mut self, message: GossipMessage) -> Result<(), Box<dyn std::error::Error>> {
        if message.from_node == self.node_id {
            return Ok(()); // Ignore own messages
        }
        
        if message.ttl == 0 {
            return Ok(()); // Message expired
        }
        
        let mut topology = self.topology.write().await;
        
        match message.message_type {
            GossipMessageType::NodeJoin(ref node_info) => {
                println!("🆕 Node joined: {} at {} (partition {})", 
                    node_info.id.0, node_info.address, node_info.partition_id);
                topology.add_node(node_info.clone());
                // Announce our own partition ownership to the new node
                self.announce_partition_ownership().await;
            }
            GossipMessageType::NodeLeave(ref node_id) => {
                println!("👋 Node left: {}", node_id.0);
                topology.remove_node(node_id);
            }
            GossipMessageType::NodeUpdate(ref node_info) => {
                println!("🔄 Node updated: {} (partition {})", 
                    node_info.id.0, node_info.partition_id);
                topology.update_node(node_info.clone());
            }
            GossipMessageType::Heartbeat(ref node_id) => {
                if let Some(node) = topology.nodes.get_mut(node_id) {
                    node.update_last_seen();
                }
            }
            GossipMessageType::PartitionUpdate { partition_id, ref owner } => {
                println!("📍 Partition {} owned by {}", partition_id, owner.0);
                topology.partitions.insert(partition_id, owner.clone());
            }
            GossipMessageType::ClusterState(ref nodes) => {
                println!("🌐 Received cluster state with {} nodes", nodes.len());
                for node in nodes {
                    if node.id != self.node_id {
                        topology.add_node(node.clone());
                    }
                }
            }
        }
        
        // Forward message to neighbors (with reduced TTL)
        let mut forward_message = message;
        forward_message.ttl -= 1;
        forward_message.from_node = self.node_id.clone();
        
        if forward_message.ttl > 0 {
            let _ = self.gossip_tx.send(forward_message);
        }
        
        Ok(())
    }
    
    async fn announce_partition_ownership(&self) {
        let partition_message = GossipMessage {
            from_node: self.node_id.clone(),
            message_type: GossipMessageType::PartitionUpdate {
                partition_id: self.node_info.partition_id,
                owner: self.node_id.clone(),
            },
            timestamp: SystemTime::now(),
            ttl: 3,
        };
        
        let _ = self.gossip_tx.send(partition_message);
    }
    
    pub async fn get_healthy_nodes(&self) -> Vec<NodeInfo> {
        let topology = self.topology.read().await;
        topology.get_alive_nodes().into_iter().cloned().collect()
    }
    
    pub async fn get_partition_owners(&self) -> HashMap<u32, NodeInfo> {
        let topology = self.topology.read().await;
        topology.partitions.iter()
            .filter_map(|(partition_id, node_id)| {
                topology.nodes.get(node_id).map(|node| (*partition_id, node.clone()))
            })
            .collect()
    }
    
    pub async fn print_topology(&self) {
        let topology = self.topology.read().await;
        println!("🌐 Current mesh topology:");
        println!("   Nodes: {}", topology.nodes.len());
        for (node_id, node) in &topology.nodes {
            println!("     - {} at {} (partition {}, alive: {})", 
                node_id.0, node.address, node.partition_id, node.is_alive);
        }
        println!("   Partitions: {}", topology.partitions.len());
        for (partition_id, owner_id) in &topology.partitions {
            println!("     - Partition {} owned by {}", partition_id, owner_id.0);
        }
    }
}
