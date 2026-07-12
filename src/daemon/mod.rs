pub mod lifecycle;
pub mod pipeline;
pub mod watcher;

use lifecycle::DaemonPaths;
use log::{debug, error, info};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use watcher::FileWatcher;

static STOP_FLAG: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigterm(_sig: i32) {
    eprintln!("DEBUG: Signal handler called!");
    STOP_FLAG.store(true, Ordering::Relaxed);
}

pub struct Daemon {
    project_root: std::path::PathBuf,
    is_running: Arc<AtomicBool>,
}

impl Daemon {
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        Self {
            project_root: project_root.to_path_buf(),
            is_running,
        }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        let paths = DaemonPaths::new(&self.project_root);

        // Register SIGTERM handler using sigaction for better reliability
        let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
        sa.sa_sigaction = handle_sigterm as *const () as usize;
        sa.sa_flags = libc::SA_SIGINFO;
        unsafe {
            libc::sigemptyset(&raw mut sa.sa_mask);
            libc::sigaction(libc::SIGTERM, &raw const sa, std::ptr::null_mut());
        }

        // Write PID file
        lifecycle::write_pid(&paths.pid_file, std::process::id())?;

        info!(
            "AVCS daemon started (PID {})",
            std::process::id()
        );

        // Initialize file watcher
        let config = crate::core::config::Config::load(&self.project_root)?;
        let watcher = FileWatcher::new(&config.daemon, &self.project_root)?;

        // Catch-up: run pipeline once on startup
        if !STOP_FLAG.load(Ordering::Relaxed) {
            debug!("Running catch-up analysis on startup");
            pipeline::run_pipeline(&self.project_root, &self.is_running);
        }

        // Main daemon loop
        info!("Daemon entering watch loop");
        loop {
            if STOP_FLAG.load(Ordering::Relaxed) {
                info!("Shutdown signal received, exiting");
                break;
            }

            match watcher.recv() {
                Ok(_changed_paths) => {
                    if STOP_FLAG.load(Ordering::Relaxed) {
                        break;
                    }
                    debug!("Running pipeline after file changes");
                    pipeline::run_pipeline(&self.project_root, &self.is_running);
                }
                Err(e) => {
                    error!("Watcher receive error: {e}");
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        // Cleanup
        self.is_running.store(false, Ordering::Relaxed);
        let _ = lifecycle::remove_pid(&paths.pid_file);
        info!("Daemon stopped");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_daemon_new() {
        let dir = tempdir().unwrap();
        let daemon = Daemon::new(dir.path());
        assert!(daemon.is_running.load(Ordering::Relaxed));
    }

    #[test]
    fn test_stop_flag_default() {
        STOP_FLAG.store(false, Ordering::Relaxed);
        assert!(!STOP_FLAG.load(Ordering::Relaxed));
    }
}
