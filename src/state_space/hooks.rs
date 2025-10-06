use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use alloy::primitives::Address;
use tokio::sync::RwLock;
use tracing::error;

use crate::state_space::{SerializableStateSpace, StateSpace};

/// A hook that can be used to observe, and react to state changes.
pub type StateHook<T> = Arc<dyn Fn(&T) + Send + Sync + 'static>;

/// Registry for managing state change hooks.
#[derive(Clone)]
pub struct HookRegistry<T: Send + Sync + Clone + 'static> {
    inner: Arc<RwLock<HookRegistryInner<T>>>,
    next_id: Arc<AtomicU64>,
}

#[derive(Clone)]
struct HookRegistryInner<T: Send + Sync + Clone + 'static> {
    hooks: HashMap<usize, StateHook<T>>,
}

impl<T: Send + Sync + Clone + 'static> HookRegistry<T> {
    /// Creates a new hook registry.
    pub fn new(hooks: Vec<StateHook<T>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HookRegistryInner {
                hooks: hooks.into_iter().enumerate().collect(),
            })),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Registers a hook and returns a handle that unregisters on drop.
    pub async fn register(&self, hook: StateHook<T>) -> HookHandle<T> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.inner.write().await.hooks.insert(id as usize, hook);
        HookHandle {
            id,
            reg: self.clone(),
        }
    }

    /// Notifies all registered hooks of a state change.
    pub async fn notify(&self, state_change: &T) {
        let mut hooks: Vec<StateHook<T>> = {
            let guard = self.inner.read().await;
            guard.hooks.values().cloned().collect()
        };

        for hook in hooks.iter_mut() {
            (hook)(state_change);
        }
    }

    async fn unregister(&self, id: u64) {
        self.inner.write().await.hooks.remove(&(id as usize));
    }
}

/// RAII handle for a registered hook. Unregisters the hook on drop.
#[derive(Clone)]
pub struct HookHandle<T: Send + Sync + Clone + 'static> {
    id: u64,
    reg: HookRegistry<T>,
}

impl<T: Send + Sync + Clone + 'static> Drop for HookHandle<T> {
    fn drop(&mut self) {
        let id = self.id;
        let reg = self.reg.clone();
        tokio::spawn(async move {
            reg.unregister(id).await;
        });
    }
}

#[derive(Clone, Debug)]
pub struct SnapshotConfig {
    /// Interval between consecutive snapshots.
    pub interval: Duration,
    /// Directory to store snapshots.
    pub directory: PathBuf,
    /// Maximum number of snapshots to retain.
    pub max_snapshots: usize,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(60),
            directory: PathBuf::from("./snapshots"),
            max_snapshots: 5,
        }
    }
}

impl SnapshotConfig {
    /// Creates a new snapshot configuration.
    pub fn new(interval: Duration, directory: PathBuf, max_snapshots: usize) -> Self {
        Self {
            interval,
            directory,
            max_snapshots,
        }
    }

    pub async fn into_state_hook(self, state: Arc<RwLock<StateSpace>>) -> StateHook<Vec<Address>> {
        let interval = self.interval;
        let max_snapshots = self.max_snapshots;
        let directory = self.directory.clone();

        let start = std::time::Instant::now();

        let hook = move |_: &Vec<Address>| {
            if start.elapsed() < interval {
                return;
            }

            let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
            let filename = format!("snapshot_{}.json", timestamp);
            let path = directory.join(filename);

            std::fs::create_dir_all(&directory)
                .inspect_err(
                    |e| error!(target: "snapshot", "Failed to create snapshot directory: {}", e),
                )
                .ok();

            let state = futures::executor::block_on(state.read()).clone();
            let state: SerializableStateSpace = state.into();

            let Ok(file) = std::fs::File::create(&path) else {
                error!(target: "snapshot", "Failed to create snapshot file: {:?}", path);
                return;
            };

            let writer = std::io::BufWriter::new(file);
            let Ok(_) = serde_json::to_writer_pretty(writer, &state) else {
                error!(target: "snapshot", "Failed to write snapshot to file: {:?}", path);
                return;
            };

            let Ok(mut entries) = std::fs::read_dir(&directory)
                .map(|rd| rd.filter_map(Result::ok).collect::<Vec<_>>())
            else {
                error!(target: "snapshot", "Failed to read snapshot directory: {:?}", directory);
                return;
            };

            entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
            if entries.len() > max_snapshots {
                entries.drain(..entries.len() - max_snapshots).for_each(|entry| {
                    let Ok(_) = std::fs::remove_file(entry.path()) else {
                        error!(target: "snapshot", "Failed to remove old snapshot: {:?}", entry.path());
                        return;
                    };
                });
            }
        };

        Arc::new(hook)
    }
}
