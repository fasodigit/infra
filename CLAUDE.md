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

**Fichiers conteneur** : utiliser **`Containerfile`** (et non `Dockerfile`)
pour toutes les images OCI du projet. Renommer immédiatement tout fichier
`Dockerfile` reçu en `Containerfile`. La syntaxe est identique (100%
compatible Buildah/Podman/Docker), seul le nom change. Les commandes de
build utilisent `podman build -f Containerfile .`.

**Commandes conteneur individuelles** : utiliser `podman exec`, `podman run`,
`podman ps`, `podman logs`, `podman network`, `podman build` — **jamais**
`docker exec`/`run`/`build` dans la documentation. Sur une machine
contributeur sans `podman` CLI installé, `docker` fonctionne en compat
(mêmes conteneurs, même API), mais tout nouveau doc/runbook doit être
écrit avec `podman`.

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
- Agents persistants : `kaya-rust-implementer`, `distributed-systems-rust`,
  `database-internals-rust`, `prompt-engineer`

### Discipline CWD pour les slash-commands

Toujours démarrer Claude Code **depuis `INFRA/`** (pas depuis le parent
`DEVELOPMENT-CLAUDE/`). Les slash-commands cloud (ex : `/ultrareview`,
`/security-review` quand il délègue au web-runner) utilisent le primary
working directory du processus, **pas** le CWD de la Bash tool.

```bash
cd /home/lyna/Documents/DEVELOPMENT-CLAUDE/INFRA
claude   # (ou ta commande de launch)
```

Sinon tu obtiens `Could not find merge-base with main. Make sure you're
in a git repo with a main branch.` — symptôme d'un CWD qui n'est pas
un git repo, pas d'un vrai problème de branche.

### `/ultrareview` — prérequis branche

`/ultrareview` (web-runner, ~5-10 min, $5-20 USD) compare `HEAD` vs
`main` via `git merge-base`. Il faut donc :
1. Être dans INFRA/ (cf. discipline CWD ci-dessus).
2. Être sur une **branche qui diverge de `main`** (topic branch /
   feature branch). Si HEAD = main, le command refuse parce qu'il n'y
   a rien à reviewer.
3. Workflow recommandé : topic branch `feature/<x>` → code → commits →
   `/ultrareview` avant merge → fix findings → merge vers main.

### `poulets-platform/frontend/twitter-mcp` — submodule externe

Tracked comme gitlink (pointeur de commit vers un repo twitter-mcp
externe). Apparaît souvent en `m` dans `git status` (modifs uploadées
au repo externe, pas encore reflétées dans le pointeur) — **ignorer**
tant qu'il n'y a pas besoin de bump explicite.

### Skills custom FASO

| Skill | Usage |
|-------|-------|
| `/ports` | Rapport port-policy + conflits (lit `INFRA/port-policy.yaml`) |
| `/stack-up [rust\|java\|ui]` | Boot séquence complète (containers → Vault → Java → Rust → UI) |
| `/stack-down [soft]` | Arrêt propre (kill safe sans `pkill -f`) |
| `/restart-impacted [since 15m]` | Rebuild + restart services dont le code a été modifié |
| `/status-faso` | Dashboard opérationnel (ports + HTTP + logs + Docker) |
| `/cycle-fix` | Boucle start/stop/fix jusqu'à stack healthy |
| `/kaya-implementation` | Routage agents KAYA spécialisés |

## 8. Port policy (source de vérité)

`INFRA/port-policy.yaml` — TOUS les ports du stack y sont déclarés.

- Avant d'ajouter un service, **réserver le port ICI d'abord**, puis coder
- Validator CI : `bash INFRA/scripts/validate-ports.sh`
- Les plages sont **owned** par tier ; un service qui squatte un port
  hors de sa plage = violation bloquante
- Plages clés :
  - `4800-4899` frontend / BFF
  - `8800-8899` Java HTTP (8801 auth-ms, 8901 poulets-api, 8803 notifier)
  - `8000-8099` Rust gateway (8080 ARMAGEDDON)
  - `9000-9099` Java actuator (loopback)
  - `9900-9999` admin API (loopback only)
  - `6380-6399` KAYA family

## 9. Discipline post-coding (automatique)

Après toute modification de code source, AVANT de rendre la main :
1. `/restart-impacted` pour relancer les services obsolètes
2. `/ports` pour vérifier les conflits
3. `/status-faso` pour valider la santé

**Ne JAMAIS `pkill -f <pattern>`** (tue le shell Claude Code). Toujours :
```bash
PIDS=$(ps -eo pid,cmd | grep '<pattern>' | grep -v grep | awk '{print $1}')
[[ -n "$PIDS" ]] && kill -TERM $PIDS
```

Si un `/cycle-fix` tourne dans une autre instance, **ne pas lancer**
`/restart-impacted` concurremment — attendre fin du cycle-fix.

---

*Dernière mise à jour : 2026-04-18*
