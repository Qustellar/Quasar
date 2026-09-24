use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::AssetStorage;
use qst_core::{AssetId, EngineError, EngineResult, Handle};
use serde::{Deserialize, Serialize};

pub trait Asset: Send + Sync + 'static {}
impl<T: Send + Sync + 'static> Asset for T {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LoadState {
    Unloaded,
    Loading,
    Loaded,
    Failed,
}

#[derive(Clone, Debug)]
pub struct AssetRecord {
    pub path: PathBuf,
    pub state: LoadState,
    pub generation: u32,
    pub type_id: TypeId,
}

pub struct AssetServer {
    next_index: u32,
    records: HashMap<u32, AssetRecord>,
    paths: HashMap<(PathBuf, TypeId), u32>,
    bytes: AssetStorage<BinaryAsset>,
    result_tx: std::sync::mpsc::Sender<(AssetId<BinaryAsset>, EngineResult<BinaryAsset>)>,
    result_rx: std::sync::mpsc::Receiver<(AssetId<BinaryAsset>, EngineResult<BinaryAsset>)>,
    watcher: Option<notify::RecommendedWatcher>,
    watch_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    watched_dirs: HashSet<PathBuf>,
    in_flight: HashSet<(u32, u32)>,
}

#[derive(Clone, Debug)]
pub struct BinaryAsset(pub Vec<u8>);

impl Default for AssetServer {
    fn default() -> Self {
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        Self {
            next_index: 0,
            records: HashMap::new(),
            paths: HashMap::new(),
            bytes: AssetStorage::default(),
            result_tx,
            result_rx,
            watcher: None,
            watch_rx: None,
            watched_dirs: HashSet::new(),
            in_flight: HashSet::new(),
        }
    }
}

