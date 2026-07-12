use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChangePattern {
    BugFix,
    Refactor,
    Feature,
    Performance,
    Security,
    DependencyUpdate,
    TestAddition,
    Configuration,
    Documentation,
    BreakingChange,
    DeadCodeRemoval,
    Unknown,
}

impl ChangePattern {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "BugFix" => Some(Self::BugFix),
            "Refactor" => Some(Self::Refactor),
            "Feature" => Some(Self::Feature),
            "Performance" => Some(Self::Performance),
            "Security" => Some(Self::Security),
            "DependencyUpdate" => Some(Self::DependencyUpdate),
            "TestAddition" => Some(Self::TestAddition),
            "Configuration" => Some(Self::Configuration),
            "Documentation" => Some(Self::Documentation),
            "BreakingChange" => Some(Self::BreakingChange),
            "DeadCodeRemoval" => Some(Self::DeadCodeRemoval),
            "Unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}
