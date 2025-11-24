use std::sync::Arc;
use tokio::sync::RwLock;
use futures::Stream;
use std::pin::Pin;
use async_trait::async_trait;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

/// A record in a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRecord<K, V> {
    pub key: K,
    pub value: V,
    pub timestamp: u64,
    pub partition: i32,
    pub offset: i64,
}

/// Stream processing trait
#[async_trait]
pub trait StreamProcessor: Send + Sync {
    type Input;
    type Output;
    
    async fn process(&self, input: Self::Input) -> Option<Self::Output>;
}

/// KStream - represents an unbounded stream of records
pub struct KStream<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    source: Arc<RwLock<Box<dyn Stream<Item = StreamRecord<K, V>> + Send + Unpin>>>,
}

impl<K, V> KStream<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    pub fn new(source: Box<dyn Stream<Item = StreamRecord<K, V>> + Send + Unpin>) -> Self {
        Self {
            source: Arc::new(RwLock::new(source)),
        }
    }

    /// Map operation - transform each record
    pub fn map<K2, V2, F>(self, mapper: F) -> KStream<K2, V2>
    where
        K2: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        V2: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        F: Fn(StreamRecord<K, V>) -> StreamRecord<K2, V2> + Send + Sync + 'static,
    {
        use futures::StreamExt;
        
        let source = self.source;
        let mapper = Arc::new(mapper);
        
        let new_stream = async_stream::stream! {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                yield mapper(record);
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }

    /// Filter operation - keep only records matching predicate
    pub fn filter<F>(self, predicate: F) -> KStream<K, V>
    where
        F: Fn(&StreamRecord<K, V>) -> bool + Send + Sync + 'static,
    {
        use futures::StreamExt;
        
        let source = self.source;
        let predicate = Arc::new(predicate);
        
        let new_stream = async_stream::stream! {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                if predicate(&record) {
                    yield record;
                }
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }

    /// FlatMap operation - transform each record into multiple records
    pub fn flat_map<K2, V2, F>(self, mapper: F) -> KStream<K2, V2>
    where
        K2: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        V2: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        F: Fn(StreamRecord<K, V>) -> Vec<StreamRecord<K2, V2>> + Send + Sync + 'static,
    {
        use futures::StreamExt;
        
        let source = self.source;
        let mapper = Arc::new(mapper);
        
        let new_stream = async_stream::stream! {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                for new_record in mapper(record) {
                    yield new_record;
                }
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }

    /// Group by key for aggregation
    pub fn group_by_key(self) -> GroupedStream<K, V> {
        GroupedStream::new(self)
    }

    /// Peek operation - perform side effect without modifying stream
    pub fn peek<F>(self, action: F) -> KStream<K, V>
    where
        F: Fn(&StreamRecord<K, V>) + Send + Sync + 'static,
    {
        use futures::StreamExt;
        
        let source = self.source;
        let action = Arc::new(action);
        
        let new_stream = async_stream::stream! {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                action(&record);
                yield record;
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }

    /// Branch operation - split stream into multiple streams
    pub fn branch<F>(self, predicates: Vec<F>) -> Vec<KStream<K, V>>
    where
        F: Fn(&StreamRecord<K, V>) -> bool + Send + Sync + 'static,
    {
        use tokio::sync::broadcast;
        use futures::StreamExt;
        
        let (tx, _) = broadcast::channel(1000);
        let source = self.source;
        
        // Spawn task to broadcast records
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                let _ = tx_clone.send(record);
            }
        });
        
        // Create a stream for each predicate
        predicates.into_iter().map(|predicate| {
            let mut rx = tx.subscribe();
            let predicate = Arc::new(predicate);
            
            let new_stream = async_stream::stream! {
                while let Ok(record) = rx.recv().await {
                    if predicate(&record) {
                        yield record;
                    }
                }
            };
            
            KStream::new(Box::new(Box::pin(new_stream)))
        }).collect()
    }

    /// Merge with another stream
    pub fn merge(self, other: KStream<K, V>) -> KStream<K, V> {
        use futures::StreamExt;
        
        let source1 = self.source;
        let source2 = other.source;
        
        let new_stream = async_stream::stream! {
            let mut s1 = source1.write().await;
            let mut s2 = source2.write().await;
            
            loop {
                tokio::select! {
                    Some(record) = s1.next() => yield record,
                    Some(record) = s2.next() => yield record,
                    else => break,
                }
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }

    /// Write to a topic (sink operation)
    pub async fn to(self, topic: String, producer: Arc<rafka_producer::Producer>) {
        use futures::StreamExt;
        
        let mut source = self.source.write().await;
        while let Some(record) = source.next().await {
            let key = serde_json::to_string(&record.key).unwrap_or_default();
            let value = serde_json::to_string(&record.value).unwrap_or_default();
            
            if let Err(e) = producer.publish(topic.clone(), value, key).await {
                eprintln!("Failed to publish to topic {}: {}", topic, e);
            }
        }
    }

    /// Consume and process each record
    pub async fn for_each<F>(self, action: F)
    where
        F: Fn(StreamRecord<K, V>) + Send + Sync,
    {
        use futures::StreamExt;
        
        let mut source = self.source.write().await;
        while let Some(record) = source.next().await {
            action(record);
        }
    }
}

/// Grouped stream for aggregations
pub struct GroupedStream<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    stream: KStream<K, V>,
}

impl<K, V> GroupedStream<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + std::hash::Hash + Eq + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    pub fn new(stream: KStream<K, V>) -> Self {
        Self { stream }
    }

    /// Count records by key
    pub fn count(self) -> KStream<K, u64> {
        use futures::StreamExt;
        use std::collections::HashMap;
        
        let source = self.stream.source;
        let counts = Arc::new(RwLock::new(HashMap::new()));
        
        let new_stream = async_stream::stream! {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                let mut counts = counts.write().await;
                let count = counts.entry(record.key.clone()).or_insert(0);
                *count += 1;
                
                yield StreamRecord {
                    key: record.key,
                    value: *count,
                    timestamp: record.timestamp,
                    partition: record.partition,
                    offset: record.offset,
                };
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }

    /// Aggregate records by key
    pub fn aggregate<A, F>(self, initializer: A, aggregator: F) -> KStream<K, A>
    where
        A: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        F: Fn(A, V) -> A + Send + Sync + 'static,
    {
        use futures::StreamExt;
        use std::collections::HashMap;
        
        let source = self.stream.source;
        let aggregates = Arc::new(RwLock::new(HashMap::new()));
        let aggregator = Arc::new(aggregator);
        
        let new_stream = async_stream::stream! {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                let mut aggs = aggregates.write().await;
                let agg = aggs.entry(record.key.clone())
                    .or_insert_with(|| initializer.clone());
                *agg = aggregator(agg.clone(), record.value);
                
                yield StreamRecord {
                    key: record.key,
                    value: agg.clone(),
                    timestamp: record.timestamp,
                    partition: record.partition,
                    offset: record.offset,
                };
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }

    /// Reduce records by key
    pub fn reduce<F>(self, reducer: F) -> KStream<K, V>
    where
        F: Fn(V, V) -> V + Send + Sync + 'static,
    {
        use futures::StreamExt;
        use std::collections::HashMap;
        
        let source = self.stream.source;
        let reductions: Arc<RwLock<HashMap<K, V>>> = Arc::new(RwLock::new(HashMap::new()));
        let reducer = Arc::new(reducer);
        
        let new_stream = async_stream::stream! {
            let mut source = source.write().await;
            while let Some(record) = source.next().await {
                let mut reds = reductions.write().await;
                let reduced = reds.entry(record.key.clone())
                    .and_modify(|v: &mut V| *v = reducer(v.clone(), record.value.clone()))
                    .or_insert(record.value.clone());
                
                yield StreamRecord {
                    key: record.key,
                    value: reduced.clone(),
                    timestamp: record.timestamp,
                    partition: record.partition,
                    offset: record.offset,
                };
            }
        };
        
        KStream::new(Box::new(Box::pin(new_stream)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn test_map() {
        let records = vec![
            StreamRecord { key: "a".to_string(), value: 1, timestamp: 0, partition: 0, offset: 0 },
            StreamRecord { key: "b".to_string(), value: 2, timestamp: 1, partition: 0, offset: 1 },
        ];
        
        let source = stream::iter(records);
        let kstream = KStream::new(Box::new(source));
        
        let mapped = kstream.map(|r| StreamRecord {
            key: r.key,
            value: r.value * 2,
            timestamp: r.timestamp,
            partition: r.partition,
            offset: r.offset,
        });
        
        // Test would continue with assertions
    }

    #[tokio::test]
    async fn test_filter() {
        let records = vec![
            StreamRecord { key: "a".to_string(), value: 1, timestamp: 0, partition: 0, offset: 0 },
            StreamRecord { key: "b".to_string(), value: 2, timestamp: 1, partition: 0, offset: 1 },
            StreamRecord { key: "c".to_string(), value: 3, timestamp: 2, partition: 0, offset: 2 },
        ];
        
        let source = stream::iter(records);
        let kstream = KStream::new(Box::new(source));
        
        let filtered = kstream.filter(|r| r.value > 1);
        
        // Test would continue with assertions
    }
}
