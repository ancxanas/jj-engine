use log::info;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DaemonPaths {
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
    pub run_dir: PathBuf,
}

impl DaemonPaths {
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        let run_dir = project_root.join(".autonomous-vcs");
        Self {
            pid_file: run_dir.join("daemon.pid"),
            log_file: run_dir.join("daemon.log"),
            run_dir,
        }
    }
}

pub fn write_pid(pid_file: &Path, pid: u32) -> anyhow::Result<()> {
    fs::write(pid_file, pid.to_string())?;
    info!("PID {pid} written to {}", pid_file.display());
    Ok(())
}

pub fn read_pid(pid_file: &Path) -> anyhow::Result<Option<u32>> {
    if !pid_file.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(pid_file)?;
    let pid: u32 = content.trim().parse()?;
    Ok(Some(pid))
}

pub fn remove_pid(pid_file: &Path) -> anyhow::Result<()> {
    if pid_file.exists() {
        fs::remove_file(pid_file)?;
        info!("PID file removed: {}", pid_file.display());
    }
    Ok(())
}

#[must_use]
pub fn is_running(pid: u32) -> bool {
    // Pids above i32::MAX are invalid on all supported platforms.
    // Also avoids the POSIX special case where kill(-1, 0) signals all processes.
    let Ok(pid_i32) = i32::try_from(pid) else {
        return false;
    };
    if pid_i32 <= 0 {
        return false;
    }
    unsafe { libc::kill(pid_i32, 0) == 0 }
}

pub fn stop_daemon(project_root: &Path) -> anyhow::Result<()> {
    let paths = DaemonPaths::new(project_root);
    if let Some(pid) = read_pid(&paths.pid_file)? {
        if is_running(pid) {
            if let Ok(pid_i32) = i32::try_from(pid) {
                unsafe { libc::kill(pid_i32, libc::SIGTERM) };
            }
            info!("Sent SIGTERM to daemon (PID {pid})");
            // Wait briefly for graceful shutdown
            for _ in 0..50 {
                if !is_running(pid) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if is_running(pid) {
                anyhow::bail!("Daemon (PID {pid}) did not stop within 5s");
            }
        } else {
            info!("Daemon (PID {pid}) is not running, cleaning up stale PID file");
        }
        remove_pid(&paths.pid_file)?;
    } else {
        info!("No daemon running (no PID file found)");
    }
    Ok(())
}

pub fn daemon_status(project_root: &Path) -> anyhow::Result<Option<u32>> {
    let paths = DaemonPaths::new(project_root);
    match read_pid(&paths.pid_file)? {
        Some(pid) => {
            if is_running(pid) {
                Ok(Some(pid))
            } else {
                remove_pid(&paths.pid_file)?;
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_write_and_read_pid() {
        let dir = tempdir().unwrap();
        let pid_file = dir.path().join("test.pid");

        write_pid(&pid_file, 12345).unwrap();
        let pid = read_pid(&pid_file).unwrap();
        assert_eq!(pid, Some(12345));
    }

    #[test]
    fn test_read_pid_nonexistent() {
        let dir = tempdir().unwrap();
        let pid_file = dir.path().join("nonexistent.pid");
        let pid = read_pid(&pid_file).unwrap();
        assert_eq!(pid, None);
    }

    #[test]
    fn test_remove_pid() {
        let dir = tempdir().unwrap();
        let pid_file = dir.path().join("test.pid");

        write_pid(&pid_file, 12345).unwrap();
        assert!(pid_file.exists());

        remove_pid(&pid_file).unwrap();
        assert!(!pid_file.exists());
    }

    #[test]
    fn test_is_running_current_process() {
        let pid = std::process::id();
        assert!(is_running(pid));
    }

    #[test]
    fn test_is_running_invalid_pid() {
        // Use a small pid that almost certainly doesn't exist
        // (u32::MAX wraps to -1 on cast_signed, which POSIX treats as "all processes")
        assert!(!is_running(999_999));
    }

    #[test]
    fn test_daemon_paths() {
        let dir = tempdir().unwrap();
        let paths = DaemonPaths::new(dir.path());
        assert_eq!(paths.pid_file, dir.path().join(".autonomous-vcs/daemon.pid"));
        assert_eq!(paths.log_file, dir.path().join(".autonomous-vcs/daemon.log"));
    }

    #[test]
    fn test_stop_daemon_no_pid_file() {
        let dir = tempdir().unwrap();
        stop_daemon(dir.path()).unwrap();
    }
}
