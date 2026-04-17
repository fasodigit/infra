# FASO DIGITALISATION — SPIRE mTLS

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Trust domain : `faso.gov.bf`

## SVID rotation : 24h (renouvelé 12h avant expiration).

## Workloads enregistrés

| SPIFFE ID | Service |
|-----------|---------|
| `spiffe://faso.gov.bf/ns/default/sa/kaya` | KAYA (6380) |
| `spiffe://faso.gov.bf/ns/default/sa/armageddon` | ARMAGEDDON gateway |
| `spiffe://faso.gov.bf/ns/default/sa/auth-ms` | auth-ms |
| `spiffe://faso.gov.bf/ns/default/sa/poulets-api` | poulets-api |
| `spiffe://faso.gov.bf/ns/default/sa/notifier-ms` | notifier-ms |

## Déploiement (Helm K8s)

```bash
helm repo add spiffe https://spiffe.github.io/helm-charts/
helm install spire-server spiffe/spire-server --namespace spire --create-namespace \
  -f INFRA/spire/server/values.yaml
helm install spire-agent spiffe/spire-agent --namespace spire \
  -f INFRA/spire/agent/values.yaml
```

## Enregistrement d'un workload

```bash
kubectl -n spire exec spire-server-0 -- \
  /opt/spire/bin/spire-server entry create \
    -spiffeID spiffe://faso.gov.bf/ns/default/sa/<workload> \
    -parentID spiffe://faso.gov.bf/ns/spire/sa/spire-agent \
    -selector k8s:ns:default \
    -selector k8s:sa:<workload> \
    -ttl 86400
```

## Intégration Rust (KAYA, ARMAGEDDON)

```toml
# Cargo.toml
spiffe = "0.4"
arc-swap = "1"
```

```rust
use spiffe::workload_api::client::WorkloadApiClient;
let client = WorkloadApiClient::new_from_path("/run/spire/sockets/agent.sock").await?;
let mut stream = client.stream_x509_contexts().await?;
while let Some(ctx) = stream.next().await {
    // Reload rustls::ServerConfig via ArcSwap — no restart required.
}
```

## Intégration Java (auth-ms, poulets, notifier)

```xml
<dependency>
  <groupId>io.spiffe</groupId>
  <artifactId>spiffe-java-sdk</artifactId>
  <version>0.8.3</version>
</dependency>
```

Bean `X509SourceBean` + reload `SslContext` à chaque callback `onUpdate`.

## Monitoring expiration

`scripts/check-svid-expiry.sh` : vérifie toutes les 4h (workflow `.github/workflows/spire-monitoring.yml`). Alerte si < 72h.

Prometheus metric : `spire_svid_expiry_seconds{workload=}`.

## Alerts Prometheus

```yaml
groups:
  - name: spire
    rules:
      - alert: SpireSvidExpiringSoon
        expr: spire_svid_expiry_seconds < 259200
        for: 15m
        labels: { severity: warning }
      - alert: SpireSvidExpiringCritical
        expr: spire_svid_expiry_seconds < 86400
        for: 5m
        labels: { severity: critical }
      - alert: SpireSvidFetchErrors
        expr: increase(spire_agent_manager_svid_rotate_failures_total[10m]) > 0
        labels: { severity: critical }
```

## Debugging

```bash
# Inspect SVID content
kubectl -n spire exec -ti spire-agent-xxx -- \
  /opt/spire/bin/spire-agent api fetch x509 \
    -socketPath /run/spire/sockets/agent.sock

# List registration entries
kubectl -n spire exec -ti spire-server-0 -- \
  /opt/spire/bin/spire-server entry show
```
