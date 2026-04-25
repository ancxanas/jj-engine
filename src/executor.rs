//! Executor: applies planned actions via jj-lib.
//!
//! Takes an ActionPlan and applies it to the JJ repository
//! using jj-lib primitives.
//!
//! Key design rules:
//! 1. Get wc_tree and parent_tree from the SAME repo.store() after loading repo.
//! 2. After any mutation (tx.commit, init_workspace), reload workspace/repo fresh.
//! 3. Never reuse Workspace or MergedTree objects across mutation boundaries.

use std::path::Path;
use std::path::PathBuf;

use crate::jj_context::JjContext;
use anyhow::Result;
use jj_lib::gitignore::GitIgnoreFile;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::matchers::FilesMatcher;
use jj_lib::op_store::RefTarget;
use jj_lib::ref_name::RefNameBuf;
use jj_lib::repo::Repo;
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::rewrite::restore_tree;
use jj_lib::working_copy::SnapshotOptions;
use jj_lib::workspace::Workspace;

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
}

/// Executes an action plan against a JJ repository.
pub async fn execute(
    repo_path: &Path,
    plan: &ActionPlan,
    work_units: &[WorkUnit],
    snapshot: &crate::semantic::SemanticSnapshot,
) -> Result<ExecutionReport> {
    let repo_path = repo_path.canonicalize()?;
    let repo_path = repo_path.as_path();

    let mut work_unit_commit_map: std::collections::HashMap<usize, jj_lib::backend::CommitId> =
        std::collections::HashMap::new();
    let mut report = match &plan.action {
        JjAction::NoOp => {
            ExecutionReport::success(vec!["No action needed.".to_string()], plan.warnings.clone())
        }

        JjAction::AmendCommit { message } => {
            let (report, commit_map) =
                execute_amend(repo_path, message, &plan.warnings, work_units).await?;
            work_unit_commit_map = commit_map;
            report
        }

        JjAction::CreateCommit { message } => {
            let (report, commit_map) =
                execute_create(repo_path, message, &plan.warnings, work_units).await?;
            work_unit_commit_map = commit_map;
            report
        }

        JjAction::SplitCommit { plans } => {
            let (split_report, commit_map) =
                execute_split(repo_path, plans, work_units, snapshot, &plan.warnings).await?;
            work_unit_commit_map = commit_map;
            split_report
        }
    };

    // Create workspaces after commits
    if plan.workspaces.is_empty() {
        return Ok(report);
    }

    // Phase A: Create all workspace metadata, threading repo forward
    let ctx = JjContext::new()?;
    let base_ws = ctx.load_workspace(repo_path)?;
    let mut latest_repo = base_ws.repo_loader().load_at_head().await?;

    // Store only path + target commit id. Never Workspace objects.
    let mut created: Vec<(PathBuf, jj_lib::backend::CommitId)> = Vec::new();

    for ws_plan in &plan.workspaces {
        let base_ws = ctx.load_workspace(repo_path)?;

        // Target commit = current wc commit before workspace metadata changes
        let target_commit_id = work_unit_commit_map
            .get(&ws_plan.work_unit_id)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!("no commit found for work unit {}", ws_plan.work_unit_id)
            })?;

        // Create workspace metadata only — no checkout yet
        let (_new_ws, new_repo) = init_workspace_metadata(&base_ws, &latest_repo, ws_plan).await?;

        created.push((ws_plan.path.clone(), target_commit_id));

        // Thread repo forward to next iteration
        latest_repo = new_repo;
    }

    // Phase B: Use a transaction to properly checkout each workspace
    // MutableRepo::check_out() updates both repo view AND working copy files
    {
        let ctx = JjContext::new()?;
        let base_ws = ctx.load_workspace(repo_path)?;
        let repo = base_ws.repo_loader().load_at_head().await?;

        let mut tx = repo.start_transaction();

        // First update repo view for all new workspaces
        for (ws_path, target_commit_id) in &created {
            let ctx = JjContext::new()?;
            let ws = ctx.load_workspace(ws_path)?;
            let ws_name = ws.workspace_name().to_owned();

            let target_commit = tx.repo_mut().store().get_commit(target_commit_id)?;

            // Creates new wc commit on top of target and updates repo view
            tx.repo_mut().check_out(ws_name, &target_commit).await?;
        }

        tx.repo_mut().rebase_descendants().await?;

        tx.commit("jj-engine: finalize workspaces".to_string())
            .await?;

        // Now update each created workspace's files on disk using a repo loaded
        // FROM THAT SAME WORKSPACE. This guarantees same Store Arc.
        for (ws_path, _) in &created {
            let ctx = JjContext::new()?;
            let mut ws = ctx.load_workspace(ws_path)?;
            let repo = ws.repo_loader().load_at_head().await?;
            let ws_name = ws.workspace_name().to_owned();

            let wc_commit_id = repo
                .view()
                .get_wc_commit_id(&ws_name)
                .ok_or_else(|| anyhow::anyhow!("no wc commit for workspace"))?
                .clone();

            let wc_commit = repo.store().get_commit(&wc_commit_id)?;
            ws.check_out(repo.op_id().clone(), None, &wc_commit).await?;

            report.actions_executed.push(format!(
                "Workspace '{}' finalized at {}",
                ws.workspace_name().as_str(),
                ws.workspace_root().display()
            ));
        }

        // Finally refresh the default workspace too
        update_working_copy(repo_path).await?;
        // Create bookmarks after workspaces
    }

    if !plan.bookmarks.is_empty() {
        let bookmark_msgs =
            execute_create_bookmarks(repo_path, &plan.bookmarks, &work_unit_commit_map).await?;

        report.actions_executed.extend(bookmark_msgs);
    }

    Ok(report)
}

