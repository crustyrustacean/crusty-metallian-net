// src/storage/in_memory.rs

use crate::storage::{StorageBackend, StorageError};
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

pub struct InMemoryStorageBackend {
    store: Mutex<HashMap<Uuid, Vec<u8>>>,
}

impl Default for InMemoryStorageBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryStorageBackend {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl StorageBackend for InMemoryStorageBackend {
    async fn delete(&self, id: Uuid) -> Result<(), StorageError> {
        let mut locked_store = self
            .store
            .lock()
            .map_err(|e| StorageError::Operation(anyhow::anyhow!("Mutex poisoned: {e}")))?;
        let _ = locked_store.remove(&id);
        Ok(())
    }

    async fn find(&self, id: Uuid) -> Result<Bytes, StorageError> {
        let locked_store = self
            .store
            .lock()
            .map_err(|e| StorageError::Operation(anyhow::anyhow!("Mutex poisoned: {e}")))?;
        match locked_store
            .get(&id)
            .ok_or_else(|| StorageError::NotFound("No value with that key.".to_string()))
        {
            Ok(b) => Ok(Bytes::copy_from_slice(b.as_slice())),
            Err(e) => Err(e),
        }
    }

    async fn save(&self, id: Uuid, bytes: Bytes) -> Result<(), StorageError> {
        let mut locked_store = self
            .store
            .lock()
            .map_err(|e| StorageError::Operation(anyhow::anyhow!("Mutex poisoned: {e}")))?;
        let _ = locked_store.insert(id, bytes.to_vec());
        Ok(())
    }
}
