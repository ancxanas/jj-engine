mod decision;
mod executor;
mod explainer;
mod policy;
mod repo_inspector;
mod semantic;
mod work_inference;

use std::path::Path;

fn main() {
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

            // Build "before" snapshot from committed tree
            let before = match repo_inspector::committed_snapshot(repo_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error building committed snapshot: {}", e);
                    std::process::exit(1);
                }
            };

            println!("Committed snapshot: {} entities", before.entities.len());

            // Build "after" snapshot from working copy files
            let after = match semantic::snapshot_project(repo_path) {
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

        // Default: repo inspection mode
        _ => {
            let path = Path::new(&args[1]);
            println!("jj-engine: inspecting repo at {:?}", path);
            match repo_inspector::inspect(path) {
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
