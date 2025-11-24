use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerConfig {
    pub broker_id: String,
    pub address: String,
    pub port: u16,
    pub partition_id: u32,
    pub total_partitions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub cluster_id: String,
    pub brokers: Vec<BrokerConfig>,
    pub health_check_interval_ms: u64,
    pub connection_timeout_ms: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            cluster_id: "rafka-cluster".to_string(),
            brokers: Vec::new(),
            health_check_interval_ms: 5000, // 5 seconds
            connection_timeout_ms: 3000,    // 3 seconds
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrokerInfo {
    pub broker_id: String,
    pub address: String,
    pub port: u16,
    pub partition_id: u32,
    pub total_partitions: u32,
    pub is_healthy: bool,
    pub last_health_check: SystemTime,
}

impl BrokerInfo {
    pub fn new(config: BrokerConfig) -> Self {
        Self {
            broker_id: config.broker_id,
            address: config.address,
            port: config.port,
            partition_id: config.partition_id,
            total_partitions: config.total_partitions,
            is_healthy: true,
            last_health_check: SystemTime::now(),
        }
    }

    pub fn to_address(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

pub struct ClusterManager {
    brokers: Arc<RwLock<HashMap<String, BrokerInfo>>>,
    cluster_id: String,
    health_check_interval_ms: u64,
}

impl ClusterManager {
    pub fn new(cluster_id: String, health_check_interval_ms: u64) -> Self {
        Self {
            brokers: Arc::new(RwLock::new(HashMap::new())),
            cluster_id,
            health_check_interval_ms,
        }
    }

    pub fn from_config(config: ClusterConfig) -> Self {
        let manager = Self::new(config.cluster_id, config.health_check_interval_ms);
        
        // Add brokers from config
        for broker_config in config.brokers {
            let broker_info = BrokerInfo::new(broker_config);
            let broker_id = broker_info.broker_id.clone();
            let brokers = manager.brokers.clone();
            tokio::spawn(async move {
                let mut brokers = brokers.write().await;
                brokers.insert(broker_id, broker_info);
            });
        }
        
        manager
    }

    pub async fn add_broker(&self, broker_info: BrokerInfo) {
        let broker_id = broker_info.broker_id.clone();
        let mut brokers = self.brokers.write().await;
        brokers.insert(broker_id, broker_info);
    }

    pub async fn remove_broker(&self, broker_id: &str) {
        let mut brokers = self.brokers.write().await;
        brokers.remove(broker_id);
    }

    pub async fn get_broker(&self, broker_id: &str) -> Option<BrokerInfo> {
        let brokers = self.brokers.read().await;
        brokers.get(broker_id).cloned()
    }

    pub async fn get_all_brokers(&self) -> Vec<BrokerInfo> {
        let brokers = self.brokers.read().await;
        brokers.values().cloned().collect()
    }

    pub async fn get_broker_for_partition(&self, partition_id: u32) -> Option<BrokerInfo> {
        let brokers = self.brokers.read().await;
        brokers.values()
            .find(|broker| broker.partition_id == partition_id)
            .cloned()
    }

    pub async fn update_broker_health(&self, broker_id: &str, is_healthy: bool) {
        let mut brokers = self.brokers.write().await;
        if let Some(broker) = brokers.get_mut(broker_id) {
            broker.is_healthy = is_healthy;
            broker.last_health_check = SystemTime::now();
        }
    }

    pub async fn get_healthy_brokers(&self) -> Vec<BrokerInfo> {
        let brokers = self.brokers.read().await;
        brokers.values()
            .filter(|broker| broker.is_healthy)
            .cloned()
            .collect()
    }

    pub fn cluster_id(&self) -> &str {
        &self.cluster_id
    }

    pub fn health_check_interval_ms(&self) -> u64 {
        self.health_check_interval_ms
    }
}
