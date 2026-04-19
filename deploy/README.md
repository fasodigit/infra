# FASO DIGITALISATION — Kubernetes Deploy

<!-- SPDX-License-Identifier: AGPL-3.0-only -->

## Architecture Diagram

```
                        ┌─────────────────────────────────────────────────────┐
                        │          OVH GRA11 — EU Sovereign Cluster           │
                        │              (3x B3-8 nodes, k8s 1.31)              │
                        └─────────────────────────────────────────────────────┘
                                              │
              ┌───────────────────────────────┼───────────────────────────────┐
              │                               │                               │
   ┌──────────▼──────────┐      ┌─────────────▼────────────┐    ┌────────────▼───────────┐
   │   ns: gateway       │      │   ns: apps               │    │  ns: kaya              │
   │   armageddon        │──┐   │   auth-ms                │    │  StatefulSet 3x        │
   │   Deployment 3x     │  │   │   poulets-api            │    │  PVC 50Gi each         │
   │   HPA CPU>70%       │  │   │   notifier-ms            │    │  Ports 6380+6381       │
   │   Ingress TLS       │  │   │   Deployment 3x each     │    │  PodMonitor + PDB      │
   │   Canary Rollout    │  │   │   2CPU/2Gi limits        │    └────────────────────────┘
   └─────────────────────┘  │   │   Actuator probes        │
                             │   │   RollingUpdate 0 unavail│
   ┌─────────────────────┐   │   └──────────────────────────┘
   │   ns: ory-stack     │   │
   │   Kratos (IdP)      │◄──┘   ┌──────────────────────────┐
   │   Keto (AuthZ)      │       │   ns: observability      │
   │   PostgreSQL 20Gi   │       │   kube-prometheus-stack  │
   └─────────────────────┘       │   Prometheus 50Gi        │
                                 │   Grafana (grafana.faso.) │
   ┌─────────────────────┐       │   Loki 20Gi              │
   │   ns: spire         │       │   Tempo 10Gi             │
   │   Server StatefulSet│       │   OTel Collector DS      │
   │   Agent DaemonSet   │       └──────────────────────────┘
   │   trust: faso.gov.bf│
   └─────────────────────┘       ┌──────────────────────────┐
                                 │   ns: flux-system        │
   ┌─────────────────────┐       │   GitRepository          │
   │   ns: vault         │       │   Kustomization infra    │
   │   (external)        │       │   Kustomization apps     │
   │   KV faso/*         │       │   HelmReleases           │
   └─────────────────────┘       │   reconcile: 5min        │
                                 └──────────────────────────┘

   ┌──────────────────────────────────────────────────────────────┐
   │  Security layers                                             │
   │  ├── NetworkPolicy: deny-all + explicit allow per service    │
   │  ├── PodSecurityStandards: restricted on all namespaces      │
   │  ├── ExternalSecrets → Vault KV faso/*                       │
   │  ├── SPIRE mTLS: trust domain faso.gov.bf                    │
   │  └── cert-manager: TLS via letsencrypt-prod                  │
   └──────────────────────────────────────────────────────────────┘
```

## Repository Layout

```
deploy/
├── helm/
│   ├── kaya/              # StatefulSet 3x, PVC 50Gi, ports 6380+6381
│   ├── armageddon/        # Deployment 3x, HPA CPU>70%, Ingress TLS
│   ├── auth-ms/           # Java 21, 2CPU/2Gi, Actuator probes
│   ├── poulets-api/       # Java 21, 2CPU/2Gi, Actuator probes
│   ├── notifier-ms/       # Java 21, 2CPU/2Gi, Actuator probes
│   ├── ory-stack/         # Umbrella: Kratos + Keto + PostgreSQL
│   └── observability/     # Umbrella: Prometheus + Grafana + Loki + Tempo + OTel
├── gitops/
│   └── flux-system/       # Flux CD Kustomizations + HelmReleases (5min reconcile)
├── external-secrets/      # ExternalSecret → Vault KV faso/*
├── spire/                 # SPIRE server StatefulSet + agent DaemonSet
├── argo-rollouts/         # Canary 1%→10%→50%→100% with Prometheus analysis
├── terraform/
│   └── ovh-kube/          # 3 nodes B3-8, GRA11 (EU), VPC, Flux bootstrap
└── .github/
    ├── workflows/
    │   └── helm-lint.yml  # ct lint-and-install + Checkov security scan
    ├── ct.yaml            # chart-testing config
    └── kind-config.yaml   # kind cluster for CI
```

## Bootstrap (Day 0)

