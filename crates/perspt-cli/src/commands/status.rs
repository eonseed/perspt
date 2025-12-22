//! Status command - show agent session status

use anyhow::Result;

/// Show current agent status
pub async fn run() -> Result<()> {
    println!("SRBN Agent Status");
    println!("─────────────────────────────");

    // Check for active session
    // In a real implementation, this would query the ledger
    let has_active_session = false;

    if has_active_session {
        println!("Session: abc123-def456");
        println!("Status: Running");
        println!("  Current node: auth-2");
        println!("  Completed: 3/7 nodes");
        println!("  Energy: V(x) = 0.234");
        println!("  Mode: Balanced");
    } else {
        println!("No active agent session");
        println!();
        println!("Start a new session with:");
        println!("  perspt agent \"<task description>\"");
    }

    Ok(())
}
