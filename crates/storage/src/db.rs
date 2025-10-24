use std::sync::Arc;
use dashmap::DashMap;
use bytes::Bytes;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::time::{SystemTime, Duration};
use std::sync::atomic::{AtomicUsize, Ordering};

// Add RetentionPolicy struct definition at the top
#[derive(Clone, Copy, Debug)]
pub struct RetentionPolicy {
    pub max_age: Duration,
    pub max_bytes: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            max_bytes: 1024 * 1024 * 1024, // 1GB
        }
    }
}

// Public interface for message data
#[derive(Clone)]
pub struct StoredMessage {
    pub offset: i64,
    pub payload: Bytes,
    pub timestamp: SystemTime,
    pub partition_id: i32,
}

// Private implementation
#[derive(Clone)]
struct MessageEntry {
    offset: i64,
    payload: Bytes,
    timestamp: SystemTime,
    partition_id: i32,
    acknowledged_by: DashMap<String, bool>,
}

impl MessageEntry {
    fn to_stored_message(&self) -> StoredMessage {
        StoredMessage {
            offset: self.offset,
            payload: self.payload.clone(),
            timestamp: self.timestamp,
            partition_id: self.partition_id,
        }
    }
}

// Represents a partition's message queue
struct PartitionQueue {
    messages: RwLock<VecDeque<MessageEntry>>,
    next_offset: RwLock<i64>,
    retention_policy: RetentionPolicy,
    current_size: AtomicUsize,
}

impl PartitionQueue {
    fn new(retention_policy: RetentionPolicy) -> Self {
        Self {
            messages: RwLock::new(VecDeque::new()),
            next_offset: RwLock::new(0),
            retention_policy,
            current_size: AtomicUsize::new(0),
        }
    }

    fn append(&self, payload: Bytes, partition_id: i32) -> i64 {
        let mut messages = self.messages.write();
        let mut next_offset = self.next_offset.write();
        
        let offset = *next_offset;
        *next_offset += 1;

        let entry = MessageEntry {
            offset,
            payload: payload.clone(),
            timestamp: SystemTime::now(),
            partition_id,
            acknowledged_by: DashMap::new(),
        };

        // Update current size
        self.current_size.fetch_add(payload.len(), Ordering::SeqCst);
        
        messages.push_back(entry);
        self.enforce_retention_policy();
        
        offset
    }

    fn enforce_retention_policy(&self) {
        let mut messages = self.messages.write();
        let now = SystemTime::now();
        let mut size = self.current_size.load(Ordering::SeqCst);

        // Remove old messages
        while let Some(entry) = messages.front() {
            if let Ok(age) = now.duration_since(entry.timestamp) {
                if age > self.retention_policy.max_age || size > self.retention_policy.max_bytes {
                    if let Some(removed) = messages.pop_front() {
                        size -= removed.payload.len();
                    }
                    continue;
                }
            }
            break;
        }

        self.current_size.store(size, Ordering::SeqCst);
    }

    fn read_from(&self, start_offset: i64, max_messages: usize) -> Vec<MessageEntry> {
        let messages = self.messages.read();
        messages
            .iter()
            .filter(|entry| entry.offset >= start_offset)
            .take(max_messages)
            .cloned()
            .collect()
    }

    fn acknowledge(&self, offset: i64, consumer_id: &str) {
        let messages = self.messages.read();
        if let Some(entry) = messages.iter().find(|e| e.offset == offset) {
            entry.acknowledged_by.insert(consumer_id.to_string(), true);
        }
    }

    fn cleanup_acknowledged(&self) {
        let mut messages = self.messages.write();
        messages.retain(|msg| msg.acknowledged_by.is_empty());
        // Update size after cleanup
        self.current_size.store(
            messages.iter().map(|msg| msg.payload.len()).sum(),
            Ordering::SeqCst
        );
    }
}

pub struct Storage {
    // topic -> partition_id -> queue
    topics: DashMap<String, DashMap<i32, Arc<PartitionQueue>>>,
    consumer_offsets: DashMap<String, DashMap<(String, i32), i64>>,
    retention_policy: RwLock<RetentionPolicy>,
}

impl Storage {
    pub fn new() -> Self {
        Self::with_retention_policy(RetentionPolicy::default())
    }

    pub fn with_retention_policy(retention_policy: RetentionPolicy) -> Self {
        Self {
            topics: DashMap::new(),
            consumer_offsets: DashMap::new(),
            retention_policy: RwLock::new(retention_policy),
        }
    }

