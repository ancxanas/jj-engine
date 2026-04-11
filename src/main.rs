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
            println!("root:           {:?}", state.root);
            println!("commit id:      {}", state.current_commit_id);
            println!("change id:      {}", state.current_change_id);
            println!(
                "description:    {}",
                state.current_description.as_deref().unwrap_or("(none)")
            );
            println!("empty commit:   {}", state.is_empty_commit);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
}
