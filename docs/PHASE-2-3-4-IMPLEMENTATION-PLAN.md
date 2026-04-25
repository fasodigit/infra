<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 FASO DIGITALISATION — Ministere du Numerique, Burkina Faso -->

# FASO DIGITALISATION — Phase 2-3-4 Implementation Plan

**Version**: 1.0
**Date**: 2026-04-24
**Status**: Approved for execution
**Prerequisite**: Phase 1 fully committed (see `docs/PHASE-1-STATUS.md`)
**Horizon**: Weeks 7-30+ (2026-05 through 2026-10)

## Architecture Context

| Layer | Component | Technology | Notes |
|-------|-----------|------------|-------|
| Gateway | **ARMAGEDDON** | Rust (Pingora) | Sovereign -- replaces Envoy |
| In-Memory DB | **KAYA** | Rust, RESP3 port 6380 | Sovereign -- replaces Redis |
| Backend | poulets-api | Java 21, Spring Boot 3.4.4, DGS 9.2.2 | GraphQL + REST |
| Backend | auth-ms | Java 21, Spring Boot 3.4.4, DGS 9.2.2 | Auth management plane |
| Backend | notifier-ms | Java 21, Spring Boot 3.4.4 | Kafka consumer, email dispatch |
| Workflow | workflow-engine | Java 21, Temporal SDK 1.27.0 | 6 workflows defined, 1 impl |
| BFF | poulets-bff | Next.js 16, port 4800 | Server-side rendering, session |
| Frontend | poulets-frontend | Angular 21, port 4801 | Apollo Client, Material |
| Message Bus | Redpanda | Kafka-compatible | Topics per domain |
| Persistence | PostgreSQL 17 | Port 5432 | Flyway migrations |
| Secrets | HashiCorp Vault | Port 8200 | AppRole + Transit + DB engine |
| Service Discovery | Consul | Port 8500 | Service mesh registration |
| Feature Flags | GrowthBook | Port 3100, MongoDB backend | REST API, KAYA cache |
| Orchestration | podman-compose | OCI runtime | **Never** docker-compose |

---

## PHASE 2 -- "Operate with Confidence" (Weeks 7-12)

### 2.1 GrowthBook Kill Switches (1 week)

**Current State**: Partial custom integration exists in poulets-api
only. `FeatureFlagsService.java` does manual REST calls to GrowthBook
with KAYA caching (TTL 30s). No official GrowthBook Java SDK used. No
integration in auth-ms or notifier-ms. BFF has no flag evaluation logic.

**Prerequisites**:

- Phase 1 Java hardening committed (Resilience4j, Vault integration)
- GrowthBook container running (`growthbook/podman-compose.growthbook.yml`)
- KAYA running on port 6380

**Files to Create**:

| File | Purpose |
|------|---------|
| `shared/growthbook-config/pom.xml` | New shared Maven module |
| `shared/growthbook-config/src/.../GrowthBookAutoConfiguration.java` | Spring Boot auto-config |
| `shared/growthbook-config/src/.../GrowthBookProperties.java` | `@ConfigurationProperties` |
| `shared/growthbook-config/src/.../KayaCachingFeatureRepository.java` | KAYA-backed SDK feature repo |
| `shared/growthbook-config/src/.../FasoFeatureFlags.java` | Typed enum of 10 kill-switch flags |
| `shared/growthbook-config/src/.../FeatureFlagMetrics.java` | Micrometer counter |
| `poulets-platform/bff/src/lib/feature-flags.ts` | BFF GrowthBook Node SDK wrapper |
| `observability/grafana/dashboards/feature-flags.json` | Grafana dashboard JSON |

**Files to Modify**:

| File | Change |
|------|--------|
| `shared/pom.xml` | Add `<module>growthbook-config</module>` |
| `poulets-platform/backend/pom.xml` | Add growthbook-config dependency |
| `auth-ms/pom.xml` | Add growthbook-config dependency |
| `notifier-ms/notifier-core/pom.xml` | Add growthbook-config dependency |
| `poulets-platform/bff/package.json` | Add `@growthbook/growthbook@^1.3.0` |

**Dependencies**:

| Artifact | Version | Scope |
|----------|---------|-------|
| `io.growthbook.sdk-java:growthbook-sdk-java` | `0.9.0` | compile |
| `@growthbook/growthbook` (npm) | `^1.3.0` | BFF runtime |

**10 Kill-Switch Flags**:

