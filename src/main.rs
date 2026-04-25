mod decision;
mod executor;
mod explainer;
mod jj_context;
mod policy;
mod repo_inspector;
mod semantic;
mod session;
mod work_inference;

use std::path::Path;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("usage:");
        eprintln!("  jj-engine <repo-path>");
        eprintln!("  jj-engine parse <file-path>");
        eprintln!("  jj-engine snapshot <project-path>");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "parse" => {
            if args.len() < 3 {
                eprintln!("usage: jj-engine parse <file-path>");
                std::process::exit(1);
            }
            let file_path = Path::new(&args[2]);
            let source = match std::fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error reading file: {}", e);
                    std::process::exit(1);
                }
            };
            match semantic::parse_file(file_path, &source) {
                Ok(entities) => {
                    println!("Entities in {}:", file_path.display());
                    println!();
                    for entity in &entities {
                        println!(
                            "  [{:?}] {} (lines {}-{}) public={} test={}",
                            entity.path.kind,
                            entity.path.name,
                            entity.line_start + 1,
                            entity.line_end + 1,
                            entity.is_public,
                            entity.is_test,
                        );
                        if !entity.calls.is_empty() {
                            println!("    calls:      {:?}", entity.calls);
                        }
                        if !entity.uses_types.is_empty() {
                            println!("    uses types: {:?}", entity.uses_types);
                        }
                        if !entity.imports.is_empty() {
                            println!("    imports:    {:?}", entity.imports);
                        }
                    }
                    println!();
                    println!("Total: {} entities", entities.len());
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "snapshot" => {
            if args.len() < 3 {
                eprintln!("usage: jj-engine snapshot <project-path>");
                std::process::exit(1);
            }
            let project_path = Path::new(&args[2]);
            match semantic::snapshot_project(project_path) {
                Ok(snapshot) => {
                    println!("Semantic snapshot of {}:", project_path.display());
                    println!("  Files scanned: {}", snapshot.files_scanned);
                    println!("  Entities found: {}", snapshot.entities.len());

                    // Group entities by file for display
                    let mut by_file: std::collections::BTreeMap<
                        &std::path::PathBuf,
                        Vec<&semantic::EntityInfo>,
                    > = std::collections::BTreeMap::new();

                    for entity in snapshot.entities.values() {
                        by_file.entry(&entity.path.file).or_default().push(entity);
                    }

                    println!();
                    for (file, file_entities) in &by_file {
                        println!("  {} ({} entities)", file.display(), file_entities.len());
                        for entity in file_entities {
                            println!("    [{:?}] {}", entity.path.kind, entity.path.name,);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "diff-test" => {
            // Test diffing with two snapshots
            // First snapshot: current project
            // Second snapshot: same project (no changes expected)

            if args.len() < 3 {
                eprintln!("usage: jj-engine diff-test <project-path>");
                std::process::exit(1);
            }

            let project_path = Path::new(&args[2]);

            println!("Testing semantic diff...\n");

            // Take snapshot
            let snapshot = match semantic::snapshot_project(project_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            };

            println!("Snapshot: {} entities\n", snapshot.entities.len());

            // Diff against itself (should be empty)
            let self_diff = semantic::diff_snapshots(&snapshot, &snapshot);
            println!("Diff against self: {} changes", self_diff.changes.len());
            assert!(self_diff.is_empty(), "self-diff should be empty");
            println!("  ✓ Self-diff is empty as expected\n");

            // Diff against empty (everything should be "added")
            let empty = semantic::SemanticSnapshot::empty();
            let from_empty_diff = semantic::diff_snapshots(&empty, &snapshot);
            println!("Diff from empty: {} changes", from_empty_diff.changes.len());
            println!("  Added: {}", from_empty_diff.added().len());
            println!("  Removed: {}", from_empty_diff.removed().len());
            assert_eq!(from_empty_diff.added().len(), snapshot.entities.len());
            println!("  ✓ All entities show as added\n");

            // Diff to empty (everything should be "removed")
            let to_empty_diff = semantic::diff_snapshots(&snapshot, &empty);
            println!("Diff to empty: {} changes", to_empty_diff.changes.len());
            println!("  Added: {}", to_empty_diff.added().len());
            println!("  Removed: {}", to_empty_diff.removed().len());
            assert_eq!(to_empty_diff.removed().len(), snapshot.entities.len());
            println!("  ✓ All entities show as removed\n");

            // Check breaking changes detection
            let has_breaking = to_empty_diff.has_breaking_changes();
            println!("Has breaking changes (removing all): {}", has_breaking);

            println!("\n✓ All diff tests passed!");
        }

        "diff" => {
            if args.len() < 3 {
                eprintln!("usage: jj-engine diff <repo-path>");
                std::process::exit(1);
            }

            let repo_path = Path::new(&args[2]);

            println!("Computing semantic diff for {:?}\n", repo_path);

            let session = match session::RepoSession::load(repo_path).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error loading repo session: {}", e);
                    std::process::exit(1);
                }
            };

            let before = match repo_inspector::committed_snapshot_from_session(&session).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building committed snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            println!("Committed snapshot: {} entities", before.entities.len());

            // Build "after" snapshot from working copy files
            let after = match semantic::snapshot_project(&session.repo_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building working copy snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            println!("Working copy snapshot: {} entities\n", after.entities.len());

            // Diff the two snapshots
            let diff = semantic::diff_snapshots(&before, &after);

            if diff.is_empty() {
                println!("No semantic changes detected.");
                return;
            }

            println!("Semantic diff:\n");

            let added = diff.added();
            if !added.is_empty() {
                println!("  Added ({}):", added.len());
                for change in &added {
                    println!(
                        "    [{:?}] {} in {}",
                        change.entity.kind,
                        change.entity.name,
                        change.entity.file.display(),
                    );
                }
                println!();
            }

            let removed = diff.removed();
            if !removed.is_empty() {
                println!("  Removed ({}):", removed.len());
                for change in &removed {
                    println!(
                        "    [{:?}] {} in {}",
                        change.entity.kind,
                        change.entity.name,
                        change.entity.file.display(),
                    );
                }
                println!();
            }

            let sig_changed = diff.signature_changed();
            if !sig_changed.is_empty() {
                println!("  Signature changed ({}):", sig_changed.len());
                for change in &sig_changed {
                    println!(
                        "    [{:?}] {} in {} {}",
                        change.entity.kind,
                        change.entity.name,
                        change.entity.file.display(),
                        if change.is_public { "[public]" } else { "" },
                    );
                }
                println!();
            }

            let impl_changed = diff.implementation_changed();
            if !impl_changed.is_empty() {
                println!("  Implementation changed ({}):", impl_changed.len());
                for change in &impl_changed {
                    println!(
                        "    [{:?}] {} in {}",
                        change.entity.kind,
                        change.entity.name,
                        change.entity.file.display(),
                    );
                }
                println!();
            }

            if diff.has_breaking_changes() {
                println!("  ⚠ Breaking changes detected (public API changed)");
            }
        }

        "graph" => {
            if args.len() < 3 {
                eprintln!("usage: jj-engine graph <project-path>");
                std::process::exit(1);
            }

            let project_path = Path::new(&args[2]);

            let snapshot = match semantic::snapshot_project(project_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            };

            println!("Building dependency graph for {}\n", project_path.display());

            let graph = work_inference::build_graph(&snapshot);

            println!("Entities: {}", snapshot.entities.len());
            println!("Edges: {}\n", graph.edge_count());

            println!("Dependency Trees:\n");
            let mut visited = std::collections::HashSet::new();

            let all_paths: Vec<_> = snapshot.entities.keys().collect();
            let mut roots: Vec<_> = all_paths
                .iter()
                .filter(|p| graph.dependents_of(p).is_empty())
                .filter(|p| {
                    p.kind != semantic::EntityKind::Import && p.kind != semantic::EntityKind::Module
                })
                .collect();

            roots.sort_by_key(|p| &p.name);

            for root in roots {
                print_tree(root, &graph, 0, &mut visited);
            }

            let all_entities: Vec<semantic::EntityPath> = snapshot
                .entities
                .keys()
                .filter(|p| {
                    p.kind != semantic::EntityKind::Import && p.kind != semantic::EntityKind::Module
                })
                .cloned()
                .collect();

            let components = work_inference::find_connected_components(&all_entities, &graph);

            println!("\nConnected components ({}):\n", components.len());
            for (i, component) in components.iter().enumerate() {
                println!("  Component {} ({} entities):", i + 1, component.len());
                for entity in component {
                    println!(
                        "    {} ({:?}) in {}",
                        entity.name,
                        entity.kind,
                        entity
                            .file
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                    );
                }
            }
        }

        "work-units" => {
            if args.len() < 3 {
                eprintln!("usage: jj-engine work-units <repo-path>");
                std::process::exit(1);
            }

            let repo_path = Path::new(&args[2]);

            println!("Inferring work units for {}\n", repo_path.display());

            let session = match session::RepoSession::load(repo_path).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error loading repo session: {}", e);
                    std::process::exit(1);
                }
            };

            let before = match repo_inspector::committed_snapshot_from_session(&session).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building committed snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            // Build after snapshot from working copy
            let after = match semantic::snapshot_project(&session.repo_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building working copy snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            // Compute semantic diff
            let diff = semantic::diff_snapshots(&before, &after);

            if diff.is_empty() {
                println!("No semantic changes detected. Nothing to do.");
                return;
            }

            // Build dependency graph from working copy snapshot
            let graph = work_inference::build_graph(&after);

            // Collect changed entities (excluding imports for now)
            let changed_entities: Vec<semantic::EntityPath> = diff
                .changes
                .iter()
                .filter(|c| {
                    c.entity.kind != semantic::EntityKind::Import
                        && c.entity.kind != semantic::EntityKind::Module
                })
                .map(|c| c.entity.clone())
                .collect();

            // Find connected components among changed entities
            let components = work_inference::find_connected_components(&changed_entities, &graph);

            // Classify into work units
            let units = work_inference::classify_work_units(components, &diff, &after);

            println!("Work units found: {}\n", units.len());

            for unit in &units {
                println!(
                    "  Unit {} [{:?}]{}",
                    unit.id,
                    unit.kind,
                    unit.related_to
                        .map(|id| format!(" (tests unit {})", id))
                        .unwrap_or_default()
                );

                println!("  Entities:");
                for entity in &unit.entities {
                    let change_type = unit
                        .changes
                        .iter()
                        .find(|c| &c.entity == entity)
                        .map(|c| format!("{:?}", c.change_type))
                        .unwrap_or_else(|| "Unknown".to_string());

                    println!("    {} ({:?}) — {}", entity.name, entity.kind, change_type,);
                }
                println!();
            }

            // Report untested features
            let untested = work_inference::find_untested_features(&units);
            if !untested.is_empty() {
                println!("⚠ Untested features (no test unit linked):");
                for id in &untested {
                    if let Some(unit) = units.iter().find(|u| u.id == *id) {
                        println!(
                            "  Unit {}: {}",
                            id,
                            unit.entities
                                .iter()
                                .map(|e| e.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
            }
        }

        "plan" => {
            if args.len() < 3 {
                eprintln!("usage: jj-engine plan <repo-path>");
                std::process::exit(1);
            }

            let repo_path = Path::new(&args[2]);

            println!("Planning actions for {}\n", repo_path.display());

            let session = match session::RepoSession::load(repo_path).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error loading repo session: {}", e);
                    std::process::exit(1);
                }
            };

            let repo_state = match repo_inspector::inspect_from_session(&session).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error reading repo state: {}", e);
                    std::process::exit(1);
                }
            };

            println!("--- Repo State ---");
            println!("safe to rewrite:   {}", repo_state.is_safe_to_rewrite);
            println!("empty commit:      {}", repo_state.is_empty_commit);
            println!("has conflicts:     {}", repo_state.has_conflicts);
            println!("has changes:       {}", repo_state.has_changes);

            let before = match repo_inspector::committed_snapshot_from_session(&session).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building committed snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            let after = match semantic::snapshot_project(&session.repo_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building working copy snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            let diff = semantic::diff_snapshots(&before, &after);

            if diff.is_empty() && !repo_state.has_changes {
                println!("\nNo changes detected. Nothing to plan.");
                return;
            }

            // Build dependency graph and infer work units
            let graph = work_inference::build_graph(&after);

            let changed_entities: Vec<semantic::EntityPath> = diff
                .changes
                .iter()
                .filter(|c| {
                    c.entity.kind != semantic::EntityKind::Import
                        && c.entity.kind != semantic::EntityKind::Module
                })
                .map(|c| c.entity.clone())
                .collect();

            let components = work_inference::find_connected_components(&changed_entities, &graph);

            let units = work_inference::classify_work_units(components, &diff, &after);

            println!("\n--- Work Units ---");
            for unit in &units {
                let names: Vec<&str> = unit.entities.iter().map(|e| e.name.as_str()).collect();
                println!("Unit {} [{:?}]: {}", unit.id, unit.kind, names.join(", "),);
            }

            // Run decision engine
            let plan = decision::decide(&repo_state, &units);

            println!("\n--- Action Plan ---");
            match &plan.action {
                decision::JjAction::NoOp => {
                    println!("No action needed.");
                }
                decision::JjAction::AmendCommit { message } => {
                    println!("Action: AmendCommit");
                    println!("  Message: \"{}\"", message);
                    println!("  Reason: single change, safe to rewrite");
                }
                decision::JjAction::CreateCommit { message } => {
                    println!("Action: CreateCommit");
                    println!("  Message: \"{}\"", message);
                    println!("  Reason: published commit, creating new child");
                }
                decision::JjAction::SplitCommit { plans } => {
                    println!("Action: SplitCommit ({} commits)\n", plans.len());
                    for plan in plans {
                        println!(
                            "  Commit {} (order {}):",
                            plan.work_unit_ids
                                .iter()
                                .map(|id| id.to_string())
                                .collect::<Vec<_>>()
                                .join(", "),
                            plan.order,
                        );
                        println!("    Message: \"{}\"", plan.message);
                    }
                }
            }

            if !plan.workspaces.is_empty() {
                println!("\n--- Workspace Plans ---");
                for ws in &plan.workspaces {
                    println!("Create workspace for unit {}:", ws.work_unit_id);
                    println!("  Name: {}", ws.name);
                    println!("  Path: {}", ws.path.display());
                }
            }

            if !plan.bookmarks.is_empty() {
                println!("\n--- Bookmark Plans ---");
                for bm in &plan.bookmarks {
                    println!("Create bookmark for unit {}:", bm.work_unit_id);
                    println!("  Name: {}", bm.name);
                }
            }

            if !plan.warnings.is_empty() {
                println!("\n--- Warnings ---");
                for warning in &plan.warnings {
                    println!("⚠ {}", warning);
                }
            }
        }

        "apply" => {
            if args.len() < 3 {
                eprintln!("usage: jj-engine apply <repo-path>");
                std::process::exit(1);
            }

            let repo_path = Path::new(&args[2]);

            println!("Applying action plan to {}\n", repo_path.display());

            let session = match session::RepoSession::load(repo_path).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error loading repo session: {}", e);
                    std::process::exit(1);
                }
            };

            let repo_state = match repo_inspector::inspect_from_session(&session).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error reading repo state: {}", e);
                    std::process::exit(1);
                }
            };

            let before = match repo_inspector::committed_snapshot_from_session(&session).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building committed snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            let after = match semantic::snapshot_project(&session.repo_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building working copy snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            let diff = semantic::diff_snapshots(&before, &after);

            if diff.is_empty() && !repo_state.has_changes {
                println!("No changes detected. Nothing to do.");
                return;
            }

            // Build dependency graph and infer work units
            let graph = work_inference::build_graph(&after);

            let changed_entities: Vec<semantic::EntityPath> = diff
                .changes
                .iter()
                .filter(|c| {
                    c.entity.kind != semantic::EntityKind::Import
                        && c.entity.kind != semantic::EntityKind::Module
                })
                .map(|c| c.entity.clone())
                .collect();

            let components = work_inference::find_connected_components(&changed_entities, &graph);

            let units = work_inference::classify_work_units(components, &diff, &after);

            // Run decision engine
            let plan = decision::decide(&repo_state, &units);

            // Print plan
            println!(
                "Action: {:?}\n",
                match &plan.action {
                    decision::JjAction::NoOp => "NoOp",
                    decision::JjAction::AmendCommit { .. } => "AmendCommit",
                    decision::JjAction::CreateCommit { .. } => "CreateCommit",
                    decision::JjAction::SplitCommit { .. } => "SplitCommit",
                }
            );

            // Execute
            match executor::execute(repo_path, &plan, &units, &after).await {
                Ok(report) => {
                    if report.success {
                        for action in &report.actions_executed {
                            println!("✓ {}", action);
                        }
                        if !report.warnings.is_empty() {
                            println!("\nWarnings:");
                            for w in &report.warnings {
                                println!("⚠ {}", w);
                            }
                        }
                        println!("\nExecution complete.");
                    } else {
                        eprintln!("Execution blocked: {}", report.error.unwrap_or_default());
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Execution failed: {}", e);
                    std::process::exit(1);
                }
            }

            if !plan.workspaces.is_empty() {
                println!("\nWorkspaces:");
                for ws in &plan.workspaces {
                    println!("  {} → {}", ws.name, ws.path.display());
                }
            }

            if !plan.bookmarks.is_empty() {
                println!("\nBookmarks:");
                for bm in &plan.bookmarks {
                    println!("  {} → unit {}", bm.name, bm.work_unit_id);
                }
            }
        }

        // Default: repo inspection mode
        _ => {
            let path = Path::new(&args[1]);
            println!("jj-engine: inspecting repo at {:?}", path);

            let session = match session::RepoSession::load(path).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error loading repo session: {}", e);
                    std::process::exit(1);
                }
            };

            match repo_inspector::inspect_from_session(&session).await {
                Ok(state) => {
                    println!("\n--- Repo State ---");
                    println!("root:              {:?}", state.root);
                    println!("commit id:         {}", state.current_commit_id);
                    println!("change id:         {}", state.current_change_id);
                    println!(
                        "description:       {}",
                        state.current_description.as_deref().unwrap_or("(none)")
                    );
                    println!("empty commit:      {}", state.is_empty_commit);
                    println!("has changes:       {}", state.has_changes);
                    println!("safe to rewrite:   {}", state.is_safe_to_rewrite);
                    println!("has remote:        {}", state.has_remote);

                    println!("\nModified files:");
                    if state.modified_files.is_empty() {
                        println!("  (none)");
                    } else {
                        for f in &state.modified_files {
                            println!("  {}", f.display());
                        }
                    }

                    println!("\nUntracked files:");
                    if state.untracked_files.is_empty() {
                        println!("  (none)");
                    } else {
                        for f in &state.untracked_files {
                            println!("  {}", f.display());
                        }
                    }

                    println!("\nConflicted files:");
                    if state.conflicted_files.is_empty() {
                        println!("  (none)");
                    } else {
                        for f in &state.conflicted_files {
                            println!("  {}", f.display());
                        }
                    }

                    println!("\nBookmarks:");
                    if state.bookmarks.is_empty() {
                        println!("  (none)");
                    } else {
                        for b in &state.bookmarks {
                            println!(
                                "  {} | tracked: {} | ahead: {} | behind: {}",
                                b.name, b.is_tracked, b.is_ahead_of_remote, b.is_behind_remote,
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn print_tree(
    path: &semantic::EntityPath,
    graph: &work_inference::DependencyGraph,
    depth: usize,
    visited: &mut std::collections::HashSet<semantic::EntityPath>,
) {
    let indent = "  ".repeat(depth);
    let name = format!(
        "{}::{}",
        path.file.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        path.name
    );

    if visited.contains(path) {
        println!("{}{}  (seen)", indent, name);
        return;
    }

    println!("{}{}", indent, name);
    visited.insert(path.clone());

    for dep in graph.dependencies_of(path) {
        print_tree(dep, graph, depth + 1, visited);
    }
}