    pub fn create_topic(&self, topic: String) {
        self.topics.insert(topic, DashMap::new());
    }

    pub fn create_partition(&self, topic: &str, partition_id: i32) -> bool {
        if let Some(partitions) = self.topics.get(topic) {
            partitions.insert(partition_id, Arc::new(PartitionQueue::new(*self.retention_policy.read())));
            true
        } else {
            false
        }
    }

    pub fn append(&self, topic: &str, partition_id: i32, message: &Bytes) -> Option<i64> {
        if let Some(partitions) = self.topics.get(topic) {
            if let Some(queue) = partitions.get(&partition_id) {
                Some(queue.append(message.clone(), partition_id))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn read(&self, topic: &str, partition_id: i32, start_offset: i64) -> Option<Vec<StoredMessage>> {
        if let Some(partitions) = self.topics.get(topic) {
            if let Some(queue) = partitions.get(&partition_id) {
                Some(queue.read_from(start_offset, 100)
                    .into_iter()
                    .map(|entry| entry.to_stored_message())
                    .collect())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn acknowledge(&self, topic: &str, partition_id: i32, offset: i64, consumer_id: &str) {
        if let Some(partitions) = self.topics.get(topic) {
            if let Some(queue) = partitions.get(&partition_id) {
                queue.acknowledge(offset, consumer_id);
            }
        }
    }

    pub fn cleanup(&self) {
        for topic in self.topics.iter() {
            for partition in topic.value().iter() {
                partition.value().cleanup_acknowledged();
            }
        }
    }

    // Track consumer's last read position
    pub fn update_consumer_offset(&self, consumer_id: &str, topic: &str, partition_id: i32, offset: i64) {
        self.consumer_offsets
            .entry(consumer_id.to_string())
            .or_insert_with(DashMap::new)
            .insert((topic.to_string(), partition_id), offset);
    }

    // Get consumer's last position
    pub fn get_consumer_offset(&self, consumer_id: &str, topic: &str, partition_id: i32) -> Option<i64> {
        self.consumer_offsets
            .get(consumer_id)?
            .get(&(topic.to_string(), partition_id))
            .map(|r| *r.value())
    }

    // Read messages from consumer's last position
    pub fn read_from_offset(&self, topic: &str, partition_id: i32, consumer_id: &str) -> Option<Vec<StoredMessage>> {
        let start_offset = self.get_consumer_offset(consumer_id, topic, partition_id)
            .unwrap_or(0);
        
        self.read(topic, partition_id, start_offset)
    }

    // Add method to update retention policy
    pub fn update_retention_policy(&self, policy: RetentionPolicy) {
        *self.retention_policy.write() = policy;
        // Enforce new policy across all partitions
        for topic in self.topics.iter() {
            for partition in topic.value().iter() {
                partition.value().enforce_retention_policy();
            }
        }
    }

    // Add method to get storage metrics
    pub fn get_metrics(&self) -> StorageMetrics {
        let mut total_messages = 0;
        let mut total_bytes = 0;
        let mut oldest_message = SystemTime::now();

        for topic in self.topics.iter() {
            for partition in topic.value().iter() {
                let queue = partition.value();
                let messages = queue.messages.read();
                total_messages += messages.len();
                total_bytes += queue.current_size.load(Ordering::SeqCst);
                if let Some(first) = messages.front() {
                    if first.timestamp < oldest_message {
                        oldest_message = first.timestamp;
                    }
                }
            }
        }

        StorageMetrics {
            total_messages,
            total_bytes,
            oldest_message,
        }
    }

    pub async fn cleanup_old_messages(&self) {
        let _policy = *self.retention_policy.read();
        
        for topic in self.topics.iter() {
            for partition in topic.value().iter() {
                partition.value().enforce_retention_policy();
            }
        }
    }

    pub fn get_retention_policy(&self) -> RetentionPolicy {
        *self.retention_policy.read()
    }
}

// Add StorageMetrics struct
#[derive(Debug)]
pub struct StorageMetrics {
    pub total_messages: usize,
    pub total_bytes: usize,
    pub oldest_message: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let storage = Storage::new();
        
        // Create topic and partition
        storage.create_topic("test".to_string());
        assert!(storage.create_partition("test", 0));

        // Append and read message
        let message = Bytes::from("hello world");
        let offset = storage.append("test", 0, &message).unwrap();
        
        let read_messages = storage.read("test", 0, offset).unwrap();
        assert_eq!(read_messages[0].payload, message);
    }
}
