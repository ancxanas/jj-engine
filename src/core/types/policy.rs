use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    AutoCommittable,
    RequiresReview,
    Blocked { reason: String },
}
