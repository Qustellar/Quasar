use crate::Asset;
use qst_core::{AssetId, Handle};
use std::collections::HashMap;
use std::sync::Arc;

pub struct AssetStorage<T: Asset> {
    values: HashMap<AssetId<T>, Arc<T>>,
}

impl<T: Asset> Default for AssetStorage<T> {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}

impl<T: Asset> AssetStorage<T> {
    pub fn insert(&mut self, handle: Handle<T>, value: T) {
        self.values.insert(handle.id, Arc::new(value));
    }
    pub fn get(&self, handle: Handle<T>) -> Option<Arc<T>> {
        self.values.get(&handle.id).cloned()
    }
    pub fn remove(&mut self, handle: Handle<T>) -> Option<Arc<T>> {
        self.values.remove(&handle.id)
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
