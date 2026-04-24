//! Explainer: generates human-readable commit messages and reports.
//!
//! This module is responsible for all text output — commit messages,
//! warnings, and explanations. The decision engine decides WHAT to do.
//! The explainer decides HOW to describe it.

use crate::semantic::EntityKind;
use crate::semantic::SemanticChangeType;
use crate::work_inference::WorkKind;
use crate::work_inference::WorkUnit;

/// Generates a commit message for a single work unit.
pub fn generate_message(unit: &WorkUnit) -> String {
    let names = entity_names(unit);
    let formatted = format_names(&names);

    match unit.kind {
        WorkKind::Feature => format!("feat: add {}", formatted),
        WorkKind::BugFix => format!("fix: update {}", formatted),
        WorkKind::Refactor => {
            let all_removed = unit
                .changes
                .iter()
                .all(|c| c.change_type == SemanticChangeType::Removed);
            let all_added_private = unit
                .changes
                .iter()
                .all(|c| c.change_type == SemanticChangeType::Added && !c.is_public);

            if all_removed {
                format!("refactor: remove {}", formatted)
            } else if all_added_private {
                format!("refactor: add internal {}", formatted)
            } else {
                format!("refactor: update {}", formatted)
            }
        }
        WorkKind::Test => format!("test: add tests for {}", formatted),
    }
}

/// Generates a commit message for linked test units.
pub fn generate_test_message(test_units: &[&WorkUnit]) -> String {
    let names: Vec<String> = test_units.iter().flat_map(|u| entity_names(u)).collect();
    let formatted = format_names(&names);
    format!("test: add {}", formatted)
}

/// Generates a combined message when all units go in one commit.
pub fn generate_combined_message(units: &[WorkUnit]) -> String {
    let primary = units
        .iter()
        .find(|u| u.kind != WorkKind::Test)
        .unwrap_or(&units[0]);

    let names = entity_names(primary);
    let formatted = format_names(&names);

    match primary.kind {
        WorkKind::Feature => {
            let has_tests = units.iter().any(|u| u.kind == WorkKind::Test);
            if has_tests {
                format!("feat: add {} with tests", formatted)
            } else {
                format!("feat: add {}", formatted)
            }
        }
        WorkKind::BugFix => format!("fix: update {}", formatted),
        WorkKind::Refactor => format!("refactor: update {}", formatted),
        WorkKind::Test => format!("test: add {}", formatted),
    }
}

/// Generates warnings based on work units.
pub fn generate_warnings(units: &[WorkUnit]) -> Vec<String> {
    let mut warnings = Vec::new();

    let test_related_ids: std::collections::HashSet<usize> = units
        .iter()
        .filter(|u| u.kind == WorkKind::Test)
        .filter_map(|u| u.related_to)
        .collect();

    for unit in units {
        // Untested features
        if unit.kind == WorkKind::Feature && !test_related_ids.contains(&unit.id) {
            let names = entity_names(unit);
            warnings.push(format!("feature {} has no tests", format_names(&names)));
        }

        // Public API changes and removals
        for change in &unit.changes {
            if change.is_public {
                match change.change_type {
                    SemanticChangeType::SignatureChanged => {
                        warnings.push(format!(
                            "public API changed: {} — push may break consumers",
                            change.entity.name
                        ));
                    }
                    SemanticChangeType::Removed => {
                        warnings.push(format!("public entity removed: {}", change.entity.name));
                    }
                    _ => {}
                }
            }
        }
    }

    warnings
}

/// Extracts entity names from a work unit, excluding imports and modules.
/// Includes file stem for disambiguation.
pub fn entity_names(unit: &WorkUnit) -> Vec<String> {
    unit.entities
        .iter()
        .filter(|e| e.kind != EntityKind::Import && e.kind != EntityKind::Module)
        .map(|e| {
            let file_stem = e.file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            format!("{}::{}", file_stem, e.name)
        })
        .collect()
}

/// Formats a list of names into a human-readable string.
///
/// 1 name: "foo"
/// 2 names: "foo and bar"
/// 3+ names: "foo, bar, and baz"
pub fn format_names(names: &[String]) -> String {
    match names.len() {
        0 => "changes".to_string(),
        1 => names[0].clone(),
        2 => format!("{} and {}", names[0], names[1]),
        _ => {
            let last = &names[names.len() - 1];
            let rest = &names[..names.len() - 1];
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}
