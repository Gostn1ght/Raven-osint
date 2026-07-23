// ═══════════════════════════════════════════════════════════════════════════════
// PERSISTENCE — lightweight JSON snapshot store.
// ═══════════════════════════════════════════════════════════════════════════════
// Replaces the previous (dead, non-compiling) sqlx layer. State is snapshotted to a
// single JSON file with an atomic write (temp file + rename), and loaded on boot.
// Sessions are intentionally *not* persisted — they are ephemeral by design.

use serde::{de::DeserializeOwned, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// Opens (or prepares) a store at `path`, creating parent dirs as needed.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self { path }
    }

    /// Loads and deserializes the snapshot, or `None` if absent/unreadable.
    pub fn load<T: DeserializeOwned>(&self) -> Option<T> {
        let data = std::fs::read(&self.path).ok()?;
        match serde_json::from_slice(&data) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::error!("store: failed to parse {}: {}", self.path.display(), e);
                None
            }
        }
    }

    /// Atomically writes the snapshot (temp file + rename) so a crash mid-write
    /// never corrupts the live data file.
    pub fn save<T: Serialize>(&self, value: &T) {
        let json = match serde_json::to_vec_pretty(value) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("store: serialize failed: {}", e);
                return;
            }
        };
        let tmp = self.path.with_extension("json.tmp");
        let write_result = (|| -> std::io::Result<()> {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.sync_all()?;
            std::fs::rename(&tmp, &self.path)
        })();
        if let Err(e) = write_result {
            tracing::error!("store: failed to persist {}: {}", self.path.display(), e);
            let _ = std::fs::remove_file(&tmp);
        }
    }
}
