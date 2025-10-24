use rafka_core::proto::rafka::{
    PublishRequest, PublishResponse,
};
use tonic::{Request, transport::Endpoint};
use uuid::Uuid;
use crate::connection_pool::ConnectionPool;

pub struct Producer {
    connection_pool: ConnectionPool,
    producer_id: String,
}

impl Producer {
    pub async fn new(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let endpoint = Endpoint::from_shared(format!("http://{}", addr))?;
        let connection_pool = ConnectionPool::new(endpoint, 10); // Max 10 connections
        let producer_id = Uuid::new_v4().to_string();

        println!("Producer registered with ID: {} (using connection pool)", producer_id);

        Ok(Self {
            connection_pool,
            producer_id,
        })
    }

    pub async fn publish(
        &mut self,
        topic: String,
        message: String,
        key: String,
    ) -> Result<PublishResponse, Box<dyn std::error::Error>> {
        // Get connection from pool
        let mut client = self.connection_pool.get_connection().await?;
        
        let response = client
            .publish(Request::new(PublishRequest {
                producer_id: self.producer_id.clone(),
                topic,
                key,
                payload: message,
            }))
            .await?;

        let result = response.into_inner();
        println!("Message published to partition {} with offset {}", 
            result.partition, result.offset);

        // Return connection to pool
        self.connection_pool.return_connection(client).await;

        Ok(result)
    }
}
