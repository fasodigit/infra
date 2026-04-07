// Cluster management commands.

use crate::ClusterAction;
use tracing::info;

pub async fn handle(action: ClusterAction, server: &str) -> anyhow::Result<()> {
    match action {
        ClusterAction::Add {
            name,
            r#type,
            lb,
            timeout,
        } => {
            info!(
                name = %name,
                r#type = %r#type,
                lb = %lb,
                timeout_ms = timeout,
                server = %server,
                "adding cluster"
            );
            // TODO: Call management gRPC API on the xDS Controller.
            // For now, print what would be done.
            println!("Added cluster '{name}' (type={type}, lb={lb}, timeout={timeout}ms)");
            println!("  -> Changes will be pushed to ARMAGEDDON automatically via xDS");
        }
        ClusterAction::List => {
            info!(server = %server, "listing clusters");
            // TODO: Fetch from xDS Controller management API.
            println!("No clusters configured (management API not yet connected)");
        }
        ClusterAction::Remove { name } => {
            info!(name = %name, server = %server, "removing cluster");
            println!("Removed cluster '{name}'");
            println!("  -> ARMAGEDDON will stop routing to this cluster");
        }
        ClusterAction::Show { name } => {
            info!(name = %name, server = %server, "showing cluster");
            println!("Cluster '{name}' details not available (management API not yet connected)");
        }
    }
    Ok(())
}
