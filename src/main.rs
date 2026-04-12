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
                    println!(
                        "Semantic snapshot of {}:",
                        project_path.display()
                    );
                    println!("  Files scanned: {}", snapshot.files_scanned);
                    println!("  Entities found: {}", snapshot.entities.len());

                    // Group entities by file for display
                    let mut by_file: std::collections::BTreeMap<
                        &std::path::PathBuf,
                        Vec<&semantic::EntityInfo>,
                    > = std::collections::BTreeMap::new();

                    for entity in snapshot.entities.values() {
                        by_file
                            .entry(&entity.path.file)
                            .or_default()
                            .push(entity);
                    }

                    println!();
                    for (file, file_entities) in &by_file {
                        println!(
                            "  {} ({} entities)",
                            file.display(),
                            file_entities.len()
                        );
                        for entity in file_entities {
                            println!(
                                "    [{:?}] {}",
                                entity.path.kind,
                                entity.path.name,
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
                                b.name,
                                b.is_tracked,
                                b.is_ahead_of_remote,
                                b.is_behind_remote,
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
