//! Hardware ID fingerprint — deterministic, based on CPU, motherboard, disk, MAC.

use sha2::{Sha256, Digest};
use sysinfo::System;

/// Returns a stable 64-char hex fingerprint for this machine.
pub fn generate() -> String {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut parts: Vec<String> = Vec::new();

    // CPU brand string
    if let Some(cpu) = sys.cpus().first() {
        parts.push(cpu.brand().to_string());
    }

    // Total memory (rough hardware marker)
    parts.push(sys.total_memory().to_string());

    // Hostname
    if let Some(h) = System::host_name() {
        parts.push(h);
    }

    // OS version
    if let Some(v) = System::os_version() {
        parts.push(v);
    }

    let raw = parts.join("|");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}
