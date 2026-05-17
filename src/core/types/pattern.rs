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
