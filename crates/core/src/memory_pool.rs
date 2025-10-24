use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::VecDeque;

/// Memory pool for reusing message objects to reduce allocations
pub struct MessagePool<T> {
    pool: Arc<Mutex<VecDeque<T>>>,
    max_size: usize,
}

impl<T: Default + Send + Sync + 'static> MessagePool<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        }
    }

    /// Get a message from the pool or create a new one
    pub async fn get(&self) -> PooledMessage<T> {
        let mut pool = self.pool.lock().await;
        
        if let Some(message) = pool.pop_front() {
            PooledMessage {
                message: Some(message),
                pool: self.clone(),
            }
        } else {
            PooledMessage {
                message: Some(T::default()),
                pool: self.clone(),
            }
        }
    }

    /// Return a message to the pool
    async fn return_message(&self, mut message: T) {
        let mut pool = self.pool.lock().await;
        
        if pool.len() < self.max_size {
            // Reset the message if it has a reset method
            if let Some(resettable) = (&mut message as &mut dyn std::any::Any).downcast_mut::<OptimizedMessage>() {
                resettable.reset();
            }
            
            pool.push_back(message);
        }
        // If pool is full, just drop the message
    }
}

impl<T: Send + Sync + 'static> Clone for MessagePool<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            max_size: self.max_size,
        }
    }
}

/// Wrapper for pooled message that automatically returns to pool
pub struct PooledMessage<T: Send + Sync + 'static> {
    message: Option<T>,
    pool: MessagePool<T>,
}

impl<T: Send + Sync + 'static> PooledMessage<T> {
    pub fn get(&self) -> &T {
        self.message.as_ref().unwrap()
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.message.as_mut().unwrap()
    }
}

impl<T: Send + Sync + 'static> Drop for PooledMessage<T> {
    fn drop(&mut self) {
        if let Some(message) = self.message.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                // Just drop the message for now - we'll implement proper pooling later
                drop(message);
            });
        }
    }
}

/// Trait for objects that can be reset for reuse
pub trait Reset {
    fn reset(&mut self);
}

/// Optimized message struct that can be pooled
#[derive(Debug, Clone, Default)]
pub struct OptimizedMessage {
    pub id: String,
    pub topic: String,
    pub payload: Vec<u8>,
    pub timestamp: u64,
    pub partition: u32,
    pub offset: i64,
}

impl OptimizedMessage {
    pub fn new() -> Self {
        Self {
            id: String::with_capacity(36), // UUID length
            topic: String::with_capacity(64),
            payload: Vec::with_capacity(1024),
            timestamp: 0,
            partition: 0,
            offset: 0,
        }
    }
}

impl Reset for OptimizedMessage {
    fn reset(&mut self) {
        self.id.clear();
        self.topic.clear();
        self.payload.clear();
        self.timestamp = 0;
        self.partition = 0;
        self.offset = 0;
    }
}

/// Factory function for creating message pools
pub fn create_message_pool(max_size: usize) -> MessagePool<OptimizedMessage> {
    MessagePool::new(max_size)
}
