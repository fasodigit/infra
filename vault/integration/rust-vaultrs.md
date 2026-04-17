<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# Intégration `vaultrs` — KAYA / ARMAGEDDON

## Cargo.toml

```toml
[dependencies]
vaultrs = "0.7"
vaultrs-login = "0.2"
```

## Authentification AppRole + lecture KV v2

```rust
use vaultrs::client::{VaultClient, VaultClientSettingsBuilder};
use vaultrs::kv2;
use vaultrs_login::{engines::approle::AppRoleLogin, LoginClient};

pub async fn load_kaya_secrets() -> anyhow::Result<KayaSecrets> {
    let client = VaultClient::new(
        VaultClientSettingsBuilder::default()
            .address(std::env::var("VAULT_ADDR")?)
            .build()?,
    )?;

    // AppRole login
    let login = AppRoleLogin {
        role_id: std::env::var("VAULT_ROLE_ID")?,
        secret_id: std::env::var("VAULT_SECRET_ID")?,
    };
    client.login("approle", &login).await?;

    // Read KV v2 under `faso/kaya/auth`
    let auth: serde_json::Value = kv2::read(&client, "faso", "kaya/auth").await?;
    let functions: serde_json::Value = kv2::read(&client, "faso", "kaya/functions").await?;

    Ok(KayaSecrets {
        auth_password: auth["password"].as_str().unwrap().to_string(),
        functions_signing_key: functions["signing_key"].as_str().unwrap().as_bytes().to_vec(),
    })
}
```

## Renewal automatique du token

`vaultrs` supporte `client.renew_self()` ; à appeler en tâche Tokio périodique (< TTL/2).

```rust
tokio::spawn({
    let client = client.clone();
    async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1800));
        loop {
            interval.tick().await;
            if let Err(e) = client.renew_self(None).await {
                tracing::warn!(err = %e, "vault token renewal failed");
            }
        }
    }
});
```

## Transit encrypt/decrypt (KAYA persistence)

```rust
use vaultrs::transit;

let ciphertext = transit::data::encrypt(
    &client,
    "transit",
    "persistence-key",
    &base64::engine::general_purpose::STANDARD.encode(plaintext),
    None,
).await?;
```

## Rechargement hot-swap

Stocker `Arc<ArcSwap<KayaSecrets>>` et rafraîchir périodiquement :

```rust
let secrets: Arc<ArcSwap<KayaSecrets>> = Arc::new(ArcSwap::from_pointee(load_kaya_secrets().await?));

tokio::spawn({
    let secrets = Arc::clone(&secrets);
    async move {
        let mut iv = tokio::time::interval(Duration::from_secs(900));
        loop {
            iv.tick().await;
            match load_kaya_secrets().await {
                Ok(s) => secrets.store(Arc::new(s)),
                Err(e) => tracing::error!(err=%e, "Vault reload failed"),
            }
        }
    }
});
```

## ARMAGEDDON — intégration similaire

`armageddon-config/src/vault.rs` (nouveau crate si volume important) :
- Lire `faso/armageddon/admin`, `github`, `redpanda-sasl`
- Exposer `VaultSecrets` dans `GatewayConfig`
- Recharger à chaque `/admin/config/reload` (axum route déjà livrée)

## Notes

- Le crate `vaultrs` suit les évolutions Vault jusqu'à 1.18+
- Alternative : `hashicorp_vault` (plus ancien, moins maintenu)
- Pour K8s auth à la place d'AppRole : `vaultrs::auth::kubernetes`
