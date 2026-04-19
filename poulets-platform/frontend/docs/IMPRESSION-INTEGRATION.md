<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# Impression — Guide d'intégration Poulets BF

## Vue d'ensemble

Le pipeline d'impression de Poulets BF repose sur 2 services Java clonés
depuis `fasodigit/Etat-civil` et rebrandés `bf.gov.faso` :

```
UI Poulets (/admin/impression)
      │ REST
      ▼
poulets-bff (proxy)
      │ REST + JWT
      ▼
impression-service :8921  (JPA + Redis + Kafka)
      │ HTTP HMAC
      ▼
ec-certificate-renderer :8920  (Playwright + Handlebars)
      │
      ▼ PDF bytes
  + archivage WORM (Postgres + hash SHA-256)
  + QR code vérifiable
```

## Sources

- `INFRA/poulets-platform/backend/services/ec-certificate-renderer/`
- `INFRA/poulets-platform/backend/services/impression-service/`
- `INFRA/poulets-platform/backend/services/ANALYSE-EC-CERTIFICATE-RENDERER.md`
- `INFRA/poulets-platform/backend/services/README.md` (dépendances à résoudre)

## Ports & config

| Service | Port HTTP | Port gRPC | Env critiques |
|---|---|---|---|
| `ec-certificate-renderer` | 8920 | — | `INTERNAL_AUTH_SECRET`, `PLAYWRIGHT_BROWSERS_PATH` |
| `impression-service` | 8921 | 9921 | `RENDERER_URL`, `INTERNAL_AUTH_SECRET`, `SPRING_DATASOURCE_URL`, `SPRING_DATA_REDIS_HOST`, `SPRING_KAFKA_BOOTSTRAP_SERVERS` |

## Démarrage local

```bash
cd INFRA/docker/compose

# Stack principale (si pas déjà démarrée)
podman-compose -f podman-compose.yml up -d faso-postgres faso-kaya faso-redpanda

# Impression stack
podman-compose -f podman-compose.yml -f podman-compose.impression.yml up -d faso-ec-renderer faso-impression

# Vérification
curl http://localhost:8920/health        # renderer health
curl http://localhost:8921/actuator/health  # impression health
```

## API

### ec-certificate-renderer (8920)

Auth : header `X-Internal-Auth: {timestamp}:{hmac_sha256_hex}`  
Secret : env `INTERNAL_AUTH_SECRET` (Vault `secret/renderer/hmac_key` en prod)

```bash
# Single render
curl -X POST http://localhost:8920/render/certificat-halal \
  -H "Content-Type: application/json" \
  -H "X-Internal-Auth: 1713432000:$(echo -n "1713432000:POST:/render/certificat-halal" | openssl dgst -sha256 -hmac "$SECRET" -hex | awk '{print $2}')" \
  -d '{"eleveurName":"Kassim Ouédraogo","lotId":"L-2026-041","quantity":48}' \
  --output certif.pdf

# Batch render → ZIP
curl -X POST http://localhost:8920/render/batch \
  -H "Content-Type: application/json" \
  -H "X-Internal-Auth: ..." \
  -d '[{"template":"certificat-halal","data":{...}}, {"template":"contrat-commande","data":{...}}]' \
  --output batch.zip
```

### impression-service (8921)

Auth : OAuth2 Resource Server (JWT émis par ORY Kratos)

```bash
# Générer un document (enqueue job)
curl -X POST http://localhost:8921/api/impression/generate \
  -H "Authorization: Bearer ${KRATOS_SESSION_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"type":"CERTIFICAT_HALAL","documentId":"L-2026-041","data":{...}}'

# Suivre la queue
curl -H "Authorization: Bearer ..." http://localhost:8921/api/impression/queue

# Récupérer le PDF généré
curl -H "Authorization: Bearer ..." http://localhost:8921/api/impression/job-001/pdf --output certif.pdf

# Vérifier un QR code
curl -X POST -H "Authorization: Bearer ..." http://localhost:8921/api/verification/qr/job-001
```

## Frontend Poulets

Routes :
- `/admin/impression` — dashboard queue (4 KPIs + table jobs)
- `/admin/impression/archives` — archives WORM avec hash SHA-256 + QR verif
- `/admin/impression/templates` — browser des templates Handlebars disponibles

