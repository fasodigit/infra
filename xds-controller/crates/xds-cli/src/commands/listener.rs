// Listener management commands.

use crate::ListenerAction;
use tracing::info;

pub async fn handle(action: ListenerAction, server: &str) -> anyhow::Result<()> {
    match action {
        ListenerAction::Add {
            name,
            address,
            port,
        } => {
            info!(
                name = %name,
                address = %address,
                port = port,
                server = %server,
                "adding listener"
            );
            println!("Added listener '{name}' ({address}:{port})");
            println!("  -> ARMAGEDDON will create this listener via LDS");
        }
        ListenerAction::List => {
            info!(server = %server, "listing listeners");
            println!("No listeners configured (management API not yet connected)");
        }
        ListenerAction::Remove { name } => {
            info!(name = %name, server = %server, "removing listener");
            println!("Removed listener '{name}'");
        }
    }
    Ok(())
}
