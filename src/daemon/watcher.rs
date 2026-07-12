use crate::core::config::DaemonConfig;
use log::{debug, info, warn};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use notify::{RecommendedWatcher, RecursiveMode};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

pub struct FileWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
    receiver: mpsc::Receiver<DebounceEventResult>,
    watch_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
}

impl FileWatcher {
    pub fn new(config: &DaemonConfig, project_root: &Path) -> anyhow::Result<Self> {
        let (tx, receiver) = mpsc::channel();
        let timeout = Duration::from_millis(config.debounce_ms);

        let mut debouncer = new_debouncer(timeout, tx)
            .map_err(|e| anyhow::anyhow!("Failed to create debouncer: {e}"))?;

        for pattern in &config.watch_patterns {
            let glob_path = resolve_glob_pattern(project_root, pattern);
            if glob_path.exists() {
                debouncer
                    .watcher()
                    .watch(&glob_path, RecursiveMode::Recursive)
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to watch {}: {e}",
                            glob_path.display()
                        )
                    })?;
                debug!("Watching: {}", glob_path.display());
            } else {
                warn!(
                    "Watch path does not exist, skipping: {}",
                    glob_path.display()
                );
            }
        }

        info!(
            "File watcher started (debounce={}ms, watching {} patterns)",
            config.debounce_ms,
            config.watch_patterns.len()
        );

        Ok(Self {
            _debouncer: debouncer,
            receiver,
            watch_patterns: config.watch_patterns.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
        })
    }

    pub fn recv(&self) -> Result<Vec<PathBuf>, mpsc::RecvError> {
        let result = self.receiver.recv()?;
        match result {
            Ok(events) => {
                let paths: Vec<PathBuf> = events
                    .into_iter()
                    .filter(|e| !self.is_excluded(&e.path))
                    .map(|e| e.path)
                    .collect();
                if !paths.is_empty() {
                    debug!("Detected {} changed files", paths.len());
                }
                Ok(paths)
            }
            Err(e) => {
                warn!("Debouncer error: {e}");
                Ok(Vec::new())
            }
        }
    }

    pub fn try_recv(&self) -> Result<Vec<PathBuf>, mpsc::TryRecvError> {
        let result = self.receiver.try_recv()?;
        match result {
            Ok(events) => {
                let paths: Vec<PathBuf> = events
                    .into_iter()
                    .filter(|e| !self.is_excluded(&e.path))
                    .map(|e| e.path)
                    .collect();
                if !paths.is_empty() {
                    debug!("Detected {} changed files", paths.len());
                }
                Ok(paths)
            }
            Err(e) => {
                warn!("Debouncer error: {e}");
                Ok(Vec::new())
            }
        }
    }

    fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.exclude_patterns
            .iter()
            .any(|pattern| glob_match(pattern, &path_str))
    }

    #[must_use]
    pub fn watch_patterns(&self) -> &[String] {
        &self.watch_patterns
    }

    #[must_use]
    pub fn exclude_patterns(&self) -> &[String] {
        &self.exclude_patterns
    }
}

fn resolve_glob_pattern(base: &Path, pattern: &str) -> PathBuf {
    if pattern.starts_with('/') {
        PathBuf::from(pattern)
    } else if pattern.starts_with("**/") {
        // Patterns starting with **/ mean "anywhere under base"
        base.to_path_buf()
    } else if let Some(first_component) = pattern.split('/').next() {
        base.join(first_component)
    } else {
        base.to_path_buf()
    }
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();

    glob_match_parts(&pattern_parts, &path_parts)
}

fn glob_match_parts(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }

    if pattern[0] == "**" {
        // `**` matches zero or more path components
        if glob_match_parts(&pattern[1..], path) {
            return true;
        }
        if !path.is_empty() {
            return glob_match_parts(pattern, &path[1..]);
        }
        return false;
    }

    if path.is_empty() {
        return false;
    }

    if component_glob_match(pattern[0], path[0]) && glob_match_parts(&pattern[1..], &path[1..]) {
        return true;
    }

    // Try matching from the next path component (implicit leading **)
    glob_match_parts(pattern, &path[1..])
}

/// Match a single path component against a glob pattern component.
/// Handles `*` wildcards within the component (e.g., `*.rs` matches `main.rs`).
fn component_glob_match(pattern: &str, component: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == component;
    }

    let parts: Vec<&str> = pattern.splitn(2, '*').collect();
    let prefix = parts[0];
    let suffix = parts.get(1).copied().unwrap_or("");

    component.starts_with(prefix) && component.ends_with(suffix) && component.len() >= prefix.len() + suffix.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_simple() {
        assert!(glob_match("src/**/*.rs", "src/main.rs"));
        assert!(glob_match("src/**/*.rs", "src/lib/core.rs"));
        assert!(!glob_match("src/**/*.rs", "tests/main.rs"));
    }

    #[test]
    fn test_glob_match_doublestar() {
        assert!(glob_match("**/*.log", "src/debug.log"));
        assert!(glob_match("**/*.log", "deep/nested/path/file.log"));
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("Cargo.toml", "Cargo.toml"));
        assert!(!glob_match("Cargo.toml", "other.toml"));
    }

    #[test]
    fn test_resolve_glob_pattern() {
        let base = Path::new("/home/user/project");
        assert_eq!(
            resolve_glob_pattern(base, "src/**"),
            PathBuf::from("/home/user/project/src")
        );
        assert_eq!(
            resolve_glob_pattern(base, "/absolute/path"),
            PathBuf::from("/absolute/path")
        );
    }

    #[test]
    fn test_is_excluded() {
        let config = DaemonConfig {
            watch_patterns: vec!["src/**".to_string()],
            exclude_patterns: vec!["target/**".to_string(), "*.log".to_string()],
            ..Default::default()
        };
        let watcher = FileWatcher {
            _debouncer: new_debouncer(Duration::from_millis(100), |_: DebounceEventResult| {})
                .unwrap(),
            receiver: mpsc::channel().1,
            watch_patterns: config.watch_patterns.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
        };

        assert!(watcher.is_excluded(Path::new("src/target/debug/main.rs")));
        assert!(watcher.is_excluded(Path::new("debug.log")));
        assert!(!watcher.is_excluded(Path::new("src/main.rs")));
    }
}