| Flag Key | Default | Purpose |
|----------|---------|---------|
| `poulets.checkout-enabled` | `true` | Disable checkout during payment incidents |
| `poulets.mobile-money-enabled` | `true` | Kill-switch for Mobile Money |
| `poulets.new-listing-enabled` | `true` | Freeze new poulet listings |
| `auth.webauthn-beta` | `false` | Progressive WebAuthn rollout |
| `auth.sms-otp-enabled` | `true` | Kill-switch SMS OTP provider |
| `notifier.email-enabled` | `true` | Disable all email dispatch |
| `notifier.sms-enabled` | `true` | Disable all SMS dispatch |
| `armageddon.response-cache-enabled` | `true` | Kill gateway response cache |
| `poulets.graphql-subscriptions` | `false` | Progressive subscription rollout |
| `platform.maintenance-mode` | `false` | Global maintenance page |

**Risk**: LOW. Additive, backward-compatible.
**Rollback**: Remove shared module dependency, revert to existing FeatureFlagsService.
**Monitoring**: `feature_flag_evaluations_total{flag,result,service}`, cache hit ratio, alert on GrowthBook unreachable >5min.

---

### 2.2 Application Rate Limiting (1 week)

**Current State**: ARMAGEDDON has proxy-level rate limiting. No
application-level per-user rate limiting in Java services.

**Goal**: `@RateLimited` annotation backed by KAYA distributed counters.

**Files to Create**:

| File | Purpose |
|------|---------|
| `shared/security-config/src/.../ratelimit/RateLimited.java` | Custom annotation |
| `shared/security-config/src/.../ratelimit/RateLimitAspect.java` | AOP aspect using KAYA INCR+EXPIRE |
| `shared/security-config/src/.../ratelimit/RateLimitProperties.java` | Configurable per-endpoint limits |
| `shared/security-config/src/.../ratelimit/RateLimitExceededException.java` | HTTP 429 exception |

**KAYA Key Pattern**: `rl:{service}:{endpoint}:{userId}:{window}`

**Default Limits**:

| Endpoint Pattern | Window | Max | Scope |
|-----------------|--------|-----|-------|
| `mutation.createCommande` | 1min | 5 | per-user |
| `mutation.registerEleveur` | 1h | 3 | per-IP |
| `mutation.rotateJwtKeys` | 1h | 1 | per-admin |
| `*` (global fallback) | 1min | 100 | per-user |

**Risk**: LOW-MEDIUM. Fail-open if KAYA unavailable.
**Rollback**: `faso.rate-limit.enabled=false`.
**Monitoring**: `rate_limit_exceeded_total{service,endpoint}`, alert on >100 rejections/min.

---

### 2.3 Automatic Secrets Rotation (2-3 weeks)

**Current State**: Vault scripts exist (Phase 1). Spring Cloud Vault
bootstrap configured. No automatic rotation scheduling.

**Goal**: Vault Agent sidecar for K8s, DB credential rotation (1h TTL),
JWT key rotation (24h), monitoring and alerting.

**Rotation Schedule**:

| Secret Type | Period | Max TTL | Method |
|-------------|--------|---------|--------|
| PostgreSQL credentials | 1 hour | 2 hours | Vault DB engine dynamic lease |
| JWT signing keys (ES384) | 24 hours | 48 hours | auth-ms + Vault Transit |
| GrowthBook API key | 90 days | 180 days | Manual via Vault KV v2 |
| SPIRE SVIDs | 24 hours | 72 hours | Existing SPIRE rotation |

**Files to Create**:

| File | Purpose |
|------|---------|
| `vault/scripts/rotate-db-credentials.sh` | Cron script for DB rotation |
| `vault/scripts/rotate-jwt-keys.sh` | Scheduled JWT key rotation |
| `vault/k8s/vault-agent-configmap.yaml` | Agent config with auto-auth |
| `vault/k8s/cronjob-jwt-rotation.yaml` | K8s CronJob for JWT rotation |
| `observability/alertmanager/rules/secrets-rotation.yml` | Alert rules |
| `observability/alertmanager/runbooks/secret-rotation-failed.md` | Runbook |

**Risk**: MEDIUM. DB credential rotation can cause brief connection drops.
**Rollback**: `spring.cloud.vault.database.enabled=false`, fall back to static credentials.
**Monitoring**: `vault_secret_lease_ttl_seconds`, alert if JWT rotation stale >36h.

---

### 2.4 Transactional Outbox Pattern (2-3 weeks)

**Current State**: `EventPublisher` in `shared/event-bus-lib` does
fire-and-forget Kafka send. Outbox schema exists at
`docs/v3.1-souverain/outbox/outbox-schema.sql`. Alert rules exist.

**Goal**: Outbox table in PostgreSQL, `@TransactionalEventListener`,
polling relay (no Debezium -- sovereignty), dead-letter queue.

**Files to Create**:

