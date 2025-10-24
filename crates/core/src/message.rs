use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub topic: String,
    pub payload: Vec<u8>,
    pub timestamp: DateTime<Utc>,
}

impl Message {
    pub fn new(topic: String, payload: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            topic,
            payload,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAck {
    pub message_id: String,
    pub status: AckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AckStatus {
    Success,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub message_id: String,
    pub sent_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub latency_ms: i64,
}
