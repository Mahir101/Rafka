use rafka_core::proto::rafka::{
    broker_service_client::BrokerServiceClient,
    PublishRequest, PublishResponse,
};
use tonic::Request;
use uuid::Uuid;

pub struct Producer {
    client: BrokerServiceClient<tonic::transport::Channel>,
    producer_id: String,
}

impl Producer {
    pub async fn new(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = BrokerServiceClient::connect(format!("http://{}", addr)).await?;
        let producer_id = Uuid::new_v4().to_string();

        println!("Producer registered with ID: {}", producer_id);

        Ok(Self {
            client,
            producer_id,
        })
    }

    pub async fn publish(
        &mut self,
        topic: String,
        message: String,
        key: String,
    ) -> Result<PublishResponse, Box<dyn std::error::Error>> {
        let response = self.client
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

        Ok(result)
    }
}
