//! rai-daemon — the /bg supervisor binary (stub). Communicates with the CLI via
//! Unix socket / JSON-RPC. Manages background agent tasks (roster, idle eviction,
//! pinning, crash-respawn, rolling upgrades).
fn main() -> anyhow::Result<()> {
    println!("rai-daemon (stub)");
    Ok(())
}
