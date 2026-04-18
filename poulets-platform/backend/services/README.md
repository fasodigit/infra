<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# Poulets Backend — Services portés depuis Etat-civil

Ce répertoire contient les services backend Java clonés depuis
`fasodigit/Etat-civil/backend/services/` et adaptés pour FASO
DIGITALISATION.

## Services

| Service | Port HTTP | Rôle | Source |
|---|---|---|---|
| `ec-certificate-renderer` | 8920 | Moteur PDF (Playwright + Handlebars + ZXing QR + Caffeine cache) | `bf/gov/faso/renderer/**` |
| `impression-service` | 8921 | Orchestrateur métier (queue + WORM + QR verif + audit) | `bf/gov/faso/impression/**` |

## Rebranding effectué

- `bf.gov.etatcivil.renderer` → `bf.gov.faso.renderer`
- `com.actes.impression` → `bf.gov.faso.impression`
- Parents Maven : `bf.gov.actes:etat-civil-parent` → `spring-boot-starter-parent:3.4.4`
  (services autonomes, plus simples à builder)
- `groupId` : `bf.gov.actes` / `com.actes` → `bf.gov.faso`

## Dépendances externes à résoudre pour compile

Les poms référencent des libs partagées d'Etat-civil qui ne sont **pas**
clonées dans Poulets. Pour obtenir un build fonctionnel, il faudra :

### Option A — Cloner les libs partagées

Depuis `fasodigit/Etat-civil/backend/shared/` (si existe) ou
`fasodigit/shared-infrastructure/shared-libs/` :

```bash
# À investiguer — libs référencées dans les poms :
- bf.gov.actes:security-config
- bf.gov.shared:event-bus-lib
- bf.gov.shared:grpc-cluster-lib
- bf.gov.shared:resumable-transfer-lib
- com.actes:ec-cache-lib
```

### Option B — Stubs minimaux

Créer des interfaces vides dans `INFRA/shared/` et les publier localement
en `mvn install`. Adapté pour dev isolé.

### Option C — Commenter temporairement

Dans chaque `pom.xml`, ajouter `<!-- TODO clone -->` sur les blocs
`<dependency>` pour les libs manquantes et adapter le code qui les
utilise (stubs Java inline).

## Build attendu à terme

```bash
cd INFRA/poulets-platform/backend/services
./mvnw -pl ec-certificate-renderer clean compile -DskipTests
./mvnw -pl impression-service clean compile -DskipTests
```

## Intégration podman-compose

Voir `INFRA/docker/compose/podman-compose.impression.yml` (Phase 5).

## Templates Handlebars à ajouter

Dans `ec-certificate-renderer/src/main/resources/templates/` :
- `certificat-halal.hbs` (certif lot)
- `contrat-commande.hbs`
- `recepisse-livraison.hbs`
- `attestation-elevage.hbs`

Les templates existants (ACTE_NAISSANCE, ACTE_MARIAGE, etc.) viennent
d'Etat-civil et peuvent être retirés ou gardés comme référence.

## API exposée

### ec-certificate-renderer (8920)
- `POST /render/{templateName}` — body JSON data → PDF bytes (HMAC required)
- `POST /render/batch` — body JSON array → ZIP of PDFs (max 50, HMAC required)
- `GET /health` — public
- `GET /actuator/health/readiness` — k8s probe

Auth : `X-Internal-Auth: {timestamp}:{hmac_sha256_hex}`.
Secret : env `INTERNAL_AUTH_SECRET` (Vault `secret/renderer/hmac_key`).

### impression-service (8921)
- `POST /api/impression/generate` — déclenche job + appelle renderer
- `GET /api/impression/queue` — liste jobs
- `GET /api/impression/{id}` — détail job
- `GET /api/impression/{id}/pdf` — download PDF
- `POST /api/verification/qr/{code}` — vérif QR

Auth : OAuth2 Resource Server (JWT).

## Notes origine

Le fichier `ANALYSE-EC-CERTIFICATE-RENDERER.md` contient l'audit complet du
renderer par les devs Etat-civil — latences, goulots, roadmap Rust.
