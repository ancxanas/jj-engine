use anyhow::Context;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub language: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisConfig {
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutonomyConfig {
    pub auto_commit_patterns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitConfig {
    pub message_style: String,
    pub include_evidence_footer: bool,
    pub sign_commits: bool,
    pub max_subject_length: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputConfig {
    pub default_format: String,
    pub color: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaemonConfig {
    pub enabled: bool,
    pub debounce_ms: u64,
    pub watch_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub log_level: String,
    pub auto_commit: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: 2000,
            watch_patterns: vec![
                "src/**/*".to_string(),
                "*.toml".to_string(),
                "*.md".to_string(),
            ],
            exclude_patterns: vec!["target/**".to_string(), "node_modules/**".to_string()],
            log_level: "info".to_string(),
            auto_commit: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    pub analysis: AnalysisConfig,
    pub autonomy: AutonomyConfig,
    pub commit: CommitConfig,
    pub output: OutputConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project: ProjectConfig {
                name: String::new(),
                language: "typescript".to_string(),
            },
            analysis: AnalysisConfig {
                include_patterns: vec!["src/**/*".to_string(), "*.md".to_string()],
                exclude_patterns: vec![
                    "node_modules/**".to_string(),
                    "dist/**".to_string(),
                    ".next/**".to_string(),
                    "coverage/**".to_string(),
                ],
            },
            autonomy: AutonomyConfig {
                auto_commit_patterns: vec!["Documentation".to_string()],
            },
            commit: CommitConfig {
                message_style: "conventional".to_string(),
                include_evidence_footer: true,
                sign_commits: false,
                max_subject_length: 72,
            },
            output: OutputConfig {
                default_format: "human".to_string(),
                color: true,
            },
            daemon: DaemonConfig::default(),
        }
    }
}

impl Config {
    pub fn load(project_root: &Path) -> anyhow::Result<Self> {
        let config_path = project_root.join(".autonomous-vcs").join("config.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(config_path).context("failed to read config file")?;
        let config: Self = toml::from_str(&content).context("failed to parse config TOML")?;

        Ok(config)
    }
}
