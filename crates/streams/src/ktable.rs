// KTable - changelog stream representation
// Placeholder for future implementation

use serde::{Serialize, de::DeserializeOwned};

pub struct KTable<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> KTable<K, V>
where
    K: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}
