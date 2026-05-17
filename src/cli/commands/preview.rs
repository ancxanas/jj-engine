pub fn run(json: bool) -> anyhow::Result<()> {
    crate::cli::commands::analyze::run(json)
}
