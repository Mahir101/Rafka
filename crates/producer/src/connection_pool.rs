use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint};
use rafka_core::proto::rafka::broker_service_client::BrokerServiceClient;

/// Simplified connection pool for broker clients
pub struct ConnectionPool {
    connections: Arc<Mutex<Vec<BrokerServiceClient<Channel>>>>,
    endpoint: Endpoint,
    max_connections: usize,
}

impl ConnectionPool {
    pub fn new(
        endpoint: Endpoint,
        max_connections: usize,
    ) -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            endpoint,
            max_connections,
        }
    }

    pub async fn get_connection(&self) -> Result<BrokerServiceClient<Channel>, String> {
        let mut connections = self.connections.lock().await;
        
        // Try to get existing connection
        if let Some(client) = connections.pop() {
            return Ok(client);
        }

        // Create new connection
        let client = BrokerServiceClient::connect(self.endpoint.clone()).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        Ok(client)
    }

    pub async fn return_connection(&self, client: BrokerServiceClient<Channel>) {
        let mut connections = self.connections.lock().await;
        
        if connections.len() < self.max_connections {
            connections.push(client);
        }
        // If pool is full, just drop the connection
    }
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            endpoint: self.endpoint.clone(),
            max_connections: self.max_connections,
        }
    }
}