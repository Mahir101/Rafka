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
};
use std::sync::atomic::{AtomicUsize, Ordering};
use rafka_storage::db::{Storage, RetentionPolicy, StorageMetrics};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use std::time::Duration;

pub struct Broker {
    topics: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    messages: Arc<RwLock<HashMap<u32, broadcast::Sender<ConsumeResponse>>>>,
    message_counter: AtomicUsize,
    broadcast_capacity: usize,
    partition_id: u32,
    total_partitions: u32,
    storage: Arc<Storage>,
    consumer_offsets: Arc<RwLock<HashMap<(String, String), i64>>>,
}

impl Broker {
    pub fn new(partition_id: u32, total_partitions: u32, retention_policy: Option<RetentionPolicy>) -> Self {
        const BROADCAST_CAPACITY: usize = 1024 * 16;
        
        let storage = Arc::new(Storage::with_retention_policy(
            retention_policy.unwrap_or_default()
        ));
        
        Self {
            topics: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            message_counter: AtomicUsize::new(0),
            broadcast_capacity: BROADCAST_CAPACITY,
            partition_id,
            total_partitions,
            storage,
            consumer_offsets: Arc::new(RwLock::new(HashMap::new())),
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
}

#[tonic::async_trait]
impl BrokerService for Broker {
    async fn publish(
        &self,
        request: Request<PublishRequest>,
    ) -> Result<Response<PublishResponse>, Status> {
        let req = request.into_inner();
        
        if !self.owns_partition(&req.key) {
            return Err(Status::failed_precondition(format!(
                "Message belongs to partition {} not {}",
                self.hash_key(&req.key) % self.total_partitions,
                self.partition_id
            )));
        }

        self.ensure_topic(&req.topic).await;
        
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
