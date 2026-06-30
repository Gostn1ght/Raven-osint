use sha2::{Sha256, Digest};
use sysinfo::System;

pub fn get_hwid() -> String {
    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = System::host_name().unwrap_or_default();
    let os_version = System::os_version().unwrap_or_default();
    let cpu_count = sys.cpus().len().to_string();
    let total_mem = sys.total_memory().to_string();

    let raw = format!("{hostname}|{os_version}|{cpu_count}|{total_mem}");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}
