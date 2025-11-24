use crate::stream::{KStream, StreamRecord};
use rafka_consumer::Consumer;
use rafka_producer::Producer;
use std::sync::Arc;
use serde::{Serialize, de::DeserializeOwned};

/// Builder for creating stream processing topologies
pub struct StreamBuilder {
    broker_address: String,
}

impl StreamBuilder {
    pub fn new(broker_address: impl Into<String>) -> Self {
        Self {
            broker_address: broker_address.into(),
        }
    }

    /// Create a stream from a topic
    pub async fn stream<K, V>(&self, topic: impl Into<String>) -> Result<KStream<K, V>, Box<dyn std::error::Error>>
    where
        K: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    {
        let topic = topic.into();
        let consumer = Consumer::new(&self.broker_address).await?;
        consumer.subscribe(topic.clone()).await?;
        
        let mut rx = consumer.consume(topic).await?;
        
        let stream = async_stream::stream! {
            while let Some(message) = rx.recv().await {
                // Parse message into StreamRecord
                if let (Ok(key), Ok(value)) = (
                    serde_json::from_str::<K>(&message),
                    serde_json::from_str::<V>(&message)
                ) {
                    yield StreamRecord {
                        key,
                        value,
                        timestamp: 0, // Would come from message metadata
                        partition: 0,
                        offset: 0,
                    };
                }
            }
        };
        
        Ok(KStream::new(Box::new(Box::pin(stream))))
    }

    /// Create a producer for writing to topics
    pub async fn producer(&self) -> Result<Arc<Producer>, Box<dyn std::error::Error>> {
        let producer = Producer::new(&self.broker_address).await?;
        Ok(Arc::new(producer))
    }
}
