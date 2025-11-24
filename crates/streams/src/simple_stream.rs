use rafka_consumer::Consumer;
use rafka_producer::Producer;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Simple stream processor for basic streaming operations
pub struct SimpleStream {
    broker_address: String,
}

impl SimpleStream {
    pub fn new(broker_address: impl Into<String>) -> Self {
        Self {
            broker_address: broker_address.into(),
        }
    }

    /// Process messages from a topic with a transformation function
    pub async fn process<F>(
        &self,
        input_topic: String,
        output_topic: String,
        processor: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(String) -> Option<String> + Send + Sync + 'static,
    {
        let mut consumer = Consumer::new(&self.broker_address, None).await?;
        consumer.subscribe(input_topic.clone()).await?;
        
        let mut producer = Producer::new(&self.broker_address).await?;
        let mut rx = consumer.consume(input_topic).await?;
        
        let processor = Arc::new(processor);
        
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Some(transformed) = processor(message.clone()) {
                    let key = format!("key-{}", chrono::Utc::now().timestamp());
                    if let Err(e) = producer.publish(output_topic.clone(), transformed, key).await {
                        eprintln!("Failed to publish transformed message: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Aggregate messages by key with a window
    pub async fn aggregate<F>(
        &self,
        topic: String,
        window_seconds: u64,
        aggregator: F,
    ) -> Result<mpsc::Receiver<(String, String)>, Box<dyn std::error::Error>>
    where
        F: Fn(Vec<String>) -> String + Send + Sync + 'static,
    {
        let mut consumer = Consumer::new(&self.broker_address, None).await?;
        consumer.subscribe(topic.clone()).await?;
        
        let mut rx = consumer.consume(topic).await?;
        let (tx, output_rx) = mpsc::channel(100);
        
        let aggregator = Arc::new(aggregator);
        
        tokio::spawn(async move {
            let mut buffer: Vec<String> = Vec::new();
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(window_seconds));
            
            loop {
                tokio::select! {
                    Some(message) = rx.recv() => {
                        buffer.push(message);
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            let result = aggregator(buffer.clone());
                            let key = format!("agg-{}", chrono::Utc::now().timestamp());
                            let _ = tx.send((key, result)).await;
                            buffer.clear();
                        }
                    }
                }
            }
        });
        
        Ok(output_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_stream() {
        // Basic test structure
        let stream = SimpleStream::new("127.0.0.1:50051");
        assert_eq!(stream.broker_address, "127.0.0.1:50051");
    }
}
