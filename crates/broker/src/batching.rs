use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};
use rafka_core::proto::rafka::PublishRequest;

/// High-performance message batch for reducing network overhead
#[derive(Debug, Clone)]
pub struct MessageBatch {
    pub messages: Vec<PublishRequest>,
    pub batch_size: usize,
    pub max_wait_time: Duration,
    pub created_at: Instant,
}

impl MessageBatch {
    pub fn new(batch_size: usize, max_wait_time: Duration) -> Self {
        Self {
            messages: Vec::with_capacity(batch_size),
            batch_size,
            max_wait_time,
            created_at: Instant::now(),
        }
    }

    pub fn add_message(&mut self, message: PublishRequest) -> bool {
        if self.messages.len() >= self.batch_size {
            return false; // Batch is full
        }
        
        self.messages.push(message);
        true
    }

    pub fn is_ready(&self) -> bool {
        self.messages.len() >= self.batch_size || 
        self.created_at.elapsed() >= self.max_wait_time
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn take_messages(&mut self) -> Vec<PublishRequest> {
        std::mem::take(&mut self.messages)
    }
}

/// Batched message processor for high throughput
pub struct BatchedProcessor {
    batches: Arc<Mutex<VecDeque<MessageBatch>>>,
    batch_size: usize,
    max_wait_time: Duration,
}

impl BatchedProcessor {
    pub fn new(batch_size: usize, max_wait_time: Duration) -> Self {
        Self {
            batches: Arc::new(Mutex::new(VecDeque::new())),
            batch_size,
            max_wait_time,
        }
    }

    pub async fn add_message(&self, message: PublishRequest) -> Result<(), String> {
        let mut batches = self.batches.lock().await;
        
        // Try to add to existing batch
        if let Some(batch) = batches.back_mut() {
            if batch.add_message(message.clone()) {
                return Ok(());
            }
        }
        
        // Create new batch
        let mut new_batch = MessageBatch::new(self.batch_size, self.max_wait_time);
        new_batch.add_message(message);
        batches.push_back(new_batch);
        
        Ok(())
    }

    pub async fn get_ready_batches(&self) -> Vec<MessageBatch> {
        let mut batches = self.batches.lock().await;
        let mut ready_batches = Vec::new();
        
        while let Some(batch) = batches.front() {
            if batch.is_ready() {
                if let Some(batch) = batches.pop_front() {
                    if !batch.is_empty() {
                        ready_batches.push(batch);
                    }
                }
            } else {
                break;
            }
        }
        
        ready_batches
    }
}
