use rafka_core::proto::rafka::{
    broker_service_client::BrokerServiceClient,
    ConsumeRequest, SubscribeRequest, AcknowledgeRequest, UpdateOffsetRequest,
    RegisterRequest, ClientType,
};
use tonic::Request;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct Consumer {
    client: BrokerServiceClient<tonic::transport::Channel>,
    consumer_id: String,
    _current_offset: i64,
}

impl Consumer {
    pub async fn new(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut client = BrokerServiceClient::connect(format!("http://{}", addr)).await?;
        let consumer_id = Uuid::new_v4().to_string();

        // Register consumer
        client
            .register(Request::new(RegisterRequest {
                client_id: consumer_id.clone(),
                client_type: ClientType::Consumer as i32,
            }))
            .await?;

        println!("Consumer registered with ID: {}", consumer_id);

        Ok(Self {
            client,
            consumer_id,
            _current_offset: 0,
        })
    }

    pub async fn subscribe(&mut self, topic: String) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .subscribe(Request::new(SubscribeRequest {
                consumer_id: self.consumer_id.clone(),
                topic,
            }))
            .await?;
        Ok(())
    }

    pub async fn consume(&mut self, topic: String) -> Result<mpsc::Receiver<String>, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel(100);
        let mut stream = self.client
            .consume(Request::new(ConsumeRequest {
                id: self.consumer_id.clone(),
                topic: topic.clone(),
            }))
            .await?
            .into_inner();

        let consumer_id = self.consumer_id.clone();
        let mut client = self.client.clone();

        tokio::spawn(async move {
            while let Ok(Some(message)) = stream.message().await {
                let _ = tx.send(message.payload).await;
                
                // Acknowledge message
                let _ = client
                    .acknowledge(Request::new(AcknowledgeRequest {
                        consumer_id: consumer_id.clone(),
                        topic: topic.clone(),
                        message_id: message.message_id,
                    }))
                    .await;

                // Update offset
                let _ = client
                    .update_offset(Request::new(UpdateOffsetRequest {
                        consumer_id: consumer_id.clone(),
                        topic: topic.clone(),
                        offset: message.offset,
                    }))
                    .await;
            }
        });

        Ok(rx)
    }
}