| File | Purpose |
|------|---------|
| `shared/event-bus-lib/src/.../outbox/OutboxEvent.java` | JPA entity |
| `shared/event-bus-lib/src/.../outbox/OutboxRepository.java` | Spring Data JPA repository |
| `shared/event-bus-lib/src/.../outbox/OutboxEventListener.java` | `@TransactionalEventListener(AFTER_COMMIT)` |
| `shared/event-bus-lib/src/.../outbox/OutboxRelay.java` | Scheduled polling relay -> Redpanda |
| `shared/event-bus-lib/src/.../outbox/DeadLetterHandler.java` | DLQ after max retries |
| `*/db/migration/V3__create_outbox_events.sql` | Flyway migration (x3 services) |

**Files to Modify**:

| File | Change |
|------|--------|
| `shared/event-bus-lib/.../EventPublisher.java` | Write to outbox table instead of direct Kafka |

**Relay**: Polling-based, `SELECT FOR UPDATE SKIP LOCKED`, batch 50, poll 500ms.
**Risk**: MEDIUM. Polling adds ~250ms latency. Acceptable for non-real-time events.
**Rollback**: `faso.outbox.enabled=false`, revert to direct Kafka send.
**Monitoring**: `outbox_events_pending_count`, `outbox_dead_letter_total`, alert if backlog >1000.

---

## PHASE 3 -- "Scale & Accelerate" (Weeks 13-20)

### 3.1 Temporal Workflows (4-6 weeks)

**Current State**: Temporal compose exists. workflow-engine module has
6 workflow interfaces but only `OrderWorkflowImpl` implemented.
`PouletsActivities` interface has 25+ methods but **no implementation**.

**Goal**: Implement `PouletsActivitiesImpl`, complete 5 remaining
workflows, add 3 new workflows.

**Workflows to Complete**:

| Workflow | States | Timeout |
|----------|--------|---------|
| HalalCertification | REQUESTED->INSPECTED->APPROVED/REJECTED | 7d |
| MfaOnboarding | PROMPTED->REMINDER_1->ENFORCED/LOCKED | 30d |
| LotGrowth | CREATED->WEEK_N_WEIGH_IN(repeating)->COMPLETED | 90d |
| DisputeSaga | OPENED->EVIDENCE->MEDIATION->RESOLVED | 14d |
| AnnouncePublish | SUBMITTED->MODERATION->PUBLISHED/REJECTED | 48h |

**New Workflows**:

| Workflow | States | Timeout |
|----------|--------|---------|
| PaymentEscrow | INITIATED->SELLER_CONFIRMED->DELIVERED->RELEASED | 48h/step |
| KycValidation | UPLOADED->OCR->HUMAN_REVIEW->APPROVED | 24h SLA |
| DeliveryTracking | PICKUP->IN_TRANSIT->DELIVERED->CONFIRMED | 72h |

**Risk**: HIGH. Temporal workflows are stateful and long-running.
**Rollback**: Remove workflow registration, existing sync flow continues.
**Monitoring**: `temporal_workflow_completed_total{workflow_type,status}`, alert if workflow stuck >48h.

---

### 3.2 GraphQL Federation (3-4 weeks)

**Current State**: auth-ms and poulets-api expose independent GraphQL
schemas via Netflix DGS 9.2.2. No federation, no supergraph.

**Goal**: Apollo Router as federation gateway, DGS `@key` annotations,
supergraph composition with CI schema check.

**Entity Mapping**:

| Type | Service | Key | Cross-References |
|------|---------|-----|-----------------|
| `User` | auth-ms | `id` | Referenced by Eleveur, Client |
| `Eleveur` | poulets-api | `id` | Referenced by Commande |
| `Commande` | poulets-api | `id` | References User via clientId |

**Risk**: MEDIUM-HIGH. Federation changes query execution paths.
**Rollback**: Disable Router, point back to individual endpoints.
**Monitoring**: `apollo_router_query_latency_seconds`, alert if subgraph unreachable >1min.

---

### 3.3 Dev Container (3-5 days)

**Current State**: No `.devcontainer/`. Manual setup takes ~2 days.

**Goal**: `.devcontainer/` with Java 21, Rust, Node 22, Bun, podman,
Vault CLI, Consul CLI, Angular CLI, tctl.

**Risk**: LOW. Additive, no production impact.

---

### 3.4 Contract Testing with Pact (2-3 weeks)

**Current State**: No contract testing between services. E2E only.

**Goal**: Consumer-driven contracts between BFF and Java services.

**Dependencies**: `@pact-foundation/pact@^13.0.0` (npm), `au.com.dius.pact.provider:junit5:4.6.0` (Java).

**Risk**: LOW. Test-only change.

---

### 3.5 Chaos Engineering (3 weeks)

**Current State**: 6 experiment YAMLs exist. No automation, no CI.

**Goal**: New experiments (vault-seal, disk-pressure), game day
automation, nightly CI chaos pipeline on staging.

**New Experiments**: vault-seal, disk-pressure, kaya-cluster-partition,
redpanda-leader-election, temporal-server-restart.

**Risk**: MEDIUM. Staging-only, duration-limited, manual approval for prod.