### Prerequisites
- `terraform >= 1.7`, `helm >= 3.16`, `flux >= 2.4`, `kubectl >= 1.31`
- OVH API credentials in environment
- Vault instance running at `https://vault.faso.gov.bf`
- SOPS age key for secret decryption

### Step 1 — Provision OVH cluster
```bash
cd deploy/terraform/ovh-kube
terraform init
terraform plan -var-file=prod.tfvars
terraform apply -var-file=prod.tfvars
# Kubeconfig exported to Vault automatically via outputs
```

### Step 2 — Configure kubectl
```bash
# Retrieve kubeconfig from Vault
vault kv get -field=kubeconfig faso/cluster/kubeconfig > ~/.kube/faso-kube
export KUBECONFIG=~/.kube/faso-kube
kubectl get nodes
```

### Step 3 — Bootstrap Flux CD
```bash
flux bootstrap github \
  --owner=fasodigitalisation \
  --repository=infra \
  --branch=main \
  --path=deploy/gitops/flux-system \
  --namespace=flux-system
```

### Step 4 — Seed Vault secrets
```bash
# All app secrets under faso/* KV path
vault kv put faso/kaya password="<generated>"
vault kv put faso/armageddon jwt-secret="<generated>"
vault kv put faso/auth-ms db-password="<generated>" jwt-secret="<generated>"
vault kv put faso/poulets-api db-password="<generated>"
vault kv put faso/notifier-ms smtp-password="<generated>" sms-api-key="<generated>"
vault kv put faso/ory-stack postgres-password="<generated>" kratos-password="<generated>" keto-password="<generated>"
vault kv put faso/observability grafana-admin-password="<generated>"
```

### Step 5 — Verify Flux reconciliation
```bash
flux get all -A
flux get kustomizations
flux get helmreleases -A
```

## Canary Rollout (armageddon)

```bash
# Update image tag to trigger canary
kubectl argo rollouts set image armageddon \
  armageddon=registry.faso.gov.bf/faso/armageddon:1.1.0 \
  -n gateway

# Watch canary progress
kubectl argo rollouts get rollout armageddon -n gateway --watch

# Manually promote (after analysis passes)
kubectl argo rollouts promote armageddon -n gateway

# Abort and rollback
kubectl argo rollouts abort armageddon -n gateway
```

## Troubleshooting

### Pod stuck in Pending
```bash
kubectl describe pod <pod> -n <ns>
# Check: resource requests vs node capacity, PVC binding, taint/toleration
kubectl get events -n <ns> --sort-by='.lastTimestamp'
```

### Flux not reconciling
```bash
flux logs --all-namespaces --level=error
flux reconcile kustomization infra --with-source
flux reconcile helmrelease kaya -n flux-system
```

### ExternalSecret not syncing
```bash
kubectl describe externalsecret <name> -n <ns>
# Check: Vault token, KV path exists, ClusterSecretStore status
kubectl get clustersecretstore vault-faso -o yaml
```

### NetworkPolicy blocking traffic
```bash
# Trace with netshoot
kubectl run netshoot --rm -it --image=nicolaka/netshoot -- bash
# From pod: curl -v http://kaya.kaya.svc.cluster.local:6380
# Check policies:
kubectl get networkpolicies -A
```

### SPIRE agent not attesting
```bash
kubectl logs -n spire daemonset/spire-agent
kubectl exec -n spire statefulset/spire-server -- \
  /opt/spire/bin/spire-server agent list
```

### HPA not scaling
```bash
kubectl describe hpa armageddon -n gateway
# Ensure metrics-server is running
kubectl top pods -n gateway
```

## Helm Chart Versions

| Chart | Version | App Version |
|-------|---------|-------------|
| kaya | 1.0.0 | 0.9.0 |
| armageddon | 1.0.0 | 1.0.0 |
| auth-ms | 1.0.0 | 1.0.0 |
| poulets-api | 1.0.0 | 1.0.0 |
| notifier-ms | 1.0.0 | 1.0.0 |
| ory-stack | 1.0.0 | 1.0.0 |
| observability | 1.0.0 | 1.0.0 |

## Security Compliance

| Control | Implementation |
|---------|---------------|
| No hardcoded secrets | ExternalSecrets → Vault KV |
| mTLS service-to-service | SPIRE + trust domain faso.gov.bf |
| Network segmentation | deny-all + explicit allow NetworkPolicies |
| Pod hardening | PodSecurityStandards restricted, readOnlyRootFS, non-root |
| TLS termination | cert-manager + letsencrypt-prod |
| Progressive delivery | Argo Rollouts canary + Prometheus analysis |
| GitOps audit trail | Flux CD, all changes via Git PRs |
| EU data sovereignty | OVH GRA11 (Gravelines, France) |
