//! Decision engine: determines next JJ actions.
//!
//! Takes RepoState and WorkUnits, produces an ActionPlan
//! describing what JJ operations should be performed.
//! This module is pure logic — no jj-lib calls.

use crate::repo_inspector::RepoState;
use crate::semantic::SemanticChangeType;
use crate::work_inference::WorkKind;
use crate::work_inference::WorkUnit;

/// A specific JJ action the engine wants to perform.
#[derive(Debug, Clone)]
pub enum JjAction {
    /// Nothing to do.
    NoOp,

    /// Amend the current working copy commit.
    AmendCommit { message: String },

    /// Create a new commit on top of the current one.
    CreateCommit { message: String },

    /// Split current work into multiple commits.
    SplitCommit { plans: Vec<CommitPlan> },

    /// Only possible when conflicts exist.
    /// This is the absolute last resort.
    RequireHumanIntervention { reason: String },
}

/// A plan for a single commit within a split.
#[derive(Debug, Clone)]
pub struct CommitPlan {
    /// Which work unit ids go into this commit.
    pub work_unit_ids: Vec<usize>,

    /// The commit message.
    pub message: String,

    /// Ordering priority (lower = committed first).
    pub order: usize,
}

/// The full action plan produced by the decision engine.
#[derive(Debug, Clone)]
pub struct ActionPlan {
    /// The top-level action.
    pub action: JjAction,

    /// Non-blocking warnings.
    pub warnings: Vec<String>,
}

/// Main decision function.
///
/// Takes repository state and classified work units.
/// Returns an action plan describing what to do.
pub fn decide(repo_state: &RepoState, work_units: &[WorkUnit]) -> ActionPlan {
    let mut warnings: Vec<String> = Vec::new();

    // Step 1: Check blockers
    if repo_state.has_conflicts {
        return ActionPlan {
            action: JjAction::RequireHumanIntervention {
                reason: "conflicts must be resolved before engine can proceed".to_string(),
            },
            warnings,
        };
    }

    // Step 2: No changes
    if work_units.is_empty() {
        return ActionPlan {
            action: JjAction::NoOp,
            warnings,
        };
    }

    // Generate warnings
    warnings = generate_warnings(work_units);

    // Step 3: Determine if we can amend or must create
    let can_amend = repo_state.is_safe_to_rewrite;

    // Step 4: Single work unit (or single impl + its test)
    if is_single_logical_unit(work_units) {
        let message = generate_combined_message(work_units);

        let action = if can_amend {
            JjAction::AmendCommit { message }
        } else {
            JjAction::CreateCommit { message }
        };

        return ActionPlan { action, warnings };
    }

    // Step 5: Multiple work units — split
    let mut plans = build_commit_plans(work_units);

    // Step 6: Merge small same-kind refactors
    plans = merge_small_refactors(plans, work_units);

    // Step 7: Sort by order
    plans.sort_by_key(|p| p.order);

    let action = if plans.len() == 1 {
        // After merging, only one plan remains
        let plan = &plans[0];
        if can_amend {
            JjAction::AmendCommit {
                message: plan.message.clone(),
            }
        } else {
            JjAction::CreateCommit {
                message: plan.message.clone(),
            }
        }
    } else {
        JjAction::SplitCommit { plans }
    };

    ActionPlan { action, warnings }
}

/// Checks if work units represent a single logical unit.
///
/// True if:
/// - only one work unit
/// - one impl unit + one test unit linked to it
fn is_single_logical_unit(units: &[WorkUnit]) -> bool {
    if units.len() == 1 {
        return true;
    }

    if units.len() == 2 {
        let has_impl = units.iter().any(|u| u.kind != WorkKind::Test);
        let has_test = units.iter().any(|u| u.kind == WorkKind::Test);
        let test_linked = units
            .iter()
            .filter(|u| u.kind == WorkKind::Test)
            .all(|u| u.related_to.is_some());

        return has_impl && has_test && test_linked;
    }

    false
}

/// Builds one CommitPlan per work unit with proper ordering.
fn build_commit_plans(units: &[WorkUnit]) -> Vec<CommitPlan> {
    let mut plans: Vec<CommitPlan> = Vec::new();

    // Group: gather test units that should follow their impl unit
    let mut test_map: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();

    for unit in units {
        if unit.kind == WorkKind::Test {
            if let Some(related_id) = unit.related_to {
                test_map.entry(related_id).or_default().push(unit.id);
            }
        }
    }

    let mut order: usize = 0;

    // Refactors first
    for unit in units {
        if unit.kind == WorkKind::Refactor {
            plans.push(CommitPlan {
                work_unit_ids: vec![unit.id],
                message: generate_message(unit),
                order,
            });
            order += 1;
        }
    }

    // Features next, each followed by its tests
    for unit in units {
        if unit.kind == WorkKind::Feature {
            plans.push(CommitPlan {
                work_unit_ids: vec![unit.id],
                message: generate_message(unit),
                order,
            });
            order += 1;

            // Add linked tests immediately after
            if let Some(test_ids) = test_map.get(&unit.id) {
                let test_units: Vec<&WorkUnit> =
                    units.iter().filter(|u| test_ids.contains(&u.id)).collect();

                if !test_units.is_empty() {
                    let test_message = generate_test_message(&test_units);
                    plans.push(CommitPlan {
                        work_unit_ids: test_ids.clone(),
                        message: test_message,
                        order,
                    });
                    order += 1;
                }
            }
        }
    }

    // BugFixes
    for unit in units {
        if unit.kind == WorkKind::BugFix {
            plans.push(CommitPlan {
                work_unit_ids: vec![unit.id],
                message: generate_message(unit),
                order,
            });
            order += 1;
        }
    }

    // Unlinked tests (tests with no related impl unit)
    for unit in units {
        if unit.kind == WorkKind::Test && unit.related_to.is_none() {
            plans.push(CommitPlan {
                work_unit_ids: vec![unit.id],
                message: generate_message(unit),
                order,
            });
            order += 1;
        }
    }

    plans
}

