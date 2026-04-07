// Server status command.

use tracing::info;

pub async fn handle(server: &str) -> anyhow::Result<()> {
    info!(server = %server, "checking xDS Controller status");
    // TODO: Health check the xDS Controller gRPC endpoint.
    println!("xDS Controller Status");
    println!("  Server: {server}");
    println!("  Status: (management API not yet connected)");
    println!("  Connected ARMAGEDDON instances: unknown");
    println!("  SPIRE agent: unknown");
    println!("  KAYA store: unknown");
    Ok(())
}
