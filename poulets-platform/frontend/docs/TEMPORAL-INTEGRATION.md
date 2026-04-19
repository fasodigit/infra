<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# Temporal.io — intégration Poulets BF

## Pourquoi Temporal.io

Référence : `INFRA/docs/v3.1-souverain/GUIDE-ARCHITECTURAL-v3.1-SOUVERAIN.md` §4.
Temporal.io est **acté comme orchestrateur souverain FASO** pour :

- Sagas compensatoires (rollback distribué)
- Timers longs (retries exponentiels, délais jours/semaines)
- Four-eyes (double validation humaine)
- Workflows humains asynchrones
- Purges légales programmées (RGPD)

**Remplace explicitement** : `workflow-orchestrator` (Netflix Conductor, exclu).

## Les 17 features Temporal applicables à Poulets BF

### Priorité P0 — Indispensables pour le MVP workflow

| # | Feature | API / Décorateur | Usage Poulets |
|---|---|---|---|
| 1 | **Workflows durables** | `@WorkflowInterface` / `@WorkflowMethod` | `OrderWorkflow` (1-7j) · `HalalCertificationWorkflow` (2-30j) · `MfaOnboardingWorkflow` (7-30j) · `LotGrowthWorkflow` (45-60j) · `DisputeSaga` (3-14j) · `AnnouncePublishWorkflow` (<5min) |
| 2 | **Activities** (retries + timeouts) | `@ActivityInterface` + `RetryOptions` + 4 timeouts (schedule-to-start/close, start-to-close, heartbeat) | `SendEmailActivity` · `ReservePouletsActivity` · `UpdateReputationActivity` · `CallEcCertificateRendererActivity` · `ArchiveToWormActivity` |
| 3 | **Signals** | `@SignalMethod` | `eleveurConfirmsOrder(orderId)` · `adminApproveStep(stepId)` · `fourEyesApprove(adminId)` · `clientAcceptsDelivery(orderId)` |
| 4 | **Queries** (lecture sync) | `@QueryMethod` | `getOrderState()` · `getHalalProgress()` · `getActiveApprovals()` — consommés par `/admin/workflows/:id` |
| 5 | **Timers longs** | `Workflow.sleep(Duration)` — zéro ressource consommée | Relance MFA J+3/J+7/J+30 · Expiration halal T+180j · Timeout AAL2 30min |
| 7 | **Sagas compensatoires** | Compensation chain dans `@WorkflowMethod` | `DisputeSaga` (refund OR uphold) · `OrderSaga` (releasePoulets si paiement échoue) |

### Priorité P1 — Production hardening

| # | Feature | API / Décorateur | Usage Poulets |
|---|---|---|---|
| 6 | **Cron Workflows** | `@CronSchedule("0 0 * * *")` | Purge légale quotidienne (RGPD) · ETL stats nightly · Rappel pesées hebdo éleveurs |
| 8 | **Child Workflows** | `Workflow.newChildWorkflowStub()` | `OrderWorkflow` lance `PaymentChildWorkflow` + `LogisticsChildWorkflow` en parallèle |
| 9 | **Continue-as-New** | `Workflow.continueAsNew()` | Workflows cron qui bouclent (évite history infini, reset après 1000 itérations) |
| 10 | **Workflow Versioning** | `Workflow.getVersion(changeId, minVersion, maxVersion)` | Déployer halal 6→7 étapes sans casser les workflows en vol |
| 12 | **Search Attributes** | Custom attrs + ElasticSearch intégré | Recherche « workflows halal de Kassim Ouédraogo en cours » depuis `/admin/workflows` |
| 15 | **Heartbeats** | `Activity.getExecutionContext().heartbeat(details)` | `GenerateCertificateActivity` heartbeat toutes les 5s pour PDFs batch volumineux (éviter timeout start-to-close) |
| 17 | **Namespaces multi-tenant** | `WorkflowClientOptions.namespace("poulets")` | Isolation `poulets` / `etat-civil` / `sogesy` dans même cluster Temporal |

### Priorité P2 — Opérations avancées

