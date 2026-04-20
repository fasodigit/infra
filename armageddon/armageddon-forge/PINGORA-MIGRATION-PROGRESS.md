# PINGORA-MIGRATION-PROGRESS

État de la migration Pingora du backend proxy `armageddon-forge`, à la fin de
la vague 1 autonome (2026-04-20).

- **Tracker maître** : [#108](https://github.com/fasodigit/infra/issues/108)
- **Design matrix** : [`PINGORA-MIGRATION.md`](PINGORA-MIGRATION.md)
- **Méthodologie bench** : [`BENCH-METHODOLOGY.md`](BENCH-METHODOLOGY.md)
- **Design shadow** : [`SHADOW-MODE.md`](SHADOW-MODE.md)
- **Runtime bridge** : [`src/pingora/RUNTIME.md`](src/pingora/RUNTIME.md)
- **Branche** : `feat/pingora-migration` (locale, non poussée)

## Synthèse — commits de la branche

| SHA | Commit | Gate | LOC net | Tests |
|------|---|---|---|---|
| 8a786e8 | fix(armageddon): harden gateway — 6 security fixes (ultrareview) | préalable | +898 / -80 | 6 crates verts |
| a842a98 | feat(armageddon-forge): scaffold pingora migration M0 | #101 | +1445 sur 27 fichiers | 18/18 |
| 69682a3 | port router + CORS + VEIL (M1 wave 1) | #102 (subs #95 #96 #100) | +1106 sur 3 filtres | 35/35 |
| 888ed52 | port PoolKey selector + round-robin LB (M2 wave 1) | #103 | +864 sur 3 fichiers | 18/18 |
| 2c78d32 | engines pipeline + real AEGIS adapter (M3 wave 1) | #104 | +951 sur 9 fichiers + Cargo.toml | 10/10 |
| 04de00c | streaming compression gzip/brotli/zstd (M4 wave 1) | #105 | +716 sur 2 fichiers | 16/16 |
| 4be840d | bench harness + shadow-mode design (M5 wave 1) | #106 | +1024 sur 4 nouveaux fichiers | bash -n clean, cargo bench --no-run OK |

**Total code ajouté** : environ 5 140 LOC net nouveaux + 898 LOC sécurité préservée.
**Total tests** : 94/94 pass sur `cargo test -p armageddon-forge --features pingora --lib pingora`.

## État par gate

### Gate #101 — M0 Foundations **(complet vague 1)**

Scaffold structurel complet dans `src/pingora/` :

```
src/pingora/
├── mod.rs            # feature-gated entry point
├── ctx.rs            # RequestCtx (tous les champs forward-compatibles)
├── gateway.rs        # PingoraGateway + ProxyHttp impl (filter-chain walker)
├── server.rs         # build_server()
├── runtime.rs        # OnceLock tokio Runtime bridge sur thread OS dédié
├── RUNTIME.md        # design doc Option A (Pingora main + tokio isolé)
├── filters/          # ForgeFilter trait, Decision enum, 6 filter slots
├── upstream/         # selector, lb (+ 4 stubs)
├── engines/          # pipeline, aegis_adapter (+ 6 stubs)
└── protocols/        # compression (+ 3 stubs)
```

Pas de reliquat. Prêt pour M1 consolidation.

### Gate #102 — M1 Filters applicatifs **(vague 1, 3/6 subs)**

| Sub-issue | État | LOC | Tests |
|---|---|---:|---:|
| [#95 router](https://github.com/fasodigit/infra/issues/95) | **done wave 1** | 241 | 7/7 |
| [#96 cors](https://github.com/fasodigit/infra/issues/96) | **done wave 1** | 477 | 14/14 |
| [#97 jwt](https://github.com/fasodigit/infra/issues/97) | stub M0 — **M1 wave 2** | 0 | 0 |
| [#98 feature-flag](https://github.com/fasodigit/infra/issues/98) | stub M0 — **M1 wave 2** | 0 | 0 |
| [#99 otel](https://github.com/fasodigit/infra/issues/99) | stub M0 — **M1 wave 2** | 0 | 0 |
| [#100 veil](https://github.com/fasodigit/infra/issues/100) | **done wave 1** | 435 | 12/12 |

**À faire M1 wave 2** :
- JWT : reprendre `src/jwt.rs` (553 LOC) et câbler KAYA RESP3 via le
  `tokio_handle()` bridge. Cache JWKS 300s, session cache `jwt:<sha256>`.
- feature-flag : reprendre `src/feature_flag_filter.rs` (398 LOC) en
  **préservant** le scrub inconditionnel en tête de `call()` (fix ultrareview
  bug_005, commit 8a786e8). 3 tests de régression à porter verbatim.
- otel : hook `ProxyHttp::logging()` pour la fermeture du span; propagation
  `traceparent` dans `upstream_request_filter`.
- **Trade-off résiduel** : CORS origin et VEIL nonce sont stashés dans
  `RequestCtx.feature_flags` avec préfixes (`cors:origin:`, `veil:nonce:`).
  À remplacer par un slot typé dédié dans `ctx.rs` (petite consolidation).
- **TLS detection** : `session.is_tls()` n'existe pas en Pingora 0.3. VEIL
  utilise `X-Forwarded-Proto: https` comme fallback. Upgrade path :
  `session.digest().ssl_digest.is_some()` quand l'API se stabilise.

### Gate #103 — M2 Machinerie upstream **(vague 1, 2/6 modules)**

| Module | État | Notes |
|---|---|---|
| `selector.rs` | **done wave 1** — 610 LOC | PoolKey SPIFFE-aware (bug_006 préservé) + ClusterResolver hot-reload + résolution fail-closed |
| `lb.rs` | **done wave 1** — 245 LOC | Round-robin complet (6/6 tests). Weighted + P2C : `todo!()` |
| `mtls.rs` | stub M0 — **M2 wave 2** | |
| `circuit_breaker.rs` | stub M0 — **M2 wave 2** | Port `src/circuit_breaker.rs` (226 LOC) vers `fail_to_proxy` + `upstream_response_filter` |
| `health.rs` | stub M0 — **M2 wave 2** | Port `src/health.rs` (760 LOC) — thread tokio bg, publish vers `ArcSwap<ClusterState>` |
| `retry.rs` | stub M0 — **M2 wave 2** | |

**Invariant sécurité préservé** : `ClusterResolver::resolve()` retourne `None`
+ `error!` log (**jamais** fallback plaintext) quand
`tls_required && expected_spiffe_id.is_none()`. Verifié par
`resolver_mtls_without_expected_spiffe_fails`.

### Gate #104 — M3 8 moteurs sécurité **(vague 1, 2/9 modules)**

| Module | État | LOC | Tests |
|---|---|---:|---:|
| `pipeline.rs` | **done wave 1** | 404 | 6/6 |
| `aegis_adapter.rs` | **done wave 1** (Regorus réel) | 249 | 4/4 |
| `sentinel_adapter.rs` | stub adapter | 47 | — |
| `arbiter_adapter.rs` | stub adapter | 44 | — |
| `oracle_adapter.rs` | stub adapter | 44 | — |
| `nexus_adapter.rs` | stub adapter | 45 | — |
| `ai_adapter.rs` | stub adapter | 41 | — |
| `wasm_adapter.rs` | stub adapter | 41 | — |

Pipeline utilise `FuturesUnordered` + `tokio::time::timeout` par moteur.
Drop = cancel des futures en vol → short-circuit Deny efficace.

**Placeholder à lever** : `aegis_adapter.rs:20` construit un `HttpRequest` +
`ConnectionInfo` avec chaînes vides. Les politiques Rego qui inspectent la
méthode/path/headers voient du vide. À corriger quand `RequestCtx` est
enrichi (M1 wave 2 consolidation).

### Gate #105 — M4 Protocoles **(vague 1, 1/4 modules)**

| Module | État | LOC | Tests |
|---|---|---:|---:|
| `compression.rs` | **done wave 1** | 708 | 16/16 |
| `grpc_web.rs` | stub M0 — **M4 wave 2** | — | — |
| `websocket.rs` | stub M0 — **M4 wave 2** | — | — |
| `traffic_split.rs` | stub M0 — **M4 wave 2** | — | — |

**TODO(#105) ligne 474** : wiring `CompressionFilter` + `CompressionStream`
dans `ProxyHttp::response_filter` + `response_body_filter` une fois `ctx.rs`
enrichi d'un slot scratch par requête.

**Vérification roundtrip** : décompression byte-exact sur payload 11 000
octets (`"hello world " × 1000`) pour gzip / brotli / zstd.

### Gate #106 — M5 xDS + mesh + bench **(vague 1, 2/5 livrables)**

| Livrable | État |
|---|---|
| `benches/pingora_vs_hyper.sh` | **done** — wrk harness runnable (bash -n clean) |
| `benches/pingora_filter_chain_micro.rs` | **done** — Criterion skeleton compile |
| `SHADOW-MODE.md` | **done** — 285 lignes |
| `BENCH-METHODOLOGY.md` | **done** — 233 lignes |
| xDS ADS client wire-up | **M5 wave 2** — issue #106 |
| SPIFFE cert rotation hook | **M5 wave 2** |
| Shadow mode runtime | **M5 wave 2** |

**Bin manquant** : le script wrk appelle `cargo run --bin pingora_bench_server`
et `--bin hyper_bench_server` qui n'existent pas encore. TODO(#106) documenté
dans le script — il échoue proprement avec message si les bins sont absents.

### Gate #107 — M6 Cutover

Pas touché — dépend de M5 complet. Issue ouverte, prête.

## Contraintes de build

### Pin `sfv` obligatoire

`pingora-core 0.3.0` référence `sfv = "0"` qui résout sur 0.14.0 (API
breaking). Chaque nouveau clone doit exécuter :

```bash
cargo update -p sfv --precise 0.9.4
```

avant tout `cargo build --features pingora`.

**Raison du non-patch** : `[patch.crates-io]` refuse de pointer vers la même
source (crates.io → crates.io). Options futures (à décider) :
1. Tracker `Cargo.lock` pour les builds features (modifier `.gitignore` ligne 4).
2. Ajouter `sfv = "=0.9.4"` comme `[workspace.dependencies]` et `use` dans un
   crate pour forcer l'unification du resolver.
3. Upgrader Pingora dès qu'une version >= 0.4 fixe la dépendance.

### `cmake` requis

`pingora-core` tire `flate2` avec backend zlib-ng qui compile du C via
`cmake`. Installer sur la machine CI :

```bash
# Debian/Ubuntu
apt install cmake
# Arch / rolling (utilisé localement)
pipx install cmake   # fallback si paquet système pas dispo
```

## Matrices de vérification (fin vague 1)

| Commande | Résultat |
|---|---|
| `cargo check -p armageddon-forge` | ✅ clean (1 warning pré-existant dans `feature_flags.rs`) |
| `cargo check -p armageddon-forge --features pingora` | ✅ clean |
| `cargo test -p armageddon-forge --features pingora --lib pingora` | ✅ **94/94 passed** |
| `cargo bench --bench pingora_filter_chain_micro --features pingora --no-run` | ✅ compile |
| `bash -n benches/pingora_vs_hyper.sh` | ✅ clean |

## Ce qui reste (wave 2 minimale avant M6)

Classement par ordre d'impact, pour reprise de session :

1. **M1 wave 2** (le plus gros levier)
   - **JWT** #97 — câble KAYA RESP3 via `pingora::runtime::tokio_handle()`.
   - **feature-flag** #98 — préserver le scrub bug_005 en tête.
   - **otel** #99 — `logging()` hook + `traceparent`.
   - Ajout d'un slot scratch typé à `RequestCtx` (consolide CORS origin,
     VEIL nonce, et débloque M4 ProxyHttp integration).

2. **M2 wave 2** (bloque M5 cert rotation + shadow fidélité)
   - mtls / auto_mtls avec SPIFFE peer-id validation dans
     `upstream_request_filter`.
   - circuit_breaker + health + retry via les hooks Pingora.

3. **M3 wave 2** — 6 adapters restants (SENTINEL, ARBITER, ORACLE, NEXUS,
   AI, WASM). WASM aura le plus gros travail (thread-unsafe Wasmtime →
   thread tokio dédié + channel).

4. **M4 wave 2** — gRPC-Web (798 LOC à reporter), WebSocket (via
   `session.upgrade_to_ws()` natif Pingora), traffic_split, wiring
   compression.

5. **M5 wave 2** — xDS, SPIFFE rotation, shadow mode runtime, bins bench
   serveurs.

6. **M6** — flip `default = ["pingora"]`, deprecate hyper, cutover doc.

## Points de vigilance pour la reprise

1. **Branche non poussée**. Faire `git push -u origin feat/pingora-migration`
   (non fait en mode autonome).
2. **Cargo.lock non tracké**. Chaque contributeur doit `cargo update -p sfv
   --precise 0.9.4` avant de builder `--features pingora`. Décision de
   politique CI à prendre (voir section *Contraintes*).
3. **Armageddon-aegis dep** a été auto-ajoutée par un hook pendant la wave 1
   (dans le commit 2c78d32). Pas de surprise, ligne légitime.
4. **Submodule `poulets-platform/frontend/twitter-mcp`** apparaît modifié en
   `git status` depuis le début de la session — hors scope, ignoré.
5. **Fichiers untracked dans `tests-e2e/tests/06-payments/`** — hors scope
   pingora, ignorés.

## Liens utiles

- Master tracker : [#108](https://github.com/fasodigit/infra/issues/108)
- Sécurité base : commit `8a786e8`
- M0 scaffold : commit `a842a98`
- 5 waves 1 : commits `69682a3` → `4be840d`
- Branch : `feat/pingora-migration` (non poussée)