/// Snapshots working copy.
/// Returns workspace (for repo loading) but does NOT return wc_tree.
/// Get wc_tree from repo.store() after loading repo to guarantee same store.
async fn load_and_snapshot(repo_path: &Path) -> Result<Workspace> {
    let ctx = JjContext::new()?;
    let mut workspace = ctx.load_workspace(repo_path)?;

    let snapshot_options = SnapshotOptions {
        base_ignores: GitIgnoreFile::empty(),
        progress: None,
        start_tracking_matcher: &EverythingMatcher,
        force_tracking_matcher: &EverythingMatcher,
        max_new_file_size: u64::MAX,
    };

    let mut locked_ws = workspace.start_working_copy_mutation()?;
    let (_wc_tree, _stats) = locked_ws.locked_wc().snapshot(&snapshot_options).await?;
    let op_id = locked_ws.locked_wc().old_operation_id().clone();
    locked_ws.finish(op_id).await?;

    Ok(workspace)
}

/// Updates working copy files on disk after a transaction.
/// Always reloads workspace and repo fresh to avoid stale objects.
async fn update_working_copy(repo_path: &Path) -> Result<()> {
    let ctx = JjContext::new()?;
    let mut workspace = ctx.load_workspace(repo_path)?;
    let repo = workspace.repo_loader().load_at_head().await?;

    let ws_name = workspace.workspace_name().to_owned();
    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&ws_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit"))?
        .clone();

    // Commit from same store as repo
    let commit = repo.store().get_commit(&wc_commit_id)?;

    workspace
        .check_out(repo.op_id().clone(), None, &commit)
        .await?;

    Ok(())
}

/// Amends the current working copy commit.
async fn execute_amend(
    repo_path: &Path,
    message: &str,
    warnings: &[String],
    work_units: &[WorkUnit],
) -> Result<(
    ExecutionReport,
    std::collections::HashMap<usize, jj_lib::backend::CommitId>,
)> {
    // Step 1: Snapshot working copy
    let workspace = load_and_snapshot(repo_path).await?;

    // Step 2: Load repo from SAME workspace after snapshot
    let repo: std::sync::Arc<jj_lib::repo::ReadonlyRepo> =
        workspace.repo_loader().load_at_head().await?;
    let workspace_name = workspace.workspace_name().to_owned();

    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit"))?
        .clone();

    // Step 3: Get wc_tree from repo.store() — same store as everything else
    let store = repo.store();
    let wc_commit = store.get_commit(&wc_commit_id)?;
    let wc_tree = wc_commit.tree();

    // Step 4: Transaction
    let mut tx = repo.start_transaction();

    let new_commit = tx
        .repo_mut()
        .rewrite_commit(&wc_commit)
        .set_description(message)
        .set_tree(wc_tree)
        .write()
        .await?;

    tx.repo_mut().rebase_descendants().await?;
    tx.repo_mut()
        .set_wc_commit(workspace_name, new_commit.id().clone())?;

    tx.commit("jj-engine: amend commit".to_string()).await?;

    // Step 5: Fresh reload for checkout — avoids stale object reuse
    update_working_copy(repo_path).await?;

    let mut work_unit_to_commit = std::collections::HashMap::new();
    for unit in work_units {
        work_unit_to_commit.insert(unit.id, new_commit.id().clone());
    }

    Ok((
        ExecutionReport::success(
            vec![format!("Amended commit with message: \"{}\"", message)],
            warnings.to_vec(),
        ),
        work_unit_to_commit,
    ))
}

