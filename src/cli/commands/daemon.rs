
use std::path::Path;

use crate::daemon;
use crate::daemon::lifecycle;

pub fn run(subcommand: &str) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?;

    match subcommand {
        "start" => start_daemon(&project_root),
        "stop" => {
            stop_daemon(&project_root);
            Ok(())
        }
        "status" => status_daemon(&project_root),
        _ => {
            anyhow::bail!("Unknown daemon subcommand: {subcommand}");
        }
    }
}

fn start_daemon(project_root: &Path) -> anyhow::Result<()> {
    let paths = lifecycle::DaemonPaths::new(project_root);

    // Check if already running
    if let Some(pid) = lifecycle::read_pid(&paths.pid_file)? {
        if lifecycle::is_running(pid) {
            println!("Daemon already running (PID {pid})");
            return Ok(());
        }
        // Stale PID file
        lifecycle::remove_pid(&paths.pid_file)?;
    }

    // Fork to background: the child becomes the daemon, the parent prints a message and exits
    let pid = unsafe { libc::fork() };

    match pid.cmp(&0) {
        std::cmp::Ordering::Less => {
            anyhow::bail!("Failed to fork process");
        }
        std::cmp::Ordering::Greater => {
            // Parent process: print message and exit
            println!("AVCS daemon started in background (PID {pid})");
            std::process::exit(0);
        }
        std::cmp::Ordering::Equal => {
            // Child process: create new session, close stdio, run daemon
            unsafe {
                libc::setsid();
            }

            // Close standard file descriptors so the daemon doesn't hold the terminal
            unsafe {
                libc::close(libc::STDIN_FILENO);
                libc::close(libc::STDOUT_FILENO);
                libc::close(libc::STDERR_FILENO);
            }

            // Write PID file FIRST - before anything else that might fail
            let paths = lifecycle::DaemonPaths::new(project_root);
            if let Err(e) = lifecycle::write_pid(&paths.pid_file, std::process::id()) {
                // Can't log yet, write to a temp file for debugging
                let _ = std::fs::write("/tmp/avcs_daemon_error.txt", format!("Failed to write PID file: {e:?}"));
                std::process::exit(1);
            }

            // Ensure log directory exists
            if let Some(dir) = paths.log_file.parent() {
                if let Err(e) = std::fs::create_dir_all(dir) {
                    let _ = std::fs::write("/tmp/avcs_daemon_error.txt", format!("Failed to create log dir: {e:?}"));
                    std::process::exit(1);
                }
            }

            // Load config for log level
            let config = match crate::core::config::Config::load(project_root) {
                Ok(c) => c,
                Err(e) => {
                    let _ = std::fs::write("/tmp/avcs_daemon_error.txt", format!("Failed to load config: {e:?}"));
                    std::process::exit(1);
                }
            };
            let log_level = config.daemon.log_level;

            // Ensure log directory exists
            if let Some(dir) = paths.log_file.parent() {
                if let Err(e) = std::fs::create_dir_all(dir) {
                    let _ = std::fs::write("/tmp/avcs_daemon_error.txt", format!("Failed to create log dir 2: {e:?}"));
                    std::process::exit(1);
                }
            }

            // Initialize logger with file output and rotation (max 5MB, keep 3 files)
            let log_dir = paths.log_file.parent().unwrap_or(project_root);
            eprintln!("DEBUG: Initializing logger with log_dir: {}, log_level: {log_level}", log_dir.display());
            if let Err(e) = flexi_logger::Logger::try_with_str(&log_level)?
                .log_to_file(
                    flexi_logger::FileSpec::default()
                        .directory(log_dir)
                        .basename("daemon")
                        .suffix("log"),
                )
                .rotate(
                    flexi_logger::Criterion::Size(5_000_000), // 5MB
                    flexi_logger::Naming::Numbers,
                    flexi_logger::Cleanup::KeepLogFiles(3),
                )
                .format(flexi_logger::default_format)
                .start()
            {
                let _ = std::fs::write("/tmp/avcs_daemon_error.txt", format!("Failed to start logger: {e:?}"));
                std::process::exit(1);
            }
            eprintln!("DEBUG: Logger started successfully");

            eprintln!("DEBUG: Logger started successfully");

            // Set up the daemon and run
            eprintln!("DEBUG: Creating daemon instance");
            let daemon = daemon::Daemon::new(project_root);
            eprintln!("DEBUG: Starting daemon run loop");
            if let Err(e) = daemon.run() {
                log::error!("Daemon error: {e}");
                let _ = lifecycle::remove_pid(&paths.pid_file);
                std::process::exit(1);
            }

            std::process::exit(0);
        }
    }
}

fn stop_daemon(project_root: &Path) {
    match lifecycle::stop_daemon(project_root) {
        Ok(()) => {
            println!("AVCS daemon stopped");
        }
        Err(e) => {
            println!("Failed to stop daemon: {e}");
        }
    }
}

fn status_daemon(project_root: &Path) -> anyhow::Result<()> {
    lifecycle::daemon_status(project_root)?.map_or_else(
        || {
            println!("AVCS daemon is not running");
            Ok(())
        },
        |pid| {
            println!("AVCS daemon is running (PID {pid})");
            Ok(())
        },
    )
}
