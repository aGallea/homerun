use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "homerun", about = "HomeRun — GitHub Actions self-hosted runner manager")]
struct Cli {
    /// Disable TUI, use plain CLI output
    #[arg(long)]
    no_tui: bool,

    /// Subcommand for CLI mode
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// List all runners
    List,
    /// Show runner and system status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_tui || cli.command.is_some() {
        println!("CLI mode — not yet implemented");
    } else {
        println!("TUI mode — not yet implemented");
    }

    Ok(())
}
