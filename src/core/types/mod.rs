mod analysis;
mod evidence;
mod file;
mod impact;
mod intent;
mod pattern;
mod policy;
mod relationship;
mod structural;

pub use analysis::{AnalysisMeta, AnalysisResult, AnalysisStats};
pub use evidence::{Evidence, EvidenceKind, Location};
pub use file::{FileChange, FileChangeType};
pub use impact::{ImpactAssessment, ImpactLevel};
pub use intent::{AmbiguousChange, Intent, IntentStatus, UnclassifiedChange};
pub use pattern::ChangePattern;
pub use policy::PolicyDecision;
pub use relationship::{FileRelationship, RelationshipKind, RelationshipStrength};
pub use structural::{StructuralChange, StructuralChangeKind};