Service TS : `src/app/features/admin/impression/services/impression.service.ts`
(actuellement mock, branchement réel via BFF).

## Templates Handlebars à ajouter

Dans `ec-certificate-renderer/src/main/resources/templates/` :

| Template | Variables requises |
|---|---|
| `certificat-halal.hbs` | `eleveurName`, `lotId`, `quantity`, `race`, `abattoir`, `sacrificateur`, `dateAbattage` |
| `contrat-commande.hbs` | `clientName`, `eleveurName`, `orderId`, `quantity`, `amount`, `dateCommande` |
| `recepisse-livraison.hbs` | `clientName`, `address`, `quantity`, `livreur`, `dateLivraison` |
| `attestation-elevage.hbs` | `eleveurName`, `region`, `memberSince`, `totalSales` |

Les templates actes-civils existants (`ACTE_NAISSANCE`, etc.) peuvent être
retirés ou conservés comme référence.

## Helpers Handlebars à ajouter pour Poulets

Dans `TemplateService.java` — nouveaux helpers à implémenter :

```java
handlebars.registerHelper("formatFcfa", (Integer amount, Options opts) ->
    String.format("%,d FCFA", amount).replace(",", " "));

handlebars.registerHelper("formatHalalStep", (Integer step, Options opts) ->
    switch (step) {
        case 1 -> "Élevage halal conforme";
        case 2 -> "Identification lot";
        case 3 -> "Abattoir agréé halal";
        case 4 -> "Présence sacrificateur";
        case 5 -> "Contrôle vétérinaire";
        case 6 -> "Certificat émis";
        default -> "Étape inconnue";
    });

handlebars.registerHelper("eleveurQrLink", (String id, Options opts) ->
    "https://poulets.fasodigitalisation.bf/verify/" + id);
```

## Dépendances externes à résoudre

Les poms référencent des libs partagées non clonées (cf
`backend/services/README.md`) :
- `bf.gov.actes:security-config`
- `bf.gov.shared:event-bus-lib`
- `bf.gov.shared:grpc-cluster-lib`
- `bf.gov.shared:resumable-transfer-lib`
- `com.actes:ec-cache-lib`

**Pour compile** : soit cloner ces libs, soit créer des stubs locaux.
Voir `backend/services/README.md` pour les options A/B/C.

## Intégration BFF attendue

Le `poulets-bff` doit exposer 6 endpoints proxy sécurisés :

```
POST /api/impression/generate      → forward vers impression-service
GET  /api/impression/queue         → liste jobs (paginée)
GET  /api/impression/:id           → détail job
GET  /api/impression/:id/pdf       → stream Blob application/pdf
GET  /api/impression/archives      → archives WORM
GET  /api/impression/templates     → liste templates (proxy renderer)
POST /api/verification/qr/:code    → vérif QR via impression-service
```

## Observabilité

- `/actuator/prometheus` exposé par les 2 services
- Métriques clés :
  - `render_pdf_duration_seconds` (renderer)
  - `render_cache_hits` (renderer)
  - `impression_queue_depth` (impression)
  - `impression_job_duration_seconds` (impression)
- Dashboards Grafana à créer sous `INFRA/observability/grafana/dashboards/`

## Sécurité

- HMAC-SHA256 entre impression-service et ec-certificate-renderer
- Secret rotation via Vault (`secret/renderer/hmac_key`)
- Playwright route interceptor bloque toutes requêtes réseau externes
- OAuth2 Resource Server sur impression-service (JWT Kratos)
- WORM storage : archives scellées avec hash SHA-256 (non modifiables)
- Rate limiting via Bucket4j (côté renderer)

## Roadmap

1. **Immédiat** : résoudre les libs partagées (option A/B/C du README backend)
2. **Court terme** : implémenter BFF endpoints + brancher UI réelle
3. **Moyen terme** : ajouter templates Handlebars Poulets-specific +
   dashboards Grafana
4. **Long terme** : évaluer `cert-render-rs` (Rust) pour performance (cf
   `ANALYSE-EC-CERTIFICATE-RENDERER.md` section "4. Projection OVH Scale A6")
