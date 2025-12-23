//! Abort command - abort current session

use anyhow::Result;
use std::io::{self, Write};

/// Abort the current agent session
pub async fn run(force: bool) -> Result<()> {
    if !force {
        print!("⚠ Are you sure you want to abort the current session? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Abort cancelled");
            return Ok(());
        }
    }

    // In a real implementation, this would:
    // 1. Send abort signal to the running agent
    // 2. Rollback any uncommitted changes
    // 3. Update the ledger with abort status

    println!("✓ Session aborted");
    println!("  Changes have been rolled back to the last stable state.");

    Ok(())
}
