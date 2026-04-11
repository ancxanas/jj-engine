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

    let path = if args.len() > 1 {
        Path::new(&args[1])
    } else {
        Path::new(".")
    };

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
                        b.name, b.is_tracked, b.is_ahead_of_remote, b.is_behind_remote
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
