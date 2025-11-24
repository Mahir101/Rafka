use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde::{Serialize, de::DeserializeOwned};

/// State store trait
pub trait StateStore<K, V>: Send + Sync
where
    K: Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn put(&mut self, key: K, value: V);
    fn get(&self, key: &K) -> Option<V>;
    fn delete(&mut self, key: &K) -> Option<V>;
    fn all(&self) -> Vec<(K, V)>;
}

/// In-memory state store
pub struct InMemoryStore<K, V>
where
    K: Clone + Send + Sync + std::hash::Hash + Eq,
    V: Clone + Send + Sync,
{
    store: Arc<RwLock<HashMap<K, V>>>,
}

impl<K, V> InMemoryStore<K, V>
where
    K: Clone + Send + Sync + std::hash::Hash + Eq,
    V: Clone + Send + Sync,
{
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn put_async(&self, key: K, value: V) {
        let mut store = self.store.write().await;
        store.insert(key, value);
    }

    pub async fn get_async(&self, key: &K) -> Option<V> {
        let store = self.store.read().await;
        store.get(key).cloned()
    }

    pub async fn delete_async(&self, key: &K) -> Option<V> {
        let mut store = self.store.write().await;
        store.remove(key)
    }

    pub async fn all_async(&self) -> Vec<(K, V)> {
        let store = self.store.read().await;
        store.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

/// Persistent state store (backed by disk)
pub struct PersistentStore<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + std::hash::Hash + Eq,
    V: Clone + Send + Sync + Serialize + DeserializeOwned,
{
    memory_store: InMemoryStore<K, V>,
    path: std::path::PathBuf,
}

impl<K, V> PersistentStore<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + std::hash::Hash + Eq,
    V: Clone + Send + Sync + Serialize + DeserializeOwned,
{
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            memory_store: InMemoryStore::new(),
            path: path.into(),
        }
    }

    pub async fn put_async(&self, key: K, value: V) {
        self.memory_store.put_async(key, value).await;
        // TODO: Persist to disk
    }

    pub async fn get_async(&self, key: &K) -> Option<V> {
        self.memory_store.get_async(key).await
    }

    pub async fn flush(&self) -> Result<(), std::io::Error> {
        // TODO: Flush to disk
        Ok(())
    }
}