/// Creates a new commit on top of the current working copy commit.
async fn execute_create(
    repo_path: &Path,
    message: &str,
    warnings: &[String],
    work_units: &[WorkUnit],
) -> Result<(
    ExecutionReport,
    std::collections::HashMap<usize, jj_lib::backend::CommitId>,
)> {
    // Step 1: Snapshot
    let workspace = load_and_snapshot(repo_path).await?;

    // Step 2: Load repo from SAME workspace
    let repo: std::sync::Arc<jj_lib::repo::ReadonlyRepo> =
        workspace.repo_loader().load_at_head().await?;
    let workspace_name = workspace.workspace_name().to_owned();

    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit"))?
        .clone();

    // Step 3: Get wc_tree from repo.store()
    let store = repo.store();
    let wc_commit = store.get_commit(&wc_commit_id)?;
    let wc_tree = wc_commit.tree();

    // Step 4: Transaction
    let mut tx = repo.start_transaction();

    let new_commit = tx
        .repo_mut()
        .new_commit(vec![wc_commit_id], wc_tree)
        .set_description(message)
        .generate_new_change_id()
        .write()
        .await?;

    tx.repo_mut()
        .set_wc_commit(workspace_name, new_commit.id().clone())?;

    tx.commit("jj-engine: create commit".to_string()).await?;

    // Step 5: Fresh reload for checkout
    update_working_copy(repo_path).await?;

    let mut work_unit_to_commit = std::collections::HashMap::new();
    for unit in work_units {
        work_unit_to_commit.insert(unit.id, new_commit.id().clone());
    }

    Ok((
        ExecutionReport::success(
            vec![format!("Created commit with message: \"{}\"", message)],
            warnings.to_vec(),
        ),
        work_unit_to_commit,
    ))
}

