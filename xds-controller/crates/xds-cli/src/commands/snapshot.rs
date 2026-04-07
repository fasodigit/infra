// Snapshot display command.

use tracing::info;

pub async fn handle(server: &str) -> anyhow::Result<()> {
    info!(server = %server, "fetching configuration snapshot");
    // TODO: Fetch from xDS Controller management API.
    println!("Configuration Snapshot");
    println!("  Version: (not connected)");
    println!("  Clusters: 0");
    println!("  Endpoints: 0");
    println!("  Routes: 0");
    println!("  Listeners: 0");
    println!("  Certificates: 0");
    println!();
    println!("State stored in KAYA Collections.");
    println!("Connect to xDS Controller at {server} for live data.");
    Ok(())
}
