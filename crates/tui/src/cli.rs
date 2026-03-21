// Plain CLI mode (--no-tui)
use anyhow::Result;

pub enum CliCommand {
    List,
    Status,
}

pub async fn run(command: Option<CliCommand>) -> Result<()> {
    match command {
        Some(CliCommand::List) => {
            println!("CLI list — not yet implemented");
        }
        Some(CliCommand::Status) => {
            println!("CLI status — not yet implemented");
        }
        None => {
            println!("CLI mode — not yet implemented");
        }
    }
    Ok(())
}
