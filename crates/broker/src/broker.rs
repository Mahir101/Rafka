use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tonic::{transport::Server, Request, Response, Status};
use futures::Stream;
use std::pin::Pin;
use tokio_stream::wrappers::BroadcastStream;
use rafka_core::proto::rafka::{
    broker_service_server::{BrokerService, BrokerServiceServer},
    RegisterRequest, RegisterResponse, SubscribeRequest, SubscribeResponse,
    PublishRequest, PublishResponse, ConsumeRequest, ConsumeResponse,
    AcknowledgeRequest, AcknowledgeResponse, UpdateOffsetRequest, UpdateOffsetResponse,
    GetMetricsRequest, GetMetricsResponse,
    // New broker-to-broker communication messages
    ForwardMessageRequest, ForwardMessageResponse, GetClusterInfoRequest, GetClusterInfoResponse,
    JoinClusterRequest, JoinClusterResponse, HealthCheckRequest, HealthCheckResponse,
    BrokerInfo as ProtoBrokerInfo,
    // P2P Mesh networking messages
    GossipMessageRequest, GossipMessageResponse, GetMeshTopologyRequest, GetMeshTopologyResponse,
    JoinMeshRequest, JoinMeshResponse, LeaveMeshRequest, LeaveMeshResponse,
    MeshNodeInfo,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use rafka_storage::db::{Storage, RetentionPolicy, StorageMetrics};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use std::time::Duration;
use crate::batching::BatchedProcessor;
use rafka_core::zero_copy::ZeroCopyProcessor;
use rafka_core::memory_pool::{MessagePool, OptimizedMessage};
use rafka_core::cluster::{ClusterManager, BrokerInfo};
use rafka_core::p2p_mesh::{P2PMesh, NodeId, NodeInfo, GossipMessage, GossipMessageType as MeshGossipMessageType};
use serde_yaml;
use std::fs;
use std::net::{SocketAddr, IpAddr};
use std::str::FromStr;

pub struct Broker {
    topics: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    messages: Arc<RwLock<HashMap<u32, broadcast::Sender<ConsumeResponse>>>>,
    message_counter: AtomicUsize,
    broadcast_capacity: usize,
    partition_id: u32,
    total_partitions: u32,
    storage: Arc<Storage>,
    consumer_offsets: Arc<RwLock<HashMap<(String, String), i64>>>,
    batcher: Arc<BatchedProcessor>,
    zero_copy_processor: Arc<ZeroCopyProcessor>,
    message_pool: Arc<MessagePool<OptimizedMessage>>,
    cluster_manager: Arc<ClusterManager>,
    p2p_mesh: Arc<RwLock<Option<P2PMesh>>>,
    broker_id: String,
    address: String,
    port: u16,
}

impl Broker {
    pub fn new(partition_id: u32, total_partitions: u32, retention_policy: Option<RetentionPolicy>) -> Self {
        Self::new_with_cluster(partition_id, total_partitions, retention_policy, "127.0.0.1", 50051)
    }

    pub fn new_with_cluster(
        partition_id: u32, 
        total_partitions: u32, 
        retention_policy: Option<RetentionPolicy>,
        address: &str,
        port: u16,
    ) -> Self {
        // Optimized buffer size for better performance
        const BROADCAST_CAPACITY: usize = 1024 * 64; // Increased from 16KB to 64KB
        
        let storage = Arc::new(Storage::with_retention_policy(
            retention_policy.unwrap_or_default()
        ));
        
        // Initialize message batcher for performance optimization
        let batcher = Arc::new(BatchedProcessor::new(100, Duration::from_millis(10)));
        
        // Initialize zero-copy processor for efficient message handling
        let zero_copy_processor = Arc::new(ZeroCopyProcessor::new(1000));
        
        // Initialize memory pool for optimized message objects
        let message_pool = Arc::new(MessagePool::new(1000)); // Pool of 1000 message objects
        
        // Initialize cluster manager
        let broker_id = format!("broker-{}", partition_id);
        let cluster_manager = Arc::new(ClusterManager::new("rafka-cluster".to_string(), 5000));
        
        Self {
            topics: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            message_counter: AtomicUsize::new(0),
            broadcast_capacity: BROADCAST_CAPACITY,
            partition_id,
            total_partitions,
            storage,
            consumer_offsets: Arc::new(RwLock::new(HashMap::new())),
            batcher,
            zero_copy_processor,
            message_pool,
            cluster_manager,
            p2p_mesh: Arc::new(RwLock::new(None)),
            broker_id,
            address: address.to_string(),
            port,
        }
    }

    pub async fn load_cluster_config(&self, config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string(config_path)?;
        let cluster_config: rafka_core::cluster::ClusterConfig = serde_yaml::from_str(&config_content)?;
        
        let broker_count = cluster_config.brokers.len();
        
        // Add all brokers from config to cluster manager
        for broker_config in cluster_config.brokers {
            let broker_info = BrokerInfo::new(broker_config);
            self.cluster_manager.add_broker(broker_info).await;
        }
        
        println!("Loaded cluster configuration with {} brokers", broker_count);
        Ok(())
    }

    pub async fn initialize_p2p_mesh(&self, bootstrap_nodes: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        println!("🌐 Initializing P2P mesh networking...");
        
        let address = SocketAddr::new(
            IpAddr::from_str(&self.address)?,
            self.port
        );
        
        let mut mesh = P2PMesh::new(
            address,
            self.partition_id,
            self.total_partitions,
            Duration::from_secs(5),  // heartbeat interval
            Duration::from_secs(10), // gossip interval
            Duration::from_secs(30), // node timeout
            8,                       // max neighbors
        );
        
        // Start the mesh
        mesh.start().await?;
        
        // Join cluster with bootstrap nodes
        let bootstrap_addrs: Result<Vec<SocketAddr>, _> = bootstrap_nodes
            .iter()
            .map(|addr| addr.parse::<SocketAddr>())
            .collect();
        
        if let Ok(addrs) = bootstrap_addrs {
            mesh.join_cluster(addrs).await?;
        }
        
        // Store the mesh
        {
            let mut p2p_mesh = self.p2p_mesh.write().await;
            *p2p_mesh = Some(mesh);
        }
        
        // Print initial topology
        {
            let mesh = self.p2p_mesh.read().await;
            if let Some(ref mesh) = *mesh {
                mesh.print_topology().await;
            }
        }
        
        println!("✅ P2P mesh initialized successfully");
        Ok(())
    }

    pub async fn get_mesh_topology(&self) -> Option<rafka_core::p2p_mesh::MeshTopology> {
        let mesh = self.p2p_mesh.read().await;
        if let Some(ref mesh) = *mesh {
            Some(mesh.get_cluster_topology().await)
        } else {
            None
        }
    }

    pub async fn get_node_for_partition(&self, partition_id: u32) -> Option<NodeInfo> {
        let mesh = self.p2p_mesh.read().await;
        if let Some(ref mesh) = *mesh {
            mesh.get_node_for_partition(partition_id).await
        } else {
            None
        }
    }

    async fn ensure_channel(&self, partition_id: u32) -> broadcast::Sender<ConsumeResponse> {
        let mut channels = self.messages.write().await;
        if let Some(sender) = channels.get(&partition_id) {
            if sender.receiver_count() > 0 {
                return sender.clone();
            }
        }
        
        let (new_tx, _) = broadcast::channel(self.broadcast_capacity);
        channels.insert(partition_id, new_tx.clone());
        new_tx
    }

    /// Process a single message (fallback when batching fails)
    async fn process_single_message(&self, req: PublishRequest) -> Result<Response<PublishResponse>, Status> {
        let message_id = Uuid::new_v4().to_string();
        let offset = self.message_counter.fetch_add(1, Ordering::SeqCst) as i64;

        let response = ConsumeResponse {
            message_id: message_id.clone(),
            topic: req.topic.clone(),
            payload: req.payload,
            sent_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            offset,
        };

        let sender = self.ensure_channel(self.partition_id).await;
        if let Err(e) = sender.send(response) {
            println!("Failed to broadcast message: {}", e);
        }

        println!("Message published to partition {} with offset {}", self.partition_id, offset);
        
        Ok(Response::new(PublishResponse {
            message_id,
            success: true,
            message: format!("Published successfully to partition {} with offset {}", 
                self.partition_id, offset),
            partition: self.partition_id as i32,
            offset,
        }))
    }

    /// Process a batch of messages efficiently using zero-copy and memory pooling
    async fn process_batch(&self, batch: crate::batching::MessageBatch) {
        let messages = batch.get_messages();
        if messages.is_empty() {
            return;
        }

        println!("Processing batch of {} messages with zero-copy and memory pooling", messages.len());
        
        // Use zero-copy processing for the batch
        let message_payloads: Vec<bytes::Bytes> = messages.iter()
            .map(|req| bytes::Bytes::from(req.payload.clone()))
            .collect();
        let processed_messages = self.zero_copy_processor.process_batch(&message_payloads).await;
        
        let sender = self.ensure_channel(self.partition_id).await;
        
        for (req, processed_payload) in messages.iter().zip(processed_messages.iter()) {
            // Get optimized message object from pool
            let mut pooled_message = self.message_pool.get().await;
            let optimized_msg = pooled_message.get_mut();
            
            // Populate the pooled message
            optimized_msg.id = Uuid::new_v4().to_string();
            optimized_msg.topic = req.topic.clone();
            optimized_msg.payload = processed_payload.to_vec(); // Convert Bytes to Vec<u8>
            optimized_msg.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            optimized_msg.partition = self.partition_id;
            optimized_msg.offset = self.message_counter.fetch_add(1, Ordering::SeqCst) as i64;

            let response = ConsumeResponse {
                message_id: optimized_msg.id.clone(),
                topic: optimized_msg.topic.clone(),
                payload: String::from_utf8_lossy(&processed_payload).to_string(), // Convert Bytes to String
                sent_at: optimized_msg.timestamp as i64,
                offset: optimized_msg.offset,
            };

            if let Err(e) = sender.send(response) {
                println!("Failed to broadcast message in batch: {}", e);
            }
            
            // Pooled message will be automatically returned to pool when dropped
        }
        
        println!("Batch processed successfully with zero-copy and memory pooling optimization");
    }

    pub async fn shutdown(&self) {
        let mut channels = self.messages.write().await;
        for partition_id in 0..self.total_partitions {
            let (new_tx, _) = broadcast::channel(self.broadcast_capacity);
            channels.insert(partition_id, new_tx);
        }
    }

    pub async fn serve(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let addr = addr.parse()?;
        println!("Broker listening on {}", addr);

        Server::builder()
            .add_service(BrokerServiceServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    fn owns_partition(&self, message_key: &str) -> bool {
        let hash = self.hash_key(message_key);
        hash % self.total_partitions == self.partition_id
    }

    fn hash_key(&self, key: &str) -> u32 {
        key.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }

    async fn ensure_topic(&self, topic: &str) {
        let topics = self.topics.read().await;
        if !topics.contains_key(topic) {
            drop(topics);
            let mut topics = self.topics.write().await;
            if !topics.contains_key(topic) {
                topics.insert(topic.to_string(), HashSet::new());
                self.storage.create_topic(topic.to_string());
                self.storage.create_partition(topic, self.partition_id as i32);
            }
        }
    }

    async fn _publish_internal(&self, response: ConsumeResponse) -> Result<(), broadcast::error::SendError<ConsumeResponse>> {
        let sender = self.ensure_channel(self.partition_id).await;
        sender.send(response).map(|_| ())
    }
    
    async fn _get_consumer_offset(&self, consumer_id: &str, topic: &str) -> i64 {
        let offsets = self.consumer_offsets.read().await;
        offsets.get(&(consumer_id.to_string(), topic.to_string()))
            .copied()
            .unwrap_or(-1)
    }

    async fn set_consumer_offset(&self, consumer_id: &str, topic: &str, offset: i64) {
        let mut offsets = self.consumer_offsets.write().await;
        offsets.insert((consumer_id.to_string(), topic.to_string()), offset);
    }

    pub fn update_retention_policy(&self, max_age: Duration, max_bytes: usize) {
        self.storage.update_retention_policy(RetentionPolicy {
            max_age,
            max_bytes,
        });
    }

    pub fn get_storage_metrics(&self) -> StorageMetrics {
        self.storage.get_metrics()
    }

    async fn _cleanup_old_messages(&self) {
        let metrics = self.storage.get_metrics();
        let policy = self.storage.get_retention_policy();
        if metrics.total_bytes > policy.max_bytes {
            self.storage.cleanup_old_messages().await;
        }
    }

    async fn forward_message_to_partition(
        &self,
        req: PublishRequest,
        target_partition: u32,
    ) -> Result<Response<PublishResponse>, Status> {
        // Check if we own this partition locally first
        if self.owns_partition(&req.key) && self.partition_id == target_partition {
            // Handle locally
            return self.handle_local_message(req).await;
        }
        
        // Try P2P mesh first, fallback to static cluster config
        let target_node = if let Some(node) = self.get_node_for_partition(target_partition).await {
            Some(node)
        } else {
            // Fallback to static cluster config
            self.cluster_manager.get_broker_for_partition(target_partition).await
                .map(|broker| NodeInfo {
                    id: NodeId::from_string(broker.broker_id),
                    address: SocketAddr::new(
                        IpAddr::from_str(&broker.address).unwrap_or(IpAddr::from([127, 0, 0, 1])),
                        broker.port as u16
                    ),
                    partition_id: broker.partition_id,
                    total_partitions: broker.total_partitions,
                    last_seen: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    is_alive: broker.is_healthy,
                    metadata: HashMap::new(),
                })
        };

        if let Some(target_node) = target_node {
            // Create gRPC client to forward message
            let endpoint = format!("http://{}", target_node.address);
            let mut client = match rafka_core::proto::rafka::broker_service_client::BrokerServiceClient::connect(endpoint).await {
                Ok(client) => client,
                Err(e) => {
                    println!("Failed to connect to node {}: {}", target_node.id.0, e);
                    return Err(Status::unavailable(format!("Cannot reach node for partition {}", target_partition)));
                }
            };

            // Forward the message
            let forward_req = ForwardMessageRequest {
                from_broker_id: self.broker_id.clone(),
                topic: req.topic,
                key: req.key,
                payload: req.payload,
                target_partition: target_partition as i32,
                offset: 0, // Will be set by target broker
            };

            match client.forward_message(Request::new(forward_req)).await {
                Ok(response) => {
                    let resp = response.into_inner();
                    Ok(Response::new(PublishResponse {
                        message_id: Uuid::new_v4().to_string(),
                        success: resp.success,
                        message: format!("Message forwarded to partition {} via P2P mesh", target_partition),
                        partition: target_partition as i32,
                        offset: resp.offset,
                    }))
                }
                Err(e) => {
                    println!("Failed to forward message to node {}: {}", target_node.id.0, e);
                    Err(Status::unavailable(format!("Failed to forward message to partition {}", target_partition)))
                }
            }
        } else {
            // If no node found, try to discover it by requesting mesh topology
            println!("🔍 No node found for partition {}, requesting mesh topology...", target_partition);
            
            // Request mesh topology from all known nodes
            let mesh = self.p2p_mesh.read().await;
            if let Some(ref mesh) = *mesh {
                let topology = mesh.get_cluster_topology().await;
                let known_nodes: Vec<_> = topology.nodes.values().collect();
                
                for node in known_nodes {
                    if let Err(e) = self.request_mesh_topology_from_node(node.address).await {
                        println!("Failed to request topology from {}: {}", node.address, e);
                    }
                }
            }
            
            Err(Status::failed_precondition(format!(
                "No node found for partition {}. Mesh topology may still be synchronizing.", target_partition
            )))
        }
    }
    
    async fn handle_local_message(&self, req: PublishRequest) -> Result<Response<PublishResponse>, Status> {
        // Handle message locally using the message pool
        let mut pooled_message = self.message_pool.get().await;
        let optimized_message = pooled_message.get_mut();
        
        // Set message data
        optimized_message.id = Uuid::new_v4().to_string();
        optimized_message.topic = req.topic.clone();
        optimized_message.payload = req.payload.clone().into_bytes();
        optimized_message.timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Process with zero-copy processor using input buffer
        let message_bytes = bytes::Bytes::from(optimized_message.payload.clone());
        let processed_batch = self.zero_copy_processor.process_batch(&[message_bytes]).await
            .map_err(|e| Status::internal(format!("Zero-copy processing failed: {}", e)))?;
        
        // Parse the processed batch back to individual messages
        let processed_messages = self.zero_copy_processor.parse_batch(processed_batch).await
            .map_err(|e| Status::internal(format!("Batch parsing failed: {}", e)))?;
        
        // Store in local partition
        let partition_id = self.partition_id;
        let channel = self.ensure_channel(partition_id).await;
        
        let processed_payload = String::from_utf8(processed_messages[0].to_vec())
            .unwrap_or_else(|_| String::from_utf8_lossy(&optimized_message.payload).to_string());
        
        let response = ConsumeResponse {
            message_id: optimized_message.id.clone(),
            topic: optimized_message.topic.clone(),
            payload: processed_payload.clone(),
            sent_at: optimized_message.timestamp as i64,
            offset: self.message_counter.fetch_add(1, Ordering::SeqCst) as i64,
        };
        
        let _ = channel.send(response);
        
        // PooledMessage will automatically return to pool when dropped
        Ok(Response::new(PublishResponse {
            message_id: optimized_message.id.clone(),
            success: true,
            message: "Message processed locally with zero-copy and memory pooling".to_string(),
            partition: partition_id as i32,
            offset: self.message_counter.load(Ordering::SeqCst) as i64,
        }))
    }
    
    async fn request_mesh_topology_from_node(&self, node_addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = format!("http://{}", node_addr);
        let mut client = rafka_core::proto::rafka::broker_service_client::BrokerServiceClient::connect(endpoint).await?;
        
        let request = rafka_core::proto::rafka::GetMeshTopologyRequest {
            requesting_node_id: self.broker_id.clone(),
        };
        
        match client.get_mesh_topology(Request::new(request)).await {
            Ok(response) => {
                let topology = response.into_inner();
                println!("📊 Received mesh topology from {}: {} nodes, {} partitions", 
                    node_addr, topology.cluster_size, topology.partition_owners.len());
                
                // Update our P2P mesh with the received topology
                let mut mesh = self.p2p_mesh.write().await;
                if let Some(ref mut mesh) = *mesh {
                    let mut local_topology = mesh.topology.write().await;
                    for (partition_id, owner_id) in topology.partition_owners {
                        local_topology.partitions.insert(partition_id as u32, NodeId::from_string(owner_id));
                    }
                }
            }
            Err(e) => {
                println!("Failed to get mesh topology from {}: {}", node_addr, e);
            }
        }
        
        Ok(())
    }
}

#[tonic::async_trait]
impl BrokerService for Broker {
    async fn publish(
        &self,
        request: Request<PublishRequest>,
    ) -> Result<Response<PublishResponse>, Status> {
        let req = request.into_inner();
        
        // Calculate which partition this message belongs to
        let target_partition = self.hash_key(&req.key) % self.total_partitions;
        
        // If message doesn't belong to this broker, forward it
        if target_partition != self.partition_id {
            return self.forward_message_to_partition(req, target_partition).await;
        }

        self.ensure_topic(&req.topic).await;
        
        // Add message to batcher for performance optimization
        if let Err(e) = self.batcher.add_message(req.clone()).await {
            println!("Failed to add message to batcher: {}", e);
            // Fall back to immediate processing
            return self.process_single_message(req).await;
        }

        // Process any ready batches
        let ready_batches = self.batcher.get_ready_batches().await;
        for batch in ready_batches {
            self.process_batch(batch).await;
        }
        
        let message_id = Uuid::new_v4().to_string();
        let offset = self.message_counter.fetch_add(1, Ordering::SeqCst) as i64;
        
        Ok(Response::new(PublishResponse {
            message_id,
            success: true,
            message: format!("Message queued for batch processing"),
            partition: self.partition_id as i32,
            offset,
        }))
    }

    type ConsumeStream = MessageStream;

    async fn consume(
        &self,
        request: Request<ConsumeRequest>,
    ) -> Result<Response<Self::ConsumeStream>, Status> {
        let req = request.into_inner();
        println!("Consumer {} started consuming on partition {}", req.id, self.partition_id);
        
        let sender = self.ensure_channel(self.partition_id).await;
        let rx = sender.subscribe();
        
        Ok(Response::new(MessageStream {
            inner: BroadcastStream::new(rx)
        }))
    }

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();
        println!("Client registered: {} ({:?})", req.client_id, req.client_type);
        
        Ok(Response::new(RegisterResponse {
            success: true,
            message: "Registered successfully".to_string(),
        }))
    }
    //sigma
    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<SubscribeResponse>, Status> {
        let req = request.into_inner();
        
        self.ensure_topic(&req.topic).await;
        
        let mut topics = self.topics.write().await;
        topics
            .entry(req.topic.clone())
            .or_insert_with(HashSet::new)
            .insert(req.consumer_id.clone());

        println!("Consumer {} subscribed to topic {}", req.consumer_id, req.topic);
        
        Ok(Response::new(SubscribeResponse {
            success: true,
            message: "Subscribed successfully".to_string(),
        }))
    }

    async fn acknowledge(
        &self,
        request: Request<AcknowledgeRequest>,
    ) -> Result<Response<AcknowledgeResponse>, Status> {
        let req = request.into_inner();
        println!("Acknowledged message: {} for consumer {}", req.message_id, req.consumer_id);
        
        Ok(Response::new(AcknowledgeResponse {
            success: true,
            message: "Message acknowledged".to_string(),
        }))
    }

    async fn update_offset(
        &self,
        request: Request<UpdateOffsetRequest>
    ) -> Result<Response<UpdateOffsetResponse>, Status> {
        let req = request.into_inner();
        
        if req.offset < 0 {
            return Err(Status::invalid_argument("Offset cannot be negative"));
        }

        let topics = self.topics.read().await;
        if !topics.contains_key(&req.topic) {
            return Err(Status::not_found(format!("Topic {} not found", req.topic)));
        }

        self.set_consumer_offset(&req.consumer_id, &req.topic, req.offset).await;

        println!("Updated offset for consumer {} on topic {} to {}", 
            req.consumer_id, req.topic, req.offset);
        
        Ok(Response::new(UpdateOffsetResponse {
            success: true,
            message: format!("Offset updated to {}", req.offset),
        }))
    }

    async fn get_metrics(
        &self,
        _request: Request<GetMetricsRequest>,
    ) -> Result<Response<GetMetricsResponse>, Status> {
        let metrics = self.storage.get_metrics();
        let oldest_age = metrics.oldest_message
            .elapsed()
            .unwrap_or_default()
            .as_secs();

        Ok(Response::new(GetMetricsResponse {
            total_messages: metrics.total_messages as u64,
            total_bytes: metrics.total_bytes as u64,
            oldest_message_age_secs: oldest_age,
        }))
    }

    // Broker-to-broker communication methods
    async fn forward_message(
        &self,
        request: Request<ForwardMessageRequest>,
    ) -> Result<Response<ForwardMessageResponse>, Status> {
        let req = request.into_inner();
        
        // Check if this broker owns the target partition
        if req.target_partition as u32 != self.partition_id {
            return Err(Status::failed_precondition(format!(
                "Message forwarded to wrong partition. Expected {}, got {}",
                self.partition_id, req.target_partition
            )));
        }

        // Store the forwarded message
        self.ensure_topic(&req.topic).await;
        
        let message_id = Uuid::new_v4().to_string();
        let offset = self.message_counter.fetch_add(1, Ordering::SeqCst) as i64;

        // Create consume response for broadcasting
        let response = ConsumeResponse {
            message_id: message_id.clone(),
            topic: req.topic.clone(),
            payload: req.payload,
            sent_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            offset,
        };

        // Broadcast to local consumers
        let sender = self.ensure_channel(self.partition_id).await;
        if let Err(e) = sender.send(response) {
            println!("Failed to broadcast forwarded message: {}", e);
        }

        println!("Message forwarded from {} to partition {} with offset {}", 
                req.from_broker_id, self.partition_id, offset);

        Ok(Response::new(ForwardMessageResponse {
            success: true,
            message: format!("Message forwarded successfully to partition {}", self.partition_id),
            offset,
        }))
    }

    async fn get_cluster_info(
        &self,
        request: Request<GetClusterInfoRequest>,
    ) -> Result<Response<GetClusterInfoResponse>, Status> {
        let req = request.into_inner();
        println!("Cluster info requested by broker: {}", req.requesting_broker_id);

        let brokers = self.cluster_manager.get_all_brokers().await;
        let proto_brokers: Vec<ProtoBrokerInfo> = brokers.into_iter().map(|broker| {
            ProtoBrokerInfo {
                broker_id: broker.broker_id,
                address: broker.address,
                port: broker.port as i32,
                partition_id: broker.partition_id as i32,
                total_partitions: broker.total_partitions as i32,
                is_healthy: broker.is_healthy,
            }
        }).collect();

        Ok(Response::new(GetClusterInfoResponse {
            brokers: proto_brokers,
            cluster_size: self.cluster_manager.get_all_brokers().await.len() as i32,
            cluster_id: self.cluster_manager.cluster_id().to_string(),
        }))
    }

    async fn join_cluster(
        &self,
        request: Request<JoinClusterRequest>,
    ) -> Result<Response<JoinClusterResponse>, Status> {
        let req = request.into_inner();
        println!("Broker {} requesting to join cluster", req.broker_id);

        // Create broker info for the joining broker
        let broker_info = BrokerInfo {
            broker_id: req.broker_id.clone(),
            address: req.address,
            port: req.port as u16,
            partition_id: req.partition_id as u32,
            total_partitions: req.total_partitions as u32,
            is_healthy: true,
            last_health_check: SystemTime::now(),
        };

        // Add the broker to our cluster manager
        self.cluster_manager.add_broker(broker_info).await;

        // Get all existing brokers to return
        let existing_brokers = self.cluster_manager.get_all_brokers().await;
        let proto_brokers: Vec<ProtoBrokerInfo> = existing_brokers.into_iter().map(|broker| {
            ProtoBrokerInfo {
                broker_id: broker.broker_id,
                address: broker.address,
                port: broker.port as i32,
                partition_id: broker.partition_id as i32,
                total_partitions: broker.total_partitions as i32,
                is_healthy: broker.is_healthy,
            }
        }).collect();

        println!("Broker {} successfully joined cluster", req.broker_id);

        Ok(Response::new(JoinClusterResponse {
            success: true,
            message: format!("Successfully joined cluster"),
            existing_brokers: proto_brokers,
        }))
    }

    async fn health_check(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let req = request.into_inner();
        
        // Update broker health status
        self.cluster_manager.update_broker_health(&req.broker_id, true).await;

        Ok(Response::new(HealthCheckResponse {
            healthy: true,
            status: "OK".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        }))
    }

    // P2P Mesh networking methods
    async fn gossip_message(
        &self,
        request: Request<GossipMessageRequest>,
    ) -> Result<Response<GossipMessageResponse>, Status> {
        let req = request.into_inner();
        
        let mut mesh = self.p2p_mesh.write().await;
        if let Some(ref mut mesh) = *mesh {
            // Convert proto message to internal gossip message
            let from_node_id = req.from_node_id.clone();
            let gossip_msg = GossipMessage {
                from_node: NodeId::from_string(from_node_id.clone()),
                message_type: match req.message_type {
                    1 => MeshGossipMessageType::NodeJoin(NodeInfo::new(
                        SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 50051),
                        0, 3
                    )),
                    2 => MeshGossipMessageType::NodeLeave(NodeId::from_string(from_node_id.clone())),
                    3 => MeshGossipMessageType::NodeUpdate(NodeInfo::new(
                        SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 50051),
                        0, 3
                    )),
                    4 => MeshGossipMessageType::Heartbeat(NodeId::from_string(from_node_id.clone())),
                    5 => MeshGossipMessageType::PartitionUpdate {
                        partition_id: 0,
                        owner: NodeId::from_string(from_node_id.clone())
                    },
                    6 => MeshGossipMessageType::ClusterState(vec![]),
                    _ => return Err(Status::invalid_argument("Unknown gossip message type")),
                },
                timestamp: SystemTime::UNIX_EPOCH + Duration::from_secs(req.timestamp as u64),
                ttl: req.ttl as u32,
            };
            
            // Process the gossip message
            if let Err(e) = mesh.process_gossip_message(gossip_msg).await {
                println!("Failed to process gossip message: {}", e);
                return Err(Status::internal("Failed to process gossip message"));
            }
        }

        Ok(Response::new(GossipMessageResponse {
            success: true,
            message: "Gossip message processed".to_string(),
        }))
    }

    async fn get_mesh_topology(
        &self,
        request: Request<GetMeshTopologyRequest>,
    ) -> Result<Response<GetMeshTopologyResponse>, Status> {
        let req = request.into_inner();
        println!("Mesh topology requested by node: {}", req.requesting_node_id);

        let mesh = self.p2p_mesh.read().await;
        if let Some(ref mesh) = *mesh {
            let topology = mesh.get_cluster_topology().await;
            let partition_owners: HashMap<i32, String> = topology.partitions
                .iter()
                .map(|(partition_id, node_id)| (*partition_id as i32, node_id.0.clone()))
                .collect();
            
            let mesh_nodes: Vec<MeshNodeInfo> = topology.nodes.values()
                .map(|node| MeshNodeInfo {
                    node_id: node.id.0.clone(),
                    address: node.address.ip().to_string(),
                    port: node.address.port() as i32,
                    partition_id: node.partition_id as i32,
                    total_partitions: node.total_partitions as i32,
                    is_alive: node.is_alive,
                    last_seen_timestamp: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64 - node.last_seen as i64,
                    metadata: node.metadata.clone(),
                })
                .collect();

            Ok(Response::new(GetMeshTopologyResponse {
                nodes: mesh_nodes,
                partition_owners,
                cluster_size: topology.nodes.len() as i32,
            }))
        } else {
            Err(Status::failed_precondition("P2P mesh not initialized"))
        }
    }

    async fn join_mesh(
        &self,
        request: Request<JoinMeshRequest>,
    ) -> Result<Response<JoinMeshResponse>, Status> {
        let req = request.into_inner();
        println!("Node {} requesting to join mesh", req.node_id);

        let mesh = self.p2p_mesh.read().await;
        if let Some(ref mesh) = *mesh {
            let topology = mesh.get_cluster_topology().await;
            let existing_nodes: Vec<MeshNodeInfo> = topology.nodes.values()
                .map(|node| MeshNodeInfo {
                    node_id: node.id.0.clone(),
                    address: node.address.ip().to_string(),
                    port: node.address.port() as i32,
                    partition_id: node.partition_id as i32,
                    total_partitions: node.total_partitions as i32,
                    is_alive: node.is_alive,
                    last_seen_timestamp: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64 - node.last_seen as i64,
                    metadata: node.metadata.clone(),
                })
                .collect();

            Ok(Response::new(JoinMeshResponse {
                success: true,
                message: "Successfully joined mesh".to_string(),
                existing_nodes,
            }))
        } else {
            Err(Status::failed_precondition("P2P mesh not initialized"))
        }
    }

    async fn leave_mesh(
        &self,
        request: Request<LeaveMeshRequest>,
    ) -> Result<Response<LeaveMeshResponse>, Status> {
        let req = request.into_inner();
        println!("Node {} requesting to leave mesh", req.node_id);

        let mut mesh = self.p2p_mesh.write().await;
        if let Some(ref mut mesh) = *mesh {
            if let Err(e) = mesh.leave_cluster().await {
                println!("Failed to leave mesh: {}", e);
                return Err(Status::internal("Failed to leave mesh"));
            }
        }

        Ok(Response::new(LeaveMeshResponse {
            success: true,
            message: "Successfully left mesh".to_string(),
        }))
    }
}

pub struct MessageStream {
    pub(crate) inner: BroadcastStream<ConsumeResponse>,
}

impl Stream for MessageStream {
    type Item = Result<ConsumeResponse, Status>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(msg))) => Poll::Ready(Some(Ok(msg))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(Status::internal(e.to_string())))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