| # | Feature | API / Décorateur | Usage Poulets |
|---|---|---|---|
| 11 | **Temporal Schedules** (v1.18+) | `ScheduleClient.createSchedule()` — pause/resume, backfill, jitter | Nightly backups KAYA · Renouvellement certif halal (J-30 email) |
| 13 | **Interceptors** client + worker | `WorkerInterceptor`, `WorkflowClientInterceptor` | Tracing OTLP vers Jaeger · Audit log automatique des signals admin (conformité) |
| 14 | **Worker Versioning** (build IDs) | `WorkerOptions.buildIdForVersioning(id)` | Déploiement blue-green sans downtime |
| 16 | **Local Activities** | `Workflow.newLocalActivityStub()` — zéro overhead scheduling | Checksums, génération IDs, validation format (activities ultra-rapides <100ms) |

## Architecture Poulets avec Temporal

```
┌─────────────────────────────────────────────────────────────┐
│  UI Poulets (Angular 21)                                    │
│  /admin/workflows  ←── déjà livré J12                        │
└─────────────────────┬───────────────────────────────────────┘
                      │ HTTP REST
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  poulets-bff (Spring Boot)                                  │
│  • 6 endpoints /api/admin/workflows/*                       │
│  • Temporal Java SDK client (io.temporal:temporal-sdk)      │
└─────────────────────┬───────────────────────────────────────┘
                      │ gRPC
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  Temporal Cluster (podman-compose.temporal.yml)             │
│  • Frontend service (gRPC :7233)                            │
│  • History, Matching, Worker services                       │
│  • Temporal UI (:8088)                                      │
│  • Persistence : Postgres (DB existante FASO)               │
└─────────────────────┬───────────────────────────────────────┘
                      │ task queues
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  Workers Java (process poulets-bff OU détachés)             │
│  Implémentent les Activities :                              │
│  • SendEmailActivity  → notify-ms                           │
│  • ReservePouletsActivity → poulets-api                     │
│  • CallEcCertificateRendererActivity → impression-service   │
│  • UpdateReputationActivity → poulets-api                   │
│  • ArchiveToWormActivity → impression-service               │
└─────────────────────────────────────────────────────────────┘
```

## Workflows identifiés

### `OrderWorkflow` (P0)

```java
@WorkflowInterface
public interface OrderWorkflow {
    @WorkflowMethod
    OrderResult processOrder(OrderInput input);

    @SignalMethod
    void eleveurConfirms(String eleveurId);

    @SignalMethod
    void clientAcceptsDelivery();

    @QueryMethod
    OrderState getState();
}
```

- **Étapes** : `sendConfirmationToEleveur` → timer 24h `awaitEleveurConfirmation`
  → `reservePoulets` → `schedulePickup` → `markDelivered` → `updateReputation`
- **Compensations** : `refundPayment` + `releaseStock` si paiement échoue après reservation
- **Durée typique** : 1–7 jours (P99 ≈ 7j selon la géo éleveur/client)

### `HalalCertificationWorkflow` (P0)

```java
@WorkflowInterface
public interface HalalCertificationWorkflow {
    @WorkflowMethod
    HalalCertResult process(HalalCertInput input);

    @SignalMethod
    void adminApproveStep(int step, String adminId);

    @SignalMethod
    void fourEyesApprove(String adminId);

    @QueryMethod
    HalalProgress getProgress();
}
```

- 6 étapes (élevage conforme → identification lot → abattoir agréé → sacrificateur
  présent → contrôle vet → certificat émis)
- Chaque étape : `awaitAdminApproval` (timer 14j) + signal `adminApproveStep`
- Étape 6 nécessite **four-eyes** : 2 admins différents doivent signaler

### `MfaOnboardingWorkflow` (P0)

- Timer J+3 → `sendMfaReminderEmail` si status incomplet
- Timer J+7 → `sendSecondReminder` + snooze notifications UI
- Timer J+30 → `lockAccount` si toujours incomplet → signal admin

### `LotGrowthWorkflow` (P0)

- Cron weekly : `sendPeseeReminder` à l'éleveur
- Signal `peseeRecorded(weight, age)` — poursuit ou boucle
- Si absence 2 semaines consécutives : `notifyAdminAlert`
- Continue-as-New après 8 semaines (évite history infini)

### `DisputeSaga` (P0 compensation)

```java
@WorkflowMethod
DisputeResult resolve(DisputeInput input) {
    // 1. Escrow client payment
    Activity.reserveEscrow(orderId);
    try {
        // 2. Investigation (human)
        Workflow.await(() -> adminDecisions.size() >= 2);

        if (decision == REFUND) {
            // 3a. Refund + release + revoke reputation
            Activity.refundClient(orderId);
            Activity.releaseEscrow(orderId);
            Activity.revokeReputationPoint(eleveurId);
        } else {
            // 3b. Release to éleveur
            Activity.releaseEscrowToEleveur(orderId);
        }
    } catch (CanceledFailure e) {
        Activity.releaseEscrowToNeutral(orderId); // fallback
        throw e;
    }
}
```

