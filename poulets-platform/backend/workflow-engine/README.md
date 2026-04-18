<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# workflow-engine — Orchestrateur Temporal.io Poulets BF

Service Spring Boot qui **remplace** `workflow-orchestrator` (Netflix Conductor
d'Etat-civil, exclu par décision architecturale FASO DIGITALISATION v3.1).

Référence : `INFRA/docs/v3.1-souverain/GUIDE-ARCHITECTURAL-v3.1-SOUVERAIN.md` §4.

## 6 Workflows livrés

| Workflow | Durée | Usage |
|---|---|---|
| `OrderWorkflow` | 1–7 jours | Commande client→éleveur avec escrow + compensation |
| `HalalCertificationWorkflow` | 2–30 jours | 6 étapes de certification halal + four-eyes étape finale |
| `MfaOnboardingWorkflow` | 7–30 jours | Relances email J+3/J+7/J+30 + verrouillage compte si incomplet |
| `LotGrowthWorkflow` | 45–60 jours | Reminders pesées hebdomadaires + alerte absence 2 semaines |
| `DisputeSaga` | 3–14 jours | Médiation four-eyes + compensation (refund OR uphold) |
| `AnnouncePublishWorkflow` | <5 min – 24h | Scan modération auto + escalade humaine si flag |

Implémentation de référence livrée : `impl/OrderWorkflowImpl.java`.
Les 5 autres workflows sont déclarés comme interfaces (`@WorkflowInterface`) ;
leur implémentation suit le même pattern (ActivityOptions + RetryOptions +
signals/queries + `Workflow.sleep()` pour les timers longs).

## Structure

```
workflow-engine/
├── pom.xml                           # Temporal SDK 1.27.0 + Spring Boot 3.4.4
├── src/main/java/bf/gov/faso/workflow/
│   ├── WorkflowEngineApplication.java   # main (@SpringBootApplication)
│   ├── activities/
│   │   └── PouletsActivities.java       # @ActivityInterface (28 méthodes)
│   ├── workflows/
│   │   ├── OrderWorkflow.java
│   │   ├── HalalCertificationWorkflow.java
│   │   ├── MfaOnboardingWorkflow.java
│   │   ├── LotGrowthWorkflow.java
│   │   ├── DisputeSaga.java
│   │   └── AnnouncePublishWorkflow.java
│   └── impl/
│       └── OrderWorkflowImpl.java       # Implémentation de référence
└── src/main/resources/
    └── application.yml                  # Port 8902, namespace Temporal "poulets"
```

## Démarrage

```bash
# 1. Démarrer Temporal cluster (cf INFRA/docker/compose/podman-compose.temporal.yml)
cd INFRA/docker/compose
podman-compose -f podman-compose.yml -f podman-compose.temporal.yml up -d temporal-server temporal-ui

# 2. Build + run workflow-engine
cd INFRA/poulets-platform/backend/workflow-engine
mvn spring-boot:run

# 3. Vérifier
curl http://localhost:8902/actuator/health
# → {"status":"UP"}

# 4. UI Temporal native
open http://localhost:8088
```

## Task queues

- `poulets-main` : workflows synchrones critiques (Order, Announce)
- `poulets-long` : workflows longs (Halal, Growth, MFA, Dispute)

Configuration auto-discovery par Spring Boot Starter Temporal —
les classes `@WorkflowInterface` sont détectées au démarrage.

## Activities

L'interface `PouletsActivities` regroupe les 28 activities utilisées par les
workflows. L'implémentation concrète (`PouletsActivitiesImpl`) doit appeler :
- `poulets-api` (orders, reservations, reputation)
- `notifier-ms` (emails, SMS, push)
- `impression-service` (generate PDF)
- `auth-ms` (MFA status, lock account)
- `event-bus` (publish events)

## Observabilité

- `/actuator/prometheus` exporte métriques :
  - `temporal_workflow_started_total{workflowType}`
  - `temporal_workflow_completed_total{workflowType,status}`
  - `temporal_activity_duration_seconds{activity}`
- Dashboards Grafana à créer sous
  `INFRA/observability/grafana/dashboards/workflow-engine.json`

## Endpoints admin (consommés par BFF)

À implémenter dans un `WorkflowAdminController` :
- `GET    /api/admin/workflows?type=&status=&actorId=`
- `GET    /api/admin/workflows/{id}`
- `GET    /api/admin/workflows/{id}/history`
- `GET    /api/admin/workflows/{id}/activities`
- `POST   /api/admin/workflows/{id}/signal`
- `POST   /api/admin/workflows/{id}/cancel`
- `POST   /api/admin/workflows/{id}/terminate`
- `GET    /api/admin/workflows/stats/latency`

Ces endpoints sont déjà proxifiés par le BFF Next.js
(`bff/src/app/api/admin/workflows/*`).

## Tests

```bash
mvn test
# Utilise temporal-testing pour simuler cluster en mémoire
```

## TODO

- [ ] Implémenter les 5 workflows restants (pattern identique à OrderWorkflowImpl)
- [ ] Implémenter `PouletsActivitiesImpl` (appels HTTP vers services FASO)
- [ ] WorkflowAdminController (endpoints admin pour BFF)
- [ ] Tests Temporal avec `TestWorkflowEnvironment`
- [ ] Dockerfile/Containerfile
- [ ] Entrée dans `podman-compose.yml` stack principale
