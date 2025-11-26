use std::sync::Arc;
use dashmap::DashMap;
use bytes::Bytes;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::time::{SystemTime, Duration};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::fs::{File, OpenOptions};
use std::io::{self, Write, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use bincode;

// WalLog with async IO and separate read/write handles
pub(crate) struct WalLog {
    write_path: PathBuf,
    next_entry_id: AtomicUsize,
}

impl WalLog {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Ensure file exists
        if !path.exists() {
            std::fs::File::create(&path)?;
        }
            
        Ok(Self {
            write_path: path,
            next_entry_id: AtomicUsize::new(0),
        })
    }

    pub async fn append(&self, entry: &MessageEntry) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;
        
        let encoded: Vec<u8> = bincode::serialize(entry)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        let len = encoded.len() as u64;
        
        // Use tokio::fs for async IO to avoid blocking the runtime
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.write_path)
            .await?;
        
        file.write_all(&len.to_le_bytes()).await?;
        file.write_all(&encoded).await?;
        file.flush().await?;
        
        self.next_entry_id.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub async fn read_all(&self) -> io::Result<Vec<MessageEntry>> {
        use tokio::io::AsyncReadExt;
        
        let mut file = tokio::fs::File::open(&self.write_path).await?;
        let metadata = file.metadata().await?;
        
        let mut entries = Vec::new();
        let mut buffer = [0u8; 8];

        loop {
            match file.read_exact(&mut buffer).await {
                Ok(_) => {
                    let len = u64::from_le_bytes(buffer) as usize;
                    let mut data = vec![0u8; len];
                    file.read_exact(&mut data).await?;
                    
                    match bincode::deserialize(&data) {
                        Ok(entry) => entries.push(entry),
                        Err(e) => {
                            return Err(io::Error::new(io::ErrorKind::InvalidData, e));
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
        }
        
        Ok(entries)
    }
    
    // Compact WAL by removing old entries
    pub async fn compact(&self, keep_from_offset: i64) -> io::Result<()> {
        let entries = self.read_all().await?;
        let kept_entries: Vec<_> = entries.into_iter()
            .filter(|e| e.offset >= keep_from_offset)
            .collect();
        
        // Write to temp file
        let temp_path = self.write_path.with_extension("tmp");
        {
            use tokio::io::AsyncWriteExt;
            let mut temp_file = tokio::fs::File::create(&temp_path).await?;
            
            for entry in &kept_entries {
                let encoded: Vec<u8> = bincode::serialize(entry)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                let len = encoded.len() as u64;
                temp_file.write_all(&len.to_le_bytes()).await?;
                temp_file.write_all(&encoded).await?;
            }
            temp_file.flush().await?;
        }
        
        // Atomic rename
        tokio::fs::rename(&temp_path, &self.write_path).await?;
        Ok(())
    }
}


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

// Private implementation - using Bytes for zero-copy
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct MessageEntry {
    offset: i64,
    #[serde(with = "serde_bytes")]
    payload: Vec<u8>, // Keep as Vec for serialization, convert to Bytes on read
    timestamp: SystemTime,
    partition_id: i32,
    // Store acknowledgments persistently
    acknowledged_by: std::collections::HashSet<String>,
}

mod serde_bytes {
    use serde::{Serializer, Deserializer};

    pub fn serialize<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(data)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &[u8] = serde::Deserialize::deserialize(deserializer)?;
        Ok(s.to_vec())
    }
}


impl MessageEntry {
    fn to_stored_message(&self) -> StoredMessage {
        StoredMessage {
            offset: self.offset,
            payload: Bytes::from(self.payload.clone()), // Single clone on read
            timestamp: self.timestamp,
            partition_id: self.partition_id,
        }
    }
}

// Represents a partition's message queue with offset indexing
struct PartitionQueue {
    messages: RwLock<VecDeque<MessageEntry>>,
    // O(1) offset lookup index
    offset_index: RwLock<std::collections::HashMap<i64, usize>>,
    next_offset: RwLock<i64>,
    retention_policy: RetentionPolicy,
    current_size: AtomicUsize,
    wal: Option<Arc<WalLog>>,
}


impl PartitionQueue {
    fn new(retention_policy: RetentionPolicy, topic: &str, partition_id: i32) -> Self {
        let mut messages = VecDeque::new();
        let mut offset_index = std::collections::HashMap::new();
        let mut next_offset = 0;
        let mut current_size = 0;
        
        // Initialize WAL
        let wal_path = PathBuf::from("data").join(topic).join(format!("partition-{}.log", partition_id));
        let wal = match WalLog::new(wal_path) {
            Ok(w) => {
                // Recover from WAL - must be sync for constructor
                // We'll do async recovery separately
                Some(Arc::new(w))
            },
            Err(e) => {
                eprintln!("Failed to initialize WAL for {}/{}: {}", topic, partition_id, e);
                None
            }
        };
        
        Self {
            messages: RwLock::new(messages),
            offset_index: RwLock::new(offset_index),
            next_offset: RwLock::new(next_offset),
            retention_policy,
            current_size: AtomicUsize::new(current_size),
            wal,
        }
    }
    
    // Async recovery method to be called after construction
    async fn recover(&self) -> io::Result<()> {
        if let Some(ref wal) = self.wal {
            let entries = wal.read_all().await?;
            let mut messages = self.messages.write();
            let mut offset_index = self.offset_index.write();
            let mut next_offset = self.next_offset.write();
            let mut total_size = 0;
            
            for (idx, entry) in entries.into_iter().enumerate() {
                if entry.offset >= *next_offset {
                    *next_offset = entry.offset + 1;
                }
                total_size += entry.payload.len();
                offset_index.insert(entry.offset, idx);
                messages.push_back(entry);
            }
            
            self.current_size.store(total_size, Ordering::SeqCst);
        }
        Ok(())
    }

    // Async append with proper error handling
    async fn append(&self, payload: Bytes, partition_id: i32) -> Result<i64, io::Error> {
        let offset = {
            let mut next_offset = self.next_offset.write();
            let offset = *next_offset;
            *next_offset += 1;
            offset
        };

        let entry = MessageEntry {
            offset,
            payload: payload.to_vec(), // Single conversion
            timestamp: SystemTime::now(),
            partition_id,
            acknowledged_by: std::collections::HashSet::new(),
        };

        // Write to WAL first (durability)
        if let Some(wal) = &self.wal {
            wal.append(&entry).await?; // Propagate errors
        }
        
        // Then update memory
        let payload_len = entry.payload.len();
        {
            let mut messages = self.messages.write();
            let mut offset_index = self.offset_index.write();
            let idx = messages.len();
            offset_index.insert(offset, idx);
            messages.push_back(entry);
        }
        // Update current size atomically
        self.current_size.fetch_add(payload_len, Ordering::SeqCst);
        
        // Enforce retention policy
        self.enforce_retention_policy();
        
        Ok(offset)
    }

    fn enforce_retention_policy(&self) {
        let mut messages = self.messages.write();
        let mut offset_index = self.offset_index.write();
        let now = SystemTime::now();
        let mut removed_size = 0;

        // Remove old messages
        while let Some(entry) = messages.front() {
            let should_remove = if let Ok(age) = now.duration_since(entry.timestamp) {
                age > self.retention_policy.max_age || 
                self.current_size.load(Ordering::SeqCst) > self.retention_policy.max_bytes
            } else {
                false
            };
            
            if should_remove {
                if let Some(removed) = messages.pop_front() {
                    offset_index.remove(&removed.offset);
                    removed_size += removed.payload.len();
                }
            } else {
                break;
            }
        }
        
        // Rebuild index after removal
        offset_index.clear();
        for (idx, entry) in messages.iter().enumerate() {
            offset_index.insert(entry.offset, idx);
        }

        // Atomically update size
        self.current_size.fetch_sub(removed_size, Ordering::SeqCst);
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

    // O(1) acknowledgment using offset index
    fn acknowledge(&self, offset: i64, consumer_id: &str) -> bool {
        let offset_index = self.offset_index.read();
        if let Some(&idx) = offset_index.get(&offset) {
            drop(offset_index); // Release read lock
            let mut messages = self.messages.write();
            if let Some(entry) = messages.get_mut(idx) {
                entry.acknowledged_by.insert(consumer_id.to_string());
                return true;
            }
        }
        false
    }

    fn cleanup_acknowledged(&self) {
        let mut messages = self.messages.write();
        let mut offset_index = self.offset_index.write();
        let mut removed_size = 0;
        
        messages.retain(|msg| {
            let should_keep = msg.acknowledged_by.is_empty();
            if !should_keep {
                removed_size += msg.payload.len();
                offset_index.remove(&msg.offset);
            }
            should_keep
        });
        
        // Rebuild index
        offset_index.clear();
        for (idx, entry) in messages.iter().enumerate() {
            offset_index.insert(entry.offset, idx);
        }
        
        // Atomically decrement size
        self.current_size.fetch_sub(removed_size, Ordering::SeqCst);
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

    pub async fn create_partition(&self, topic: &str, partition_id: i32) -> Result<(), String> {
        if let Some(partitions) = self.topics.get(topic) {
            let queue = Arc::new(PartitionQueue::new(*self.retention_policy.read(), topic, partition_id));
            
            // Recover from WAL asynchronously
            if let Err(e) = queue.recover().await {
                return Err(format!("Failed to recover partition {}/{}: {}", topic, partition_id, e));
            }
            
            partitions.insert(partition_id, queue);
            Ok(())
        } else {
            Err(format!("Topic {} does not exist", topic))
        }
    }

    pub async fn append(&self, topic: &str, partition_id: i32, message: &Bytes) -> Result<i64, String> {
        if let Some(partitions) = self.topics.get(topic) {
            if let Some(queue) = partitions.get(&partition_id) {
                queue.append(message.clone(), partition_id)
                    .await
                    .map_err(|e| format!("Failed to append message: {}", e))
            } else {
                Err(format!("Partition {} not found for topic {}", partition_id, topic))
            }
        } else {
            Err(format!("Topic {} not found", topic))
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

    #[tokio::test]
    async fn test_basic_operations() {
        let storage = Storage::new();
        
        // Create topic and partition
        storage.create_topic("test".to_string());
        storage.create_partition("test", 0).await.expect("Failed to create partition");

        // Append and read message
        let message = Bytes::from("hello world");
        let offset = storage.append("test", 0, &message).await.expect("Failed to append");
        
        let read_messages = storage.read("test", 0, offset).unwrap();
        assert_eq!(read_messages[0].payload, message);
    }
}
