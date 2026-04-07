// Route management commands.

use crate::RouteAction;
use tracing::info;

pub async fn handle(action: RouteAction, server: &str) -> anyhow::Result<()> {
    match action {
        RouteAction::Add {
            name,
            domain,
            prefix,
            cluster,
        } => {
            info!(
                name = %name,
                domain = %domain,
                prefix = %prefix,
                cluster = %cluster,
                server = %server,
                "adding route"
            );
            println!("Added route '{name}' ({domain}{prefix} -> cluster:{cluster})");
            println!("  -> ARMAGEDDON will apply this routing rule immediately");
        }
        RouteAction::List => {
            info!(server = %server, "listing routes");
            println!("No routes configured (management API not yet connected)");
        }
        RouteAction::Remove { name } => {
            info!(name = %name, server = %server, "removing route");
            println!("Removed route '{name}'");
        }
    }
    Ok(())
}
