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
- Bootstrap dev : `bash INFRA/scripts/bootstrap-dev.sh` (idempotent)
- Bootstrap dev avec reset : `bash INFRA/scripts/bootstrap-dev.sh --reset` (DROPS auth_ms + poulets_db data)
- Vault paths pour la prod : voir `INFRA/scripts/bootstrap-dev.README.md`

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

## 10. Ordre des phases de validation : `cycle-fix` AVANT E2E

**Règle absolue** : toute campagne de tests E2E (Playwright Full-Stack ou
autre) doit être précédée d'une boucle `/cycle-fix` qui amène l'ensemble du
stack à un état **GREEN** (compile OK, services healthy, ports listening,
zéro erreur dans les logs Loki/podman).

**Pourquoi** : un test E2E lancé sur un stack instable produit du bruit
(faux positifs, faux négatifs, fixtures qui croient à une régression alors
que c'est juste un service down). Ça gaspille du temps de debug et fait
perdre confiance dans la suite. La discipline « stabiliser d'abord, tester
ensuite » est non négociable.

**Workflow obligatoire** :

```
[code modifications terminées]
        │
        ▼
  /cycle-fix (loop)
        ├─ podman-compose -f podman-compose.yml up -d (avec scripts seed)
        ├─ wait services healthy (probes /health/ready)
        ├─ scan logs (Loki + podman logs) pour erreurs root cause
        ├─ fix root cause (compile / config / migrations / topics / Vault)
        ├─ podman-compose restart <service-impacté>
        └─ répète jusqu'à : tous ports listening + zéro erreur fatale
        │
        ▼ (stack GREEN)
  Tests Playwright Full-Stack E2E
        ├─ run specs (3 projets : chromium-headless, smoke, chrome-headless-new)
        ├─ collecte traces Jaeger sur fail
        └─ rapport coverage + P50/P95/P99
        │
        ▼
  Si bug E2E détecté → retour cycle-fix (jamais "fix dans la suite E2E")
```

**Anti-patterns interdits** :
- ❌ Lancer `playwright test` sur un stack pas validé `/cycle-fix`.
- ❌ "Cycle-fix après E2E" pour fixer ce que les tests ont révélé — c'est
  le test qui doit valider, pas découvrir des bugs de stabilité.
- ❌ Skipper cycle-fix parce que "ça marchait hier" — chaque session ou
  chaque modif de code redémarre la boucle.

**Phasing canonique d'un projet FASO** :

| Ordre | Phase | Output |
|-------|-------|--------|
| 1 | Implémentation | Code mergé, compile OK individuellement |
| 2 | **Cycle-fix** | Stack GREEN, services healthy, zéro erreur |
| 3 | E2E Playwright | Coverage rapport, traces Jaeger archivées |
| 4 | Bug-fix sur findings E2E | Retour étape 2 si nouveau bug |

Cette discipline est validée par `/status-faso` avant de lancer toute suite
E2E. Si `/status-faso` ne montre pas tous services en `healthy` →
**bloquer** l'exécution E2E.

## 11. Couverture E2E systématique de toute nouvelle fonctionnalité

**Règle absolue** : toute fonctionnalité ajoutée (endpoint REST/gRPC/WS,
flow UI, topic Redpanda, capacité Keto, migration DB, setting Configuration
Center, hook Kratos, schéma Avro, …) doit être livrée **avec** sa (ou ses)
spec(s) Playwright correspondante(s). Pas de feature sans test E2E.

**Pourquoi** : sans coverage écrite au moment de l'ajout, la feature dérive
(régression silencieuse au prochain refactor, flow cassé qu'on ne détecte
qu'en prod, faux sentiment de sécurité). La discipline « écrit la feature
ET son E2E dans le même PR » est non négociable.

**Comment** :

1. Au moment d'écrire le code feature, l'auteur **écrit en miroir** la spec
   Playwright sous `tests-e2e/<n°-suite>/<feature>.spec.ts`.
2. La spec utilise des **données réelles** :
   - OTP via Mailpit (`MailpitClient.waitForOtp`).
   - PassKey via virtual authenticator CDP (`fixtures/webauthn.ts`).
   - TOTP via `otplib` (`fixtures/totp.ts`).
   - Acteurs déterministes via `fixtures/actors.ts` (seed=42).
   - Pas de mocks backend — le test hit la stack réelle (cycle-fix Phase
     4.c garantit qu'elle est GREEN).
3. La spec doit couvrir au moins **1 happy path + 1 erreur principale**
   (ex : code expiré, signature altérée, rate-limit dépassé).
4. Référencer la spec dans la table de mapping documentaire du projet
   (`ARCHITECTURE-SECURITE-COMPLETE.md` ou équivalent).
5. Si la feature n'est pas testable directement (ex : scheduled job
   interne, consumer Kafka sans effet UI visible), créer **un endpoint
   d'observation** ou une **métrique Prometheus** interrogeable par le
   test.

**Anti-patterns interdits** :
- ❌ Ajouter un endpoint REST sans la spec correspondante.
- ❌ « On testera plus tard / en Phase X » sans inscrire un placeholder
  daté + numéroté dans le mapping.
- ❌ Mocker le backend dans Playwright (toujours real services en
  containers — cf. PLAYWRIGHT-FULLSTACK-E2E-GUIDE.md).
- ❌ Spec qui valide uniquement le happy path (au moins 1 erreur).
- ❌ Spec qui n'est pas exécutable via `bunx playwright test
  tests-e2e/<chemin>` directement.

**Workflow obligatoire** :

```
[idée fonctionnalité]
        │
        ▼
[implémentation backend + frontend]
        │
        ▼
[écriture spec Playwright en miroir] ◄── MÊME PR
        │
        ▼
[mise à jour table mapping doc]
        │
        ▼
[cycle-fix → stack GREEN] ◄── §10
        │
        ▼
[exécution spec → assertion OK]
        │
        ▼
[merge]
```

**Conséquence sur ce projet (admin-UI)** : Phase 4.b couvre l'implémentation
des 23 modules de sécurité ; chacun doit avoir sa spec dans la suite
`tests-e2e/18-admin-workflows/` (33 specs au total mappées dans
`ARCHITECTURE-SECURITE-COMPLETE-2026-04-30.md` §5). Toute feature future
ajoutée à admin-UI suit la même discipline.

## 12. Mode d'exécution Playwright : navigateur réel + comptes seedés

**Règle absolue** : toute campagne Playwright lance un **navigateur Chromium
réel et 100 % interactif** (clics, frappes clavier, navigation, XHR/fetch,
WebSocket réels) en mode `--headless` (pas d'UI graphique visible mais
toutes les capacités du browser actives). **Aucune simulation, aucun mock,
aucun appel HTTP forgé.**

**Pourquoi** : un test E2E qui n'exécute pas le vrai DOM, le vrai cycle de
vie de session, la vraie négociation TLS, le vrai parsing JWT, n'apporte
aucune garantie sur le comportement en production. La force de Playwright
réside dans la fidélité du browser ; toute dérogation invalide la suite.

### Pré-condition : comptes SUPER-ADMIN seedés

**Email = identifiant primaire** pour tous les flows d'authentification
(signup, login, récupération, MFA enrollment). Aucun flow ne peut être
testé sans une boîte mail capturable (Mailpit en dev/CI, SMTP réel en
prod). Toute notification de validation (OTP 8 chiffres, magic-link,
password reset, recovery code) transite par email — cf.
`ARCHITECTURE-SECURITE-COMPLETE-2026-04-30.md` §6 timeline canonique.

**Conséquence pour Playwright** : la stack doit avoir
- `notifier-ms` UP (consume Redpanda, send via SMTP)
- `Mailpit :8025` accessible (capture côté E2E via `MailpitClient.waitForOtp`)
- `SMTP_HOST` configuré (Mailpit :1025 en dev)

Si `notifier-ms` ou `Mailpit` est down → bloquer la suite (cf. §10
gate cycle-fix avant E2E).

Les specs admin-UI (et toute spec qui exige un acteur SUPER-ADMIN)
requièrent que les **2 identités SUPER-ADMIN** soient déjà créées en Kratos
**avec mot de passe valide** + tuples Keto correspondants. Le seed est
réalisé par le bootstrap Phase 4.c (cf. `Phase 4.c suite` runbook) :

| Acteur | Email | UUID Kratos | Mot de passe par défaut (dev) | Variable d'env override (prod/CI) |
|---|---|---|---|---|
| Aminata Ouédraogo | `aminata.ouedraogo@faso.bf` | `253ec814-1e10-44c7-b7a7-fd44581e4393` | `ChangeMe!2026SuperAdmin` | `E2E_AMINATA_PASSWORD` |
| Souleymane Sawadogo | `s.sawadogo@faso.bf` | `5d621b0c-f611-45d8-afe3-2d299d2eb82d` | `ChangeMe!2026SecurityLead` | `E2E_SOULEYMANE_PASSWORD` |

Ces credentials **DOIVENT** apparaître dans `fixtures/actors.ts` (ou
équivalent par projet) avec `password` lu depuis la variable d'env (cf. §5
sécurité). Si la variable n'est pas définie, le test peut tomber sur le
fallback dev — uniquement en local. CI prod : variable obligatoire (échec
explicite si absente).

### Configuration Playwright canonique

```typescript
// playwright.config.ts
export default defineConfig({
  use: {
    headless: true,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    baseURL: 'http://localhost:8080',  // ARMAGEDDON — JAMAIS le frontend dev server directement
  },
  projects: [
    { name: 'chromium-headless', use: { ...devices['Desktop Chrome'] } },
    { name: 'smoke', testMatch: /\.smoke\.spec\.ts$/ },
    { name: 'chrome-headless-new', use: { channel: 'chrome', launchOptions: { args: ['--headless=new'] } } },
  ],
});
```

**Exigence path browser → backend** : `baseURL` doit pointer sur ARMAGEDDON
(souverain). Le browser ne doit JAMAIS appeler `:8801`/`:8901`/`:4433`
directement (cf. `PLAYWRIGHT-FULLSTACK-E2E-GUIDE.md` §1.2). Toute requête
XHR/fetch passe par `:8080` → ARMAGEDDON → ext_authz Keto → backend.

### Anti-patterns interdits

- ❌ `page.route(/.../, route => route.fulfill(...))` pour mocker un endpoint backend.
- ❌ `headless: false` permanent (dev visuel OK, CI doit rester `headless: true`).
- ❌ Hardcoder un mot de passe SUPER-ADMIN dans le code source de la spec.
- ❌ Utiliser `expect(true).toBe(true)` ou des stubs qui ne touchent jamais le backend.
- ❌ Skipper un test parce qu'un service backend est down — préférer **bloquer la suite** (gate Phase 4.c).

### Workflow obligatoire

```
[stack GREEN cf. §10]
        │
        ▼
[fixtures/actors.ts contient SUPER-ADMIN avec UUID + email + password env]
        │
        ▼
[bunx playwright test --project=chromium-headless] → real browser, real backend
        │
        ▼
[trace.zip + screenshot + video sur fail → Jaeger trace ID dans audit_log]
```

Cette discipline est validée à chaque session : si `fixtures/actors.ts`
n'a pas les SUPER-ADMIN seedés OU si `playwright.config.ts` mocke des
routes backend, **bloquer le merge** jusqu'à correction.

## 13. Cadence de commits sur sessions agent-driven

**Règle absolue** : sur toute session de codage **longue ou multi-phase**
(typiquement orchestration de plusieurs agents IA), commiter à 3 moments
clés. Pas de commit massif final unique — le risque de perte ou de
non-revue est trop élevé.

### Les 3 checkpoints obligatoires

1. **CHECKPOINT-AVANT** — avant de démarrer un gros chantier
   (lancement d'agents en parallèle, début de phase, refactor d'envergure) :
   commiter l'état courant pour disposer d'une **baseline propre**. Si rien
   à commiter, vérifier explicitement `git status --short` est vide.
   Sinon, créer un ou plusieurs commits atomiques pour figer l'avant.

2. **CHECKPOINT-INTER** — après chaque **long moment de code** (typiquement
   ≥ 1 stream d'agent terminé OU ≥ 30 min de travail concentré) : commiter
   les artefacts produits, atomiquement par scope (cf. exemples §13 ci-bas).
   Ne pas attendre la fin du chantier complet — chaque livrable validé =
   un commit séparé.

3. **CHECKPOINT-FINAL** — après `/cycle-fix` GREEN + Playwright passant +
   AVANT de déclarer le chantier terminé à l'utilisateur : commiter tout
   ce qui reste (fixes du cycle-fix, ajustements specs, configs validées).
   La phrase finale type *« stack GREEN, tests passent, X livrés »* ne peut
   être prononcée que quand le working tree est propre (`git status --short`
   est vide). Sinon : checkpoint-final manquant = chantier non-clos.

### Pourquoi

- **Reviewabilité** : 12 commits de 50-200 lignes chacun se reviewent ; un
  unique commit de 5000+ lignes ne se review pas (et ne sera pas relu).
- **Bisection** : si un bug apparaît plus tard, `git bisect` ne fonctionne
  qu'avec une granularité raisonnable.
- **Rollback partiel** : un stream cassé peut être `git revert` sans
  perdre les autres streams.
- **Pression cognitive** : à chaque checkpoint l'agent et l'utilisateur
  font une mini-pause de validation. Sans checkpoints, on accumule du
  risque de dérive silencieuse.

### Anti-patterns interdits

- ❌ « Je commiterai tout à la fin » — interdit. Trop tard pour atomicité.
- ❌ Déclarer « tout est OK » avec `git status` non vide.
- ❌ Un seul gigantesque commit `feat(everything): big bang` couvrant
  plusieurs phases (admin-UI + TERROIR + observability + …).
- ❌ Commiter du code qui ne compile pas — chaque commit doit individuellement
  être en état GREEN (compile + lint + format) sur le scope qu'il touche.
- ❌ Skipper le `--no-verify` pour passer en force — corriger la cause.

### Workflow obligatoire

```
[début gros chantier]
        │
        ▼
[CHECKPOINT-AVANT]  git commit ... (baseline propre)
        │
        ▼
[lancement agent(s) wave 1]
        │
        ▼
[wave 1 done]  →  [CHECKPOINT-INTER]  git commit -m "feat(scope-1): …"
        │
        ▼
[lancement wave 2]
        │
        ▼
[wave 2 done]  →  [CHECKPOINT-INTER]  git commit -m "feat(scope-2): …"
        │
        ▼
[/cycle-fix loop]
        │
        ▼
[stack GREEN]  →  [Playwright suite]
        │
        ▼
[suite passante]  →  [CHECKPOINT-FINAL]  git commit -m "fix(cycle-fix): … + test(e2e): …"
        │
        ▼
[« chantier OK ✓ » à l'utilisateur]
```

### Format conventional commits FASO (rappel §4)

- `feat(<scope>):` nouvelle fonctionnalité
- `fix(<scope>):` correction de bug
- `chore(<scope>):` refactor / tooling / pas de feature ni bug
- `docs(<scope>):` documentation pure
- `test(<scope>):` ajout/modif tests
- Trailer obligatoire : `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`

Cette discipline est observée par tous les agents qui orchestrent des
streams parallèles (admin-UI Phase 4.b, TERROIR P0+, futures phases). Si
un agent finit son livrable et ne propose pas un checkpoint à l'humain,
**c'est un bug à signaler** dans le prompt de l'agent.

---

*Dernière mise à jour : 2026-04-30*