impl AssetServer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load<T: Asset>(&mut self, path: impl AsRef<Path>) -> Handle<T> {
        let path = path.as_ref().to_path_buf();
        let key = (path.clone(), TypeId::of::<T>());
        if let Some(index) = self.paths.get(&key) {
            let record = &self.records[index];
            return Handle::new(AssetId::new(*index, record.generation));
        }
        let index = self.next_index;
        self.next_index = self.next_index.saturating_add(1);
        self.paths.insert(key, index);
        self.records.insert(
            index,
            AssetRecord {
                path,
                state: LoadState::Loading,
                generation: 0,
                type_id: TypeId::of::<T>(),
            },
        );
        Handle::new(AssetId::new(index, 0))
    }

    pub fn state<T: 'static>(&self, handle: Handle<T>) -> Option<LoadState> {
        self.records.get(&handle.id.index).and_then(|record| {
            (record.generation == handle.id.generation && record.type_id == TypeId::of::<T>())
                .then_some(record.state)
        })
    }

    pub fn mark_loaded<T: 'static>(&mut self, handle: Handle<T>) -> EngineResult<()> {
        let record = self
            .records
            .get_mut(&handle.id.index)
            .ok_or_else(|| EngineError::AssetNotFound(format!("{}", handle.id.index)))?;
        if record.generation != handle.id.generation || record.type_id != TypeId::of::<T>() {
            return Err(EngineError::AssetNotFound(format!("{}", handle.id.index)));
        }
        record.state = LoadState::Loaded;
        Ok(())
    }

    pub fn mark_failed<T: 'static>(&mut self, handle: Handle<T>) -> EngineResult<()> {
        let record = self
            .records
            .get_mut(&handle.id.index)
            .ok_or_else(|| EngineError::AssetNotFound(format!("{}", handle.id.index)))?;
        if record.generation != handle.id.generation || record.type_id != TypeId::of::<T>() {
            return Err(EngineError::AssetNotFound(format!("{}", handle.id.index)));
        }
        record.state = LoadState::Failed;
        Ok(())
    }

    pub fn reload_path(&mut self, path: impl AsRef<Path>) -> bool {
        let indices: Vec<u32> = self
            .paths
            .iter()
            .filter_map(|((registered_path, _), index)| {
                (registered_path == path.as_ref()).then_some(*index)
            })
            .collect();
        for index in &indices {
            let Some(record) = self.records.get_mut(index) else {
                continue;
            };
            if record.type_id == TypeId::of::<BinaryAsset>() {
                self.bytes
                    .remove(Handle::new(AssetId::new(*index, record.generation)));
            }
            record.generation = record.generation.saturating_add(1);
            record.state = LoadState::Loading;
        }
        !indices.is_empty()
    }

    pub fn invalidate<T: 'static>(&mut self, path: impl AsRef<Path>) -> bool {
        let Some(index) = self
            .paths
            .get(&(path.as_ref().to_path_buf(), TypeId::of::<T>()))
            .copied()
        else {
            return false;
        };
        let Some(record) = self.records.get_mut(&index) else {
            return false;
        };
        if record.type_id == TypeId::of::<BinaryAsset>() {
            self.bytes
                .remove(Handle::new(AssetId::new(index, record.generation)));
        }
        record.generation = record.generation.saturating_add(1);
        record.state = LoadState::Loading;
        true
    }

    pub fn records(&self) -> impl Iterator<Item = &AssetRecord> {
        self.records.values()
    }

    pub fn load_bytes_async(&mut self, path: impl AsRef<Path>) -> Handle<BinaryAsset> {
        let path = path.as_ref().to_path_buf();
        let handle = self.load::<BinaryAsset>(&path);
        if self.state(handle) == Some(LoadState::Loading)
            && self
                .in_flight
                .insert((handle.id.index, handle.id.generation))
        {
            let tx = self.result_tx.clone();
            std::thread::spawn(move || {
                let result = std::fs::read(path)
                    .map(BinaryAsset)
                    .map_err(EngineError::from);
                let _ = tx.send((handle.id, result));
            });
        }
        handle
    }

    pub fn poll_async(&mut self) {
        while let Ok((id, result)) = self.result_rx.try_recv() {
            self.in_flight.remove(&(id.index, id.generation));
            if self.state(Handle::new(id)).is_none() {
                continue;
            }
            match result {
                Ok(bytes) => {
                    self.bytes.insert(Handle::new(id), bytes);
                    let _ = self.mark_loaded(Handle::new(id));
                }
                Err(error) => {
                    tracing::warn!(%error, "asset load failed");
                    let _ = self.mark_failed(Handle::new(id));
                }
            }
        }
    }

    pub fn bytes(&self, handle: Handle<BinaryAsset>) -> Option<Arc<BinaryAsset>> {
        if self.state(handle) != Some(LoadState::Loaded) {
            return None;
        }
        self.bytes.get(handle)
    }

    pub fn watch_path(&mut self, path: impl AsRef<Path>) -> EngineResult<()> {
        use notify::Watcher;
        if self.watcher.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.watcher = Some(
                notify::recommended_watcher(move |event| {
                    let _ = tx.send(event);
                })
                .map_err(|error| EngineError::Runtime(error.to_string()))?,
            );
            self.watch_rx = Some(rx);
        }
        let parent = path
            .as_ref()
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if self.watched_dirs.contains(parent) {
            return Ok(());
        }
        self.watcher
            .as_mut()
            .unwrap()
            .watch(parent, notify::RecursiveMode::NonRecursive)
            .map_err(|error| EngineError::Runtime(error.to_string()))?;
        self.watched_dirs.insert(parent.to_path_buf());
        Ok(())
    }

    pub fn poll_changed_paths(&mut self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        if let Some(rx) = &self.watch_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    Ok(event)
                        if matches!(
                            event.kind,
                            notify::EventKind::Create(_)
                                | notify::EventKind::Modify(_)
                                | notify::EventKind::Remove(_)
                        ) =>
                    {
                        changed.extend(event.paths);
                    }
                    Err(error) => tracing::warn!(%error, "asset watcher failed"),
                    _ => {}
                }
            }
        }
        changed.sort();
        changed.dedup();
        changed
    }
}

#[cfg(test)]
mod tests;