---

## PHASE 4 -- "Platform Vision" (Weeks 21+)

### 4.1 KAYA Raft Cluster (8-12 weeks)

**Current State**: `kaya/src/cluster/` has ClusterManager, HashRing,
NodeId/NodeState, raft_types (RaftRequest/RaftResponse). **No
RaftStateMachine**, no leader election, no log replication.

**Goal**: Full Raft consensus with 7 implementation steps.

**Step 1 (2w)**: RaftStateMachine -- apply log entries, maintain committed index
**Step 2 (1w)**: Snapshot transfer -- gRPC streaming, Zstd compressed
**Step 3 (1w)**: Leader forwarding -- write commands routed to leader
**Step 4 (1w)**: Quorum reads -- ReadIndex-based linearizable reads
**Step 5 (1w)**: Split-brain detection -- lease-based leader validity
**Step 6 (2w)**: Membership changes -- joint consensus add/remove node
**Step 7 (2w)**: Multi-shard Raft -- separate Raft group per shard

**Target**: 3-node cluster, replication factor 3, failover <5s.

**Dependencies**: `bincode 2.0`, `tonic 0.12`, `prost 0.13`, `rand 0.8`.

**Risk**: VERY HIGH. Distributed consensus is notoriously difficult.
**Rollback**: `cluster.enabled=false`, single-node mode always available.
**Monitoring**: `kaya_raft_term`, `kaya_raft_leader`, `kaya_raft_commit_latency_seconds`,
alert on no leader >10s or split-brain.

---

### 4.2 Multi-Tenancy (4-6 weeks)

**Current State**: Single-tenant. `FasoEvent` has `tenantId` field.
Temporal compose defines namespaces for 4 tenants.

**Goal**: Schema-per-tenant PostgreSQL, tenant context propagation,
KAYA namespace, ARMAGEDDON domain routing.

**Tenant Routing**:

| Tenant | Domain | KAYA Prefix | PG Schema | Temporal NS |
|--------|--------|-------------|-----------|-------------|
| poulets | poulets.faso.bf | `poulets:` | `poulets` | `poulets` |
| etat-civil | etatcivil.faso.bf | `ec:` | `etat_civil` | `etat-civil` |
| sogesy | sogesy.faso.bf | `sogesy:` | `sogesy` | `sogesy` |
| hospital | hospital.faso.bf | `hospital:` | `hospital` | `hospital` |

**Risk**: HIGH. Data leak between tenants is critical security risk.
**Rollback**: `faso.tenant.enabled=false`, default to `poulets` tenant.
**Monitoring**: `faso_tenant_request_total{tenant,service}`, alert on missing tenant context.

---

## Sequencing Summary

| Week | Phase | Item | Effort | Risk |
|------|-------|------|--------|------|
| 7 | 2 | 2.1 GrowthBook Kill Switches | 1w | LOW |
| 8 | 2 | 2.2 Application Rate Limiting | 1w | LOW-MED |
| 9-11 | 2 | 2.3 Secrets Rotation | 2-3w | MEDIUM |
| 10-12 | 2 | 2.4 Outbox Pattern | 2-3w | MEDIUM |
| 13-18 | 3 | 3.1 Temporal Workflows | 4-6w | HIGH |
| 15-18 | 3 | 3.2 GraphQL Federation | 3-4w | MED-HIGH |
| 16 | 3 | 3.3 Dev Container | 3-5d | LOW |
| 17-19 | 3 | 3.4 Contract Testing Pact | 2-3w | LOW |
| 18-20 | 3 | 3.5 Chaos Engineering | 3w | MEDIUM |
| 21-32 | 4 | 4.1 KAYA Raft Cluster | 8-12w | VERY HIGH |
| 25-30 | 4 | 4.2 Multi-Tenancy | 4-6w | HIGH |

**Parallel Tracks**:

- 2.1 + 2.2 (week 7-8): Independent shared modules
- 2.3 + 2.4 (week 9-12): Independent subsystems
- 3.1 + 3.2 (week 13-18): Independent codebases
- 3.3 + 3.4 (week 16-19): Tooling vs testing

**Critical Path**: Phase 1 -> 2.1 -> 2.4 -> 3.1 -> 4.2

---

## Sovereignty Reminders

- **KAYA** replaces Redis/DragonflyDB. Lettuce connects on port 6380.
- **ARMAGEDDON** replaces Envoy/NGINX. All gateway logic is Rust/Pingora.
- **podman-compose** is the only orchestrator. Never `docker-compose`.
- **Containerfile** is the only image format. Never `Dockerfile`.
- All files carry `SPDX-License-Identifier: AGPL-3.0-or-later`.
- Secrets live in Vault, never in repository files.

---

*Generated 2026-04-24 by Phase 2-3-4 planning agent*