/// Splits work into multiple commits, one per CommitPlan.
async fn execute_split(
    repo_path: &Path,
    plans: &[CommitPlan],
    work_units: &[WorkUnit],
    snapshot: &crate::semantic::SemanticSnapshot,
    warnings: &[String],
) -> Result<(
    ExecutionReport,
    std::collections::HashMap<usize, jj_lib::backend::CommitId>,
)> {
    // Step 1: Snapshot
    let workspace = load_and_snapshot(repo_path).await?;
    let mut work_unit_to_commit: std::collections::HashMap<usize, jj_lib::backend::CommitId> =
        std::collections::HashMap::new();
    // Step 2: Load repo from SAME workspace
    let repo: std::sync::Arc<jj_lib::repo::ReadonlyRepo> =
        workspace.repo_loader().load_at_head().await?;
    let workspace_name = workspace.workspace_name().to_owned();

    let wc_commit_id = repo
        .view()
        .get_wc_commit_id(&workspace_name)
        .ok_or_else(|| anyhow::anyhow!("no working copy commit"))?
        .clone();

    // Step 3: Get BOTH trees from SAME store — fixes store mismatch
    let store = repo.store().clone();
    let wc_commit = store.get_commit(&wc_commit_id)?;
    let wc_tree = wc_commit.tree();

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

                let file_id = {
                    let mut cursor = std::io::Cursor::new(partial_content.into_bytes());
                    store.write_file(&repo_path_buf, &mut cursor).await?
                };

                let value =
                    jj_lib::merge::Merge::resolved(Some(jj_lib::backend::TreeValue::File {
                        id: file_id,
                        executable: false,
                        copy_id: jj_lib::backend::CopyId::placeholder(),
                    }));

                tree_builder.set_or_remove(repo_path_buf, value);
            }

            let partial_tree = tree_builder.write_tree().await?;

            let new_commit: jj_lib::commit::Commit = tx
                .repo_mut()
                .new_commit(vec![current_parent_id.clone()], partial_tree)
                .set_description(plan.message.clone())
                .generate_new_change_id()
                .write()
                .await?;

            actions_executed.push(format!("Created commit: \"{}\"", plan.message));

            current_parent_id = new_commit.id().clone();
            for unit_id in &plan.work_unit_ids {
                work_unit_to_commit.insert(*unit_id, new_commit.id().clone());
            }
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
                wc_tree.clone() as jj_lib::merged_tree::MergedTree
            } else {
                let matcher = FilesMatcher::new(repo_paths);
                restore_tree(
                    &wc_tree,
                    &parent_tree,
                    String::new(),
                    String::new(),
                    &matcher,
                )
                .await?
            };

            let new_commit: jj_lib::commit::Commit = tx
                .repo_mut()
                .new_commit(vec![current_parent_id.clone()], partial_tree)
                .set_description(plan.message.clone())
                .generate_new_change_id()
                .write()
                .await?;

            actions_executed.push(format!("Created commit: \"{}\"", plan.message));

            current_parent_id = new_commit.id().clone();
        }
    }

    tx.repo_mut()
        .set_wc_commit(workspace_name, current_parent_id.clone())?;

    tx.commit("jj-engine: split work into commits".to_string())
        .await?;

    // Step 5: Fresh reload for checkout
    update_working_copy(repo_path).await?;

    Ok((
        ExecutionReport::success(actions_executed, warnings.to_vec()),
        work_unit_to_commit,
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

/// Creates workspace metadata only. No checkout.
/// Returns the new workspace and the updated repo (for threading).
async fn init_workspace_metadata(
    base_workspace: &Workspace,
    repo: &std::sync::Arc<jj_lib::repo::ReadonlyRepo>,
    workspace_plan: &crate::decision::WorkspacePlan,
) -> Result<(Workspace, std::sync::Arc<jj_lib::repo::ReadonlyRepo>)> {
    use jj_lib::ref_name::WorkspaceNameBuf;
    use jj_lib::workspace::default_working_copy_factory;

    std::fs::create_dir_all(&workspace_plan.path)?;

    let workspace_name = WorkspaceNameBuf::from(workspace_plan.name.as_str());

    let (new_ws, new_repo) = jj_lib::workspace::Workspace::init_workspace_with_existing_repo(
        &workspace_plan.path,
        base_workspace.repo_path(),
        repo,
        &*default_working_copy_factory(),
        workspace_name,
    )
    .await?;

    Ok((new_ws, new_repo))
}

async fn execute_create_bookmarks(
    repo_path: &Path,
    bookmark_plans: &[crate::decision::BookmarkPlan],
    work_unit_commit_map: &std::collections::HashMap<usize, jj_lib::backend::CommitId>,
) -> Result<Vec<String>> {
    let ctx = JjContext::new()?;
    let workspace = ctx.load_workspace(repo_path)?;
    let repo = workspace.repo_loader().load_at_head().await?;

    let mut tx = repo.start_transaction();
    let mut messages = Vec::new();

    for bm in bookmark_plans {
        let commit_id = work_unit_commit_map
            .get(&bm.work_unit_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no commit id found for bookmark work unit {}",
                    bm.work_unit_id
                )
            })?
            .clone();

        let name_buf = RefNameBuf::from(bm.name.as_str());

        tx.repo_mut()
            .set_local_bookmark_target(name_buf.as_ref(), RefTarget::resolved(Some(commit_id)));

        messages.push(format!("Bookmark '{}' created", bm.name));
    }

    tx.commit("jj-engine: create bookmarks".to_string()).await?;

    // Refresh default workspace after bookmark transaction
    update_working_copy(repo_path).await?;

    Ok(messages)
}