### `AnnouncePublishWorkflow` (P0)

- Scan automatique `moderationScanActivity` (détection texte inapproprié, image NSFW)
- Si flag → `requireHumanModeration` (four-eyes via moderation-queue)
- Si OK → `publishAnnounce` + notify followers
- Durée typique : <5 min si auto-approuvé, jusqu'à 24h si escalade humaine

## Migration depuis workflow-orchestrator (Netflix Conductor)

| Concept Conductor | Équivalent Temporal |
|---|---|
| JSON Workflow Definition | `@WorkflowMethod` Java class |
| Task | `@ActivityMethod` Java method |
| HTTP task worker | Activity standard (`ActivityInterface`) |
| Sub-workflow | Child Workflow |
| Fork/Join | `Async.function()` + `Promise.allOf()` |
| Wait task | `Workflow.sleep()` ou `Workflow.await()` |
| SIMPLE task | Local Activity |
| DECISION task | Java `if/switch` dans `@WorkflowMethod` |

**Plan de migration** (~3-5 jours-dev) :
1. **Setup** (J1 matin) : `io.temporal:temporal-sdk` dans `poulets-bff/pom.xml` +
   `podman-compose.temporal.yml` avec Postgres persistence
2. **MVP** (J1 après-midi → J2) : `OrderWorkflow` (le plus critique métier)
3. **Signals UI** (J2) : brancher endpoints BFF sur `/admin/workflows/:id`
   (UI déjà livrée J12)
4. **Workflows restants** (J3-J4) : Halal, MFA, Growth, Dispute, Announce
5. **Cron & Schedules** (J4 après-midi) : migrer les `@Scheduled` Spring
6. **Versioning + observabilité** (J5) : production-hardening

## Setup local

```yaml
# INFRA/docker/compose/podman-compose.temporal.yml
services:
  temporal-server:
    image: temporalio/auto-setup:1.24.0
    ports: ["7233:7233"]
    environment:
      DB: postgres12
      DB_PORT: 5432
      POSTGRES_SEEDS: faso-postgres
      POSTGRES_USER: temporal
      POSTGRES_PWD: "${TEMPORAL_DB_PASSWORD:-temporal_dev}"
      NAMESPACES: "poulets,etat-civil,sogesy,default"
    networks: [faso-net]

  temporal-ui:
    image: temporalio/ui:2.25.0
    ports: ["8088:8080"]
    environment:
      TEMPORAL_ADDRESS: temporal-server:7233
      TEMPORAL_CORS_ORIGINS: "http://localhost:4801"
    depends_on: [temporal-server]
    networks: [faso-net]
```

Commande :
```bash
cd INFRA/docker/compose
podman-compose -f podman-compose.yml -f podman-compose.temporal.yml up -d temporal-server temporal-ui
```

Puis ouvrir http://localhost:8088 — UI Temporal native pour inspection
(complémentaire à `/admin/workflows` dans Poulets qui est une vue admin
de haut niveau).

## Références code

- Frontend UI : `src/app/features/admin/workflows/` (déjà livré J12)
- Service TS : `src/app/features/admin/workflows/services/temporal-workflows.service.ts`
- Types : `src/app/features/admin/workflows/models.ts`
- SDK Java à ajouter : `io.temporal:temporal-sdk:1.24.0`
- Doc officielle : https://docs.temporal.io/develop/java
- Namespaces multi-tenant : https://docs.temporal.io/namespaces

## Ce que Temporal **n'apporte pas**

- **Pas de broker event-driven** → continuer à utiliser Redpanda pour les
  événements légers (notif live, streams)
- **Pas de stockage métier** → toujours Postgres/KAYA pour l'état applicatif
- **Pas de UI utilisateur final** → Temporal UI réservée aux admins/opérateurs

## Conformité souveraineté

- **Self-hosted** obligatoire (pas de Temporal Cloud)
- Persistence Postgres déjà dans la stack FASO
- Licence MIT compatible AGPL-3.0-or-later
- Compatible v3.1-souverain (référencé explicitement dans le guide)
