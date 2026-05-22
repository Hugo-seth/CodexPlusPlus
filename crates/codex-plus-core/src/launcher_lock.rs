use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const PID_FILE: &str = "launcher.pid";
const TAKEOVER_TIMEOUT: Duration = Duration::from_secs(5);
const TAKEOVER_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Serialize, Deserialize)]
struct LauncherLock {
    pid: u32,
    executable: String,
    started_at_ms: u64,
}

pub fn pid_file_path() -> PathBuf {
    crate::paths::default_app_state_dir().join(PID_FILE)
}

pub fn write_lock() -> std::io::Result<()> {
    let pid = std::process::id();
    let executable = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let lock = LauncherLock {
        pid,
        executable,
        started_at_ms,
    };
    let path = pid_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string(&lock).map_err(std::io::Error::other)?;
    std::fs::write(&path, contents)
}

pub fn clear_lock() {
    let _ = std::fs::remove_file(pid_file_path());
}

/// Attempt to take over from a previously-running launcher.
/// Returns true if the guard port was released within the timeout.
pub fn try_takeover_guard_port() -> bool {
    let Ok(contents) = std::fs::read_to_string(pid_file_path()) else {
        return false;
    };
    let Ok(lock) = serde_json::from_str::<LauncherLock>(&contents) else {
        return false;
    };
    if lock.pid == std::process::id() {
        return false;
    }
    if !verify_pid_is_codex_launcher(lock.pid, &lock.executable) {
        return false;
    }
    if !terminate_pid(lock.pid) {
        return false;
    }
    wait_for_port_release(crate::ports::LAUNCHER_GUARD_PORT, TAKEOVER_TIMEOUT)
}

#[cfg(unix)]
fn verify_pid_is_codex_launcher(pid: u32, expected_path: &str) -> bool {
    let Ok(output) = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let comm = String::from_utf8_lossy(&output.stdout);
    let comm = comm.trim();
    if comm.is_empty() {
        return false;
    }
    let lower = comm.to_ascii_lowercase();
    lower.contains("codexplusplus")
        || lower.contains("codex-plus-plus")
        || lower.contains("codex-plus")
        || (!expected_path.is_empty() && expected_path.ends_with(comm))
}

#[cfg(unix)]
fn terminate_pid(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn verify_pid_is_codex_launcher(pid: u32, expected_path: &str) -> bool {
    let Some(path) = crate::windows_integration::query_process_image_path(pid) else {
        return false;
    };
    let path_str = path.to_string_lossy().to_ascii_lowercase();
    path_str.contains("codexplusplus")
        || path_str.contains("codex-plus-plus")
        || (!expected_path.is_empty() && path_str == expected_path.to_ascii_lowercase())
}

#[cfg(windows)]
fn terminate_pid(pid: u32) -> bool {
    crate::windows_integration::terminate_process(pid)
}

fn wait_for_port_release(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !crate::ports::can_connect_loopback_port(port) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(TAKEOVER_POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_lock_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("launcher.pid");
        let lock = LauncherLock {
            pid: 12345,
            executable: "/path/to/codex-plus-plus".to_string(),
            started_at_ms: 1_700_000_000_000,
        };
        std::fs::write(&path, serde_json::to_string(&lock).unwrap()).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let restored: LauncherLock = serde_json::from_str(&contents).unwrap();
        assert_eq!(restored.pid, 12345);
        assert_eq!(restored.executable, "/path/to/codex-plus-plus");
    }

    #[cfg(unix)]
    #[test]
    fn verify_rejects_non_codex_process() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        assert!(!verify_pid_is_codex_launcher(pid, "/path/to/launcher.pid"));
        let _ = child.kill();
        let _ = child.wait();
    }
}
