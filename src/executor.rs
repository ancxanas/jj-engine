//! Executor: applies planned actions via jj-lib.
//!
//! Takes an ActionPlan and applies it to the JJ repository
//! using jj-lib primitives. Every action is wrapped in a
//! single atomic transaction.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use jj_lib::config::StackedConfig;
use jj_lib::gitignore::GitIgnoreFile;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::matchers::FilesMatcher;
use jj_lib::repo::Repo;
use jj_lib::repo::StoreFactories;
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::rewrite::restore_tree;
use jj_lib::settings::UserSettings;
use jj_lib::working_copy::SnapshotOptions;
use jj_lib::workspace::Workspace;
use jj_lib::workspace::default_working_copy_factories;
use pollster::FutureExt as _;

use crate::decision::ActionPlan;
use crate::decision::CommitPlan;
use crate::decision::JjAction;
use crate::work_inference::WorkUnit;

/// Report of what the executor did.
#[derive(Debug)]
pub struct ExecutionReport {
    pub success: bool,
    pub actions_executed: Vec<String>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

impl ExecutionReport {
    fn success(actions: Vec<String>, warnings: Vec<String>) -> Self {
        Self {
            success: true,
            actions_executed: actions,
            warnings,
            error: None,
        }
    }

    fn failure(reason: String) -> Self {
        Self {
            success: false,
            actions_executed: Vec::new(),
            warnings: Vec::new(),
            error: Some(reason),
        }
    }
}

/// Executes an action plan against a JJ repository.
pub fn execute(
    repo_path: &Path,
    plan: &ActionPlan,
    work_units: &[WorkUnit],
    snapshot: &crate::semantic::SemanticSnapshot,
) -> Result<ExecutionReport> {
    let repo_path = repo_path.canonicalize()?;
    let repo_path = repo_path.as_path();

    match &plan.action {
        JjAction::NoOp => Ok(ExecutionReport::success(
            vec!["No action needed.".to_string()],
            plan.warnings.clone(),
        )),

        JjAction::RequireHumanIntervention { reason } => {
            Ok(ExecutionReport::failure(reason.clone()))
        }

        JjAction::AmendCommit { message } => execute_amend(repo_path, message, &plan.warnings),

        JjAction::CreateCommit { message } => execute_create(repo_path, message, &plan.warnings),

        JjAction::SplitCommit { plans } => {
            execute_split(repo_path, plans, work_units, snapshot, &plan.warnings)
        }
    }
}

/// Loads workspace and snapshots working copy.
/// Returns the workspace (still alive for reuse) and the snapshot tree.
fn load_and_snapshot(repo_path: &Path) -> Result<(Workspace, jj_lib::merged_tree::MergedTree)> {
    let config = StackedConfig::with_defaults();
    let settings = UserSettings::from_config(config)?;
    let store_factories = StoreFactories::default();
    let working_copy_factories = default_working_copy_factories();

    let mut workspace = Workspace::load(
        &settings,
        repo_path,
        &store_factories,
        &working_copy_factories,
    )?;

    let snapshot_options = SnapshotOptions {
        base_ignores: GitIgnoreFile::empty(),
        progress: None,
        start_tracking_matcher: &EverythingMatcher,
        force_tracking_matcher: &EverythingMatcher,
        max_new_file_size: u64::MAX,
    };

    let mut locked_ws = workspace.start_working_copy_mutation()?;
    let (wc_tree, _stats) = locked_ws
        .locked_wc()
        .snapshot(&snapshot_options)
        .block_on()?;
    let op_id = locked_ws.locked_wc().old_operation_id().clone();
    locked_ws.finish(op_id).block_on()?;

    Ok((workspace, wc_tree))
}

/// Amends the current working copy commit.
fn execute_amend(repo_path: &Path, message: &str, warnings: &[String]) -> Result<ExecutionReport> {
    // Load once
    let (mut workspace, wc_tree) = load_and_snapshot(repo_path)?;

    // Load repo from SAME workspace
    let repo = workspace.repo_loader().load_at_head().block_on()?;
    let workspace_name = workspace.workspace_name().to_owned();

    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit"))?
        .clone();

    let commit = repo.store().get_commit(&wc_commit_id)?;

    // Transaction
    let mut tx = repo.start_transaction();

    let new_commit = tx
        .repo_mut()
        .rewrite_commit(&commit)
        .set_description(message)
        .set_tree(wc_tree)
        .write()
        .block_on()?;

    tx.repo_mut().rebase_descendants().block_on()?;

    tx.repo_mut()
        .set_wc_commit(workspace_name, new_commit.id().clone())?;

    let new_repo = tx
        .commit("jj-engine: amend commit".to_string())
        .block_on()?;

    // Checkout using SAME workspace
    let final_commit = new_repo.store().get_commit(new_commit.id())?;
    workspace
        .check_out(new_repo.op_id().clone(), None, &final_commit)
        .block_on()?;

    Ok(ExecutionReport::success(
        vec![format!("Amended commit with message: \"{}\"", message)],
        warnings.to_vec(),
    ))
}

/// Creates a new commit on top of the current working copy commit.
fn execute_create(repo_path: &Path, message: &str, warnings: &[String]) -> Result<ExecutionReport> {
    // Load once
    let (mut workspace, wc_tree) = load_and_snapshot(repo_path)?;

    // Load repo from SAME workspace
    let repo = workspace.repo_loader().load_at_head().block_on()?;
    let workspace_name = workspace.workspace_name().to_owned();

    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit"))?
        .clone();

    // Transaction
    let mut tx = repo.start_transaction();

    let new_commit = tx
        .repo_mut()
        .new_commit(vec![wc_commit_id], wc_tree)
        .set_description(message)
        .generate_new_change_id()
        .write()
        .block_on()?;

    tx.repo_mut()
        .set_wc_commit(workspace_name, new_commit.id().clone())?;

    let new_repo = tx
        .commit("jj-engine: create commit".to_string())
        .block_on()?;

    // Checkout using SAME workspace
    let final_commit = new_repo.store().get_commit(new_commit.id())?;
    workspace
        .check_out(new_repo.op_id().clone(), None, &final_commit)
        .block_on()?;

    Ok(ExecutionReport::success(
        vec![format!("Created commit with message: \"{}\"", message)],
        warnings.to_vec(),
    ))
}

/// Splits work into multiple commits, one per CommitPlan.
fn execute_split(
    repo_path: &Path,
    plans: &[CommitPlan],
    work_units: &[WorkUnit],
    snapshot: &crate::semantic::SemanticSnapshot,
    warnings: &[String],
) -> Result<ExecutionReport> {
    // Load once
    let (mut workspace, wc_tree) = load_and_snapshot(repo_path)?;

    // Load repo from SAME workspace
    let repo = workspace.repo_loader().load_at_head().block_on()?;
    let workspace_name = workspace.workspace_name().to_owned();

    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit"))?
        .clone();

    let store = repo.store().clone();
    let wc_commit = store.get_commit(&wc_commit_id)?;

    let parent_id = wc_commit
        .parent_ids()
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no parent commit"))?;

    let parent_commit = store.get_commit(&parent_id)?;
    let parent_tree = parent_commit.tree();

    // Check if all plans share the same files
    let plan_file_sets: Vec<std::collections::HashSet<PathBuf>> = plans
        .iter()
        .map(|p| files_for_plan(p, work_units).into_iter().collect())
        .collect();

    let all_same_files =
        plan_file_sets.len() > 1 && plan_file_sets.windows(2).all(|w| w[0] == w[1]);

    let mut tx = repo.start_transaction();
    let mut actions_executed = Vec::new();
    let mut current_parent_id = parent_id.clone();

    let mut sorted_plans = plans.to_vec();
    sorted_plans.sort_by_key(|p| p.order);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    if all_same_files {
        // Entity-level splitting
        let shared_files: Vec<PathBuf> = plan_file_sets[0].iter().cloned().collect();

        let mut wc_file_contents: std::collections::HashMap<PathBuf, Vec<String>> =
            std::collections::HashMap::new();

        for file_path in &shared_files {
            let content = std::fs::read_to_string(file_path)?;
            let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
            wc_file_contents.insert(file_path.clone(), lines);
        }

        // Sort by earliest entity line number
        sorted_plans.sort_by_key(|p| {
            lines_for_plan(p, work_units, snapshot)
                .iter()
                .map(|(start, _)| *start)
                .min()
                .unwrap_or(usize::MAX)
        });

        let mut committed_up_to: usize = 0;

        for plan in &sorted_plans {
            let entity_lines = lines_for_plan(plan, work_units, snapshot);

            let max_line = entity_lines
                .iter()
                .map(|(_, end)| *end)
                .max()
                .map(|m| m + 1)
                .unwrap_or(committed_up_to);

            let include_up_to = max_line.max(committed_up_to);

            let mut tree_builder =
                jj_lib::merged_tree_builder::MergedTreeBuilder::new(parent_tree.clone());

            for file_path in &shared_files {
                let wc_lines = match wc_file_contents.get(file_path) {
                    Some(l) => l,
                    None => continue,
                };

                let partial_lines: Vec<&str> = wc_lines
                    .iter()
                    .take(include_up_to)
                    .map(|s| s.as_str())
                    .collect();

                let partial_content = partial_lines.join("\n") + "\n";

                let rel_path = file_path.strip_prefix(repo_path)?;
                let rel_str = rel_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid path"))?;
                let repo_path_buf = RepoPathBuf::from_internal_string(rel_str)?;

                let file_id = rt.block_on(async {
                    let mut cursor = std::io::Cursor::new(partial_content.into_bytes());
                    store.write_file(&repo_path_buf, &mut cursor).await
                })?;

                let value =
                    jj_lib::merge::Merge::resolved(Some(jj_lib::backend::TreeValue::File {
                        id: file_id,
                        executable: false,
                        copy_id: jj_lib::backend::CopyId::placeholder(),
                    }));

                tree_builder.set_or_remove(repo_path_buf, value);
            }

            let partial_tree = tree_builder.write_tree().block_on()?;

            let new_commit = tx
                .repo_mut()
                .new_commit(vec![current_parent_id.clone()], partial_tree)
                .set_description(plan.message.clone())
                .generate_new_change_id()
                .write()
                .block_on()?;

            actions_executed.push(format!("Created commit: \"{}\"", plan.message));

            current_parent_id = new_commit.id().clone();
            committed_up_to = include_up_to;
        }
    } else {
        // File-level splitting
        for plan in &sorted_plans {
            let plan_files = files_for_plan(plan, work_units);

            let repo_paths: Vec<RepoPathBuf> = plan_files
                .iter()
                .filter_map(|p| {
                    let rel = p.strip_prefix(repo_path).ok()?;
                    let rel_str = rel.to_str()?;
                    RepoPathBuf::from_internal_string(rel_str).ok()
                })
                .collect();

            let partial_tree = if repo_paths.is_empty() {
                wc_tree.clone()
            } else {
                let matcher = FilesMatcher::new(repo_paths);
                restore_tree(
                    &wc_tree,
                    &parent_tree,
                    String::new(),
                    String::new(),
                    &matcher,
                )
                .block_on()?
            };

            let new_commit = tx
                .repo_mut()
                .new_commit(vec![current_parent_id.clone()], partial_tree)
                .set_description(plan.message.clone())
                .generate_new_change_id()
                .write()
                .block_on()?;

            actions_executed.push(format!("Created commit: \"{}\"", plan.message));

            current_parent_id = new_commit.id().clone();
        }
    }

    tx.repo_mut()
        .set_wc_commit(workspace_name, current_parent_id.clone())?;

    let new_repo = tx
        .commit("jj-engine: split work into commits".to_string())
        .block_on()?;

    // Checkout using SAME workspace
    let final_commit = new_repo.store().get_commit(&current_parent_id)?;
    workspace
        .check_out(new_repo.op_id().clone(), None, &final_commit)
        .block_on()?;

    Ok(ExecutionReport::success(
        actions_executed,
        warnings.to_vec(),
    ))
}

/// Returns line ranges for entities in a plan's work units.
fn lines_for_plan(
    plan: &CommitPlan,
    work_units: &[WorkUnit],
    snapshot: &crate::semantic::SemanticSnapshot,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();

    for unit_id in &plan.work_unit_ids {
        if let Some(unit) = work_units.iter().find(|u| u.id == *unit_id) {
            for entity_path in &unit.entities {
                if let Some(entity_info) = snapshot.entities.get(entity_path) {
                    ranges.push((entity_info.line_start, entity_info.line_end));
                }
            }
        }
    }

    ranges.sort_by_key(|r| r.0);
    ranges
}

/// Returns files belonging to a commit plan.
fn files_for_plan(plan: &CommitPlan, work_units: &[WorkUnit]) -> Vec<PathBuf> {
    let mut files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for unit_id in &plan.work_unit_ids {
        if let Some(unit) = work_units.iter().find(|u| u.id == *unit_id) {
            for entity in &unit.entities {
                files.insert(entity.file.clone());
            }
        }
    }

    files.into_iter().collect()
}