/// Merges small Refactor units into one commit.
///
/// If multiple refactor plans exist and each has few entities,
/// merge them into a single plan.
fn merge_small_refactors(plans: Vec<CommitPlan>, units: &[WorkUnit]) -> Vec<CommitPlan> {
    let refactor_plans: Vec<&CommitPlan> = plans
        .iter()
        .filter(|p| {
            p.work_unit_ids.iter().all(|id| {
                units
                    .iter()
                    .find(|u| u.id == *id)
                    .map(|u| u.kind == WorkKind::Refactor)
                    .unwrap_or(false)
            })
        })
        .collect();

    // Only merge if there are multiple small refactors
    if refactor_plans.len() <= 1 {
        return plans;
    }

    // Check if all refactor plans are small (3 or fewer entities)
    let total_refactor_entities: usize = refactor_plans
        .iter()
        .flat_map(|p| &p.work_unit_ids)
        .filter_map(|id| units.iter().find(|u| u.id == *id))
        .map(|u| u.entities.len())
        .sum();

    if total_refactor_entities > 6 {
        // Too many entities to merge. Keep separate.
        return plans;
    }

    // Merge all refactor plans into one
    let mut merged_ids: Vec<usize> = Vec::new();
    let mut merged_names: Vec<String> = Vec::new();
    let min_order = refactor_plans.iter().map(|p| p.order).min().unwrap_or(0);

    for plan in &refactor_plans {
        for id in &plan.work_unit_ids {
            merged_ids.push(*id);
            if let Some(unit) = units.iter().find(|u| u.id == *id) {
                for entity in &unit.entities {
                    merged_names.push(entity.name.clone());
                }
            }
        }
    }

    // Determine refactor action word
    let all_removed = merged_ids.iter().all(|id| {
        units
            .iter()
            .find(|u| u.id == *id)
            .map(|u| {
                u.changes
                    .iter()
                    .all(|c| c.change_type == SemanticChangeType::Removed)
            })
            .unwrap_or(false)
    });

    let action_word = if all_removed { "remove" } else { "update" };
    let message = format!("refactor: {} {}", action_word, format_names(&merged_names));

    let merged_plan = CommitPlan {
        work_unit_ids: merged_ids,
        message,
        order: min_order,
    };

    // Rebuild plans: merged refactor + non-refactor plans
    let mut result: Vec<CommitPlan> = vec![merged_plan];
    for plan in plans {
        let is_refactor = plan.work_unit_ids.iter().all(|id| {
            units
                .iter()
                .find(|u| u.id == *id)
                .map(|u| u.kind == WorkKind::Refactor)
                .unwrap_or(false)
        });
        if !is_refactor {
            result.push(plan);
        }
    }

    result
}

/// Generates a commit message for a single work unit.
fn generate_message(unit: &WorkUnit) -> String {
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
fn generate_test_message(test_units: &[&WorkUnit]) -> String {
    let names: Vec<String> = test_units.iter().flat_map(|u| entity_names(u)).collect();
    let formatted = format_names(&names);
    format!("test: add {}", formatted)
}

/// Generates a combined message when all units go in one commit.
fn generate_combined_message(units: &[WorkUnit]) -> String {
    // Find the primary (non-test) unit
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

/// Extracts entity names from a work unit, excluding imports.
fn entity_names(unit: &WorkUnit) -> Vec<String> {
    unit.entities
        .iter()
        .filter(|e| {
            e.kind != crate::semantic::EntityKind::Import
                && e.kind != crate::semantic::EntityKind::Module
        })
        .map(|e| e.name.clone())
        .collect()
}

/// Formats a list of names into a human-readable string.
///
/// 1 name: "foo"
/// 2 names: "foo and bar"
/// 3+ names: "foo, bar, and baz"
fn format_names(names: &[String]) -> String {
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

/// Generates warnings based on work units.
fn generate_warnings(units: &[WorkUnit]) -> Vec<String> {
    let mut warnings = Vec::new();

    // Find untested features
    let test_related_ids: std::collections::HashSet<usize> = units
        .iter()
        .filter(|u| u.kind == WorkKind::Test)
        .filter_map(|u| u.related_to)
        .collect();

    for unit in units {
        if unit.kind == WorkKind::Feature && !test_related_ids.contains(&unit.id) {
            let names = entity_names(unit);
            warnings.push(format!("feature {} has no tests", format_names(&names)));
        }
    }

    // Find public signature changes
    for unit in units {
        for change in &unit.changes {
            if change.is_public && change.change_type == SemanticChangeType::SignatureChanged {
                warnings.push(format!(
                    "public API changed: {} — push may break consumers",
                    change.entity.name
                ));
            }
        }
    }

    // Find public removals
    for unit in units {
        for change in &unit.changes {
            if change.is_public && change.change_type == SemanticChangeType::Removed {
                warnings.push(format!("public entity removed: {}", change.entity.name));
            }
        }
    }

    warnings
}
