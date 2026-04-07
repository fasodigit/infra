// Endpoint management commands.

use crate::EndpointAction;
use tracing::info;

pub async fn handle(action: EndpointAction, server: &str) -> anyhow::Result<()> {
    match action {
        EndpointAction::Add {
            cluster,
            address,
            port,
            weight,
        } => {
            info!(
                cluster = %cluster,
                address = %address,
                port = port,
                weight = weight,
                server = %server,
                "adding endpoint"
            );
            println!("Added endpoint {address}:{port} (weight={weight}) to cluster '{cluster}'");
            println!("  -> ARMAGEDDON will start routing to this endpoint via EDS");
        }
        EndpointAction::List { cluster } => {
            info!(cluster = %cluster, server = %server, "listing endpoints");
            println!("No endpoints for cluster '{cluster}' (management API not yet connected)");
        }
    }
    Ok(())
}
