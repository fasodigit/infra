<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# CLAUDE.md — Règles projet FASO DIGITALISATION

Ce fichier est automatiquement chargé par Claude Code au démarrage d'une
session travaillant dans ce dépôt. Les règles ici **priment** sur les
comportements par défaut.

## 1. Conteneurisation : **podman-compose** uniquement

**Règle absolue : n'utiliser QUE `podman-compose`** (et non `docker compose` ou
`docker-compose`) pour orchestrer la stack FASO en local et en CI.

- Tous les fichiers d'orchestration sont nommés `podman-compose*.yml`.
  Ne JAMAIS créer ou laisser de fichier `docker-compose*.yml` — renommer
  immédiatement tout fichier reçu sous cette forme.
- Les scripts, workflows GitHub Actions, runbooks et documentation doivent
  invoquer `podman-compose -f <fichier>.yml <commande>`.
- Si `podman-compose` est indisponible localement (machine contributeur),
  `docker compose -f podman-compose.yml` fonctionne en compatibilité — mais
  la **source de vérité** reste `podman-compose`.

**Pourquoi** : souveraineté (podman est rootless, sans daemon central,
licence open-source Red Hat stable) ; cohérence avec l'environnement cible
Kubernetes où les conteneurs tournent sous runtime OCI (CRI-O / containerd),
pas Docker Engine.

Fichiers compose canoniques (au 2026-04-17) :

| Chemin | Rôle |
|--------|------|
| `INFRA/docker/compose/podman-compose.yml` | stack principale (postgres, kratos, keto, mailhog, oathkeeper, armageddon, auth-ms, poulets, bff, frontend, jaeger) |
| `INFRA/docker/compose/podman-compose.dev.yml` | override dev (logs verbeux, LOG_LEAK_SENSITIVE_VALUES) |
| `INFRA/vault/podman-compose.vault.yml` | Consul + Vault |
| `INFRA/ory/podman-compose.ory.yml` | ORY stack standalone |
| `INFRA/growthbook/podman-compose.growthbook.yml` | GrowthBook + MongoDB |
| `INFRA/observability/grafana/podman-compose.observability.yml` | Grafana + Prom + Loki + Tempo + OTel |

Commandes usuelles :

```bash
cd INFRA/docker/compose

# Bootstrap complet (tous les services)
bash scripts/init-secrets.sh
podman-compose -f podman-compose.yml up -d

# Ajout Vault + Consul
podman-compose -f podman-compose.yml -f ../../vault/podman-compose.vault.yml up -d consul vault

# Ajout GrowthBook
podman-compose -f podman-compose.yml -f ../../growthbook/podman-compose.growthbook.yml up -d growthbook

# Observabilité
podman-compose -f podman-compose.yml -f ../../observability/grafana/podman-compose.observability.yml up -d grafana prometheus loki tempo
```

## 2. Gestion des secrets : Vault (+ Consul backend) est la source de vérité

Depuis 2026-04-17, **tous** les nouveaux secrets FASO sont stockés dans
Vault (`INFRA/vault/`). Les fichiers `INFRA/docker/compose/secrets/*.txt`
sont conservés comme **bootstrap** uniquement — en production ils sont
injectés via Vault Agent / Spring Cloud Vault / `vaultrs`.

Workflow contributeur :

```bash
# 1. Démarrer Consul + Vault
cd INFRA/docker/compose
podman-compose -f podman-compose.yml -f ../../vault/podman-compose.vault.yml up -d consul vault

# 2. Initialiser (une seule fois par machine)
bash ../../vault/scripts/init.sh
# → ~/.faso-vault-keys.json (chmod 600, JAMAIS commité)

# 3. Seed secrets
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
bash ../../vault/scripts/seed-secrets.sh

# 4. Optionnel : DB dynamic creds + PKI
bash ../../vault/scripts/configure-database.sh
bash ../../vault/scripts/configure-pki.sh
```

Politique KV : `faso/<service>/<usage>` (cf. `INFRA/vault/README.md`).
Policies read-only par service (principe du moindre privilège).

## 3. Souveraineté (règle absolue)

| Remplacement | Pour |
|--------------|------|
| **KAYA** | Redis, DragonflyDB |
| **ARMAGEDDON** | Envoy, NGINX+, HAProxy |
| **xds-controller** | Istio control plane |

**Ne jamais** mentionner DragonflyDB / Envoy / Istio comme composants actifs
dans le code ou la documentation — uniquement comme références d'inspiration
avec note historique. Ne jamais ajouter de dépendance vers ces projets.

Exception : Vault + Consul + Postgres + Temporal + Redpanda + YugabyteDB
restent HashiCorp/tiers car pas d'alternative Rust mature. Évaluation
future : `openbao` (fork Vault) si stable.

## 4. Licence et contributions

- **AGPL-3.0-or-later** sur tous les fichiers source (SPDX injecté automatiquement
  par `scripts/spdx-headers.sh`, hook pre-commit)
- Conventional commits obligatoires (commitlint bloque les push non conformes)
- Aucun `--no-verify`, `--no-gpg-sign` — en cas de blocage, corriger le
  cause-racine, ne jamais bypasser
- Tests : `cargo nextest run` + `mvn verify` + `bun run test` doivent passer
  avant tout push

## 5. Sécurité

- **JAMAIS** de secret en clair dans le repo — voir `SECURITY.md`
- **JAMAIS** de fichier `*.pem`, `*.key`, `secrets/*.txt` committé
  (bloqué par `.gitignore`)
- Password de test : via variable `E2E_TEST_PASSWORD` uniquement
- Rotation SVID SPIRE 24 h, monitoring < 72 h (alerte auto)
- Revue sécurité via GitHub Private Vulnerability Reporting

## 6. Documentation

- README.md racine = porte d'entrée publique (projet public)
- SECURITY.md = politique divulgation responsable
- CONTRIBUTING.md = workflow
- CODE_OF_CONDUCT.md = Contributor Covenant 2.1 FR
- Guide architectural v3.1 : `INFRA/docs/v3.1-souverain/GUIDE-ARCHITECTURAL-v3.1-SOUVERAIN.md`
- SLOs Sloth : `INFRA/observability/slo/*.slo.yaml`
- Runbooks : `INFRA/observability/alertmanager/runbooks/*.md`

## 7. Environnement Claude Code

- MCP servers actifs : `context7` (doc libraries), `serena` (IDE-assistant)
- Hook `PostToolUse` cargo check après édition Rust dans `INFRA/kaya/`
- Skill `kaya-implementation` pour routage agents spécialisés
- Agents persistants : `kaya-rust-implementer`, `distributed-systems-rust`,
  `database-internals-rust`, `prompt-engineer`

---

*Dernière mise à jour : 2026-04-17*
