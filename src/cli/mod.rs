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
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => commands::init::run(),
        Commands::Analyze => commands::analyze::run(cli.json),
        Commands::Preview => commands::preview::run(cli.json),
        Commands::Commit { auto } => commands::commit::run(auto, cli.json),
        Commands::Explain { change_id } => commands::explain::run(&change_id),
        Commands::Status => commands::status::run(cli.json),
    }
}
