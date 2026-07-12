pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "avcs", about = "Autonomous Version Control System")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true, help = "Output as JSON")]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Analyze,
    Preview,
    Commit {
        #[arg(long, help = "Auto-commit safe intents")]
        auto: bool,
    },
    Explain {
        change_id: String,
    },
    Status,
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    Start,
    Stop,
    Status,
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if matches!(&cli.command, Commands::Init) {
        return commands::init::run();
    }

    match cli.command {
        Commands::Init => unreachable!(),
        Commands::Analyze => commands::analyze::run(cli.json),
        Commands::Preview => commands::preview::run(cli.json),
        Commands::Commit { auto } => commands::commit::run(auto, cli.json),
        Commands::Explain { change_id } => {
            let project_root = std::env::current_dir()?;
            let repo = crate::vcs::repo::GitRepo::open(&project_root)?;
            commands::explain::run(&repo, &change_id)
        }
        Commands::Status => commands::status::run(cli.json),
        Commands::Daemon { action } => {
            let sub = match action {
                DaemonAction::Start => "start",
                DaemonAction::Stop => "stop",
                DaemonAction::Status => "status",
            };
            commands::daemon::run(sub)
        }
    }
}
