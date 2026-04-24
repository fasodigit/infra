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
| e5ef107 | ctx typed slots + CORS/VEIL refactor (M1 consolidation) | #102 | +83 / -43 | 135/135 |
| 945ff46 | JWT ES384 filter (close #97 M1 wave 2) | #97 | +715 | 135/135 |
| 7e2a341 | feature-flag filter + bug_005 preserved (close #98 M1 wave 2) | #98 | +448 | 135/135 |
| 4807944 | OTEL traceparent filter + logging hook (close #99 M1 wave 2) | #99 | +422 | 135/135 |
| 8cef15e | fix GqlLimitError exhaustive match (UnknownFragment + CyclicFragments) | #— | +3 / -1 | cargo check OK |
| 314a89d | feat(armageddon-forge,mtls): SPIFFE-aware upstream mTLS (close M2-mtls wave 2) | M2 | +410 sur 3 fichiers | 186/186 |
| 95ef6ac | feat(armageddon-forge,circuit-breaker): Closed/Open/HalfOpen state machine (close M2-cb wave 2) | M2 | +629 sur 1 fichier | 186/186 |
| 8f5d5e1 | feat(armageddon-forge,health): background health checker ArcSwap (close M2-health wave 2) | M2 | +794 sur 1 fichier | 186/186 |
| 0f2ede3 | feat(armageddon-forge,retry): armageddon-retry Pingora adapter (close M2-retry wave 2) | M2 | +492 sur 2 fichiers | 186/186 |
| 997af61 | feat(armageddon-forge,aegis): enrich HttpRequest/ConnectionInfo from RequestCtx (close M3-1 wave 2) | #104 | +72 / -18 sur 1 fichier | 186+/186 |
| b1595c7 | feat(armageddon-forge,sentinel): port WAF/GeoIP/JA4 adapter (close M3-2 wave 2) | #104 | +294 / -24 sur 2 fichiers | 213/213 |
| bf2ac37 | feat(armageddon-forge,arbiter): port anomaly detection adapter (close M3-3 wave 2) | #104 | +214 / -21 sur 1 fichier | 213/213 |
| 9084e33 | feat(armageddon-forge,oracle): port ML scoring adapter with OTEL propagation (close M3-4 wave 2) | #104 | +249 / -21 sur 1 fichier | 213/213 |
| daaa2eb | feat(armageddon-forge,nexus): port aggregator adapter — fuse engine signals into verdict (close M3-5 wave 2) | #104 | +348 / -21 sur 1 fichier | 213/213 |
| 3eec3db | feat(armageddon-forge,ai): port AI-assisted triage adapter (close M3-6 wave 2) | #104 | +381 / -16 sur 2 fichiers | 220/220 |
| 1004628 | feat(armageddon-forge,wasm): port Proxy-Wasm adapter with dedicated thread + channel (close M3-7 wave 2) | #104 | +387 / -15 sur 1 fichier | 226/226 |

**Total code ajouté** : environ 11 332 LOC net nouveaux + 898 LOC sécurité préservée.
**Total tests** : 213/213 pass sur `cargo test -p armageddon-forge --features pingora --lib pingora`.

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
| [#97 jwt](https://github.com/fasodigit/infra/issues/97) | **done wave 2** | 715 | 13/13 |
| [#98 feature-flag](https://github.com/fasodigit/infra/issues/98) | **done wave 2** | 448 | 10/10 |
| [#99 otel](https://github.com/fasodigit/infra/issues/99) | **done wave 2** | 422 | 13/13 |
| [#100 veil](https://github.com/fasodigit/infra/issues/100) | **done wave 1** | 435 | 12/12 |

**M1 wave 2 — TERMINÉE (commits e5ef107 → 4807944)** :
- JWT : `JwtFilter` avec JWKS cache KAYA 300s + in-process fallback; `KayaJwtBackend`
  trait + `NoopKayaBackend` pour tests. 13 tests.
- feature-flag : `FeatureFlagFilter` avec scrub inconditionnel `X-Faso-Features`
  préservé (bug_005); 3 tests de régression verbatim portés.
- otel : `OtelFilter` avec `Traceparent` parser W3C, injection upstream hop,
  `on_logging` avec `duration_ms` + `http.status_code`. 13 tests.
- ctx.rs consolidation : `cors_origin`, `veil_nonce`, `bearer_token`, `span_id`,
  `request_start_ms` slots typés; `CorsFilter` et `VeilFilter` refactorisés.
- `CSP_NONCE_STASH_PREFIX` marqué `#[deprecated]` (compat seule).
- **TLS detection** : `session.is_tls()` n'existe pas en Pingora 0.3. VEIL
  utilise `X-Forwarded-Proto: https` comme fallback. Upgrade path :
  `session.digest().ssl_digest.is_some()` quand l'API se stabilise.

### Gate #103 — M2 Machinerie upstream **(vague 2, 6/6 modules — TERMINÉ)**

| Module | État | Notes |
|---|---|---|
| `selector.rs` | **done wave 1** — 610 LOC | PoolKey SPIFFE-aware (bug_006 préservé) + ClusterResolver hot-reload + résolution fail-closed |
| `lb.rs` | **done wave 1** — 245 LOC | Round-robin complet (6/6 tests). Weighted + P2C : `todo!()` |
| `mtls.rs` | **done wave 2** — 324 LOC | SpiffeChecker + UpstreamMtlsFilter; ctx.spiffe_peer_expected slot; fail-closed bug_006 |
| `circuit_breaker.rs` | **done wave 2** — 400 LOC | Closed/Open/HalfOpen; DashMap+AtomicU32+RwLock; cooldown doubling; error-rate threshold |
| `health.rs` | **done wave 2** — 540 LOC | PingoraHealthChecker; ArcSwap publish; Http/Tcp/Grpc probes; threshold transitions |
| `retry.rs` | **done wave 2** — 360 LOC | PingoraRetryPolicy wrapping armageddon-retry; Retry-After; budget; cb non-interference |

**Invariant sécurité préservé** : `ClusterResolver::resolve()` retourne `None`
+ `error!` log (**jamais** fallback plaintext) quand
`tls_required && expected_spiffe_id.is_none()`. Verifié par
`resolver_mtls_without_expected_spiffe_fails`.

### Gate #104 — M3 8 moteurs sécurité **(batch A 5/7, wave 2)**

| Module | État | LOC | Tests |
|---|---|---:|---:|
| `pipeline.rs` | **done wave 1** | 404 | 6/6 |
| `aegis_adapter.rs` | **done wave 2** (enrichi M3-1) | ~320 | 4/4 |
| `sentinel_adapter.rs` | **done wave 2** (WAF/GeoIP/JA4) | ~280 | 6/6 |
| `arbiter_adapter.rs` | **done wave 2** (Aho-Corasick CRS) | ~220 | 6/6 |
| `oracle_adapter.rs` | **done wave 2** (ONNX + OTEL) | ~260 | 6/6 |
| `nexus_adapter.rs` | **done wave 2** (aggregator brain) | ~390 | 8/8 |
| `ai_adapter.rs` | **done wave 2** (threat-intel + prompt-injection) | ~280 | 6/6 |
| `wasm_adapter.rs` | **done wave 2** (OS thread + channel, empty runtime) | ~390 | 7/7 |

Pipeline utilise `FuturesUnordered` + `tokio::time::timeout` par moteur.
Drop = cancel des futures en vol → short-circuit Deny efficace.

**M3 batch B — TERMINÉ** (commits 3eec3db → 1004628, 2026-04-24) :

- **M3-6 (AI)** : délègue à `armageddon_ai::AiEngine::inspect()`.
  Threat-intel IoC lookups + prompt-injection heuristic scorer.
  Short-circuit si `ctx.ai_score >= 0.9`. Trait `AiProvider` +
  `NoopAiProvider` (production) + `MockAiProvider` (tests) pour
  contextualisation LLM future sans toucher l'adapter. Timeout 30 ms.

- **M3-7 (WASM)** : thread OS dédié + `async_channel::unbounded`.
  Wasmtime `Store`/`Instance` sont `!Send` — ils restent confinés au
  thread worker qui tourne son propre `new_current_thread` tokio runtime.
  `WasmCtxSnapshot` serialise les champs de `RequestCtx` pour traverser
  le channel. Runtime vide (plugin loading est TODO dans
  `armageddon-wasm/src/runtime.rs`) → retourne toujours `Allow{0.0}`.
  Fail-open sur timeout 100 ms. Voir TODO(M5) ci-dessous.

**M3 COMPLET — 7/7 adapters** (commits 997af61 → 1004628).

**M3 batch A — TERMINÉ** (commits 997af61 → daaa2eb, 2026-04-24) :

- **M3-1 (AEGIS enrichissement)** : `request_context_from_ctx()` forwarde
  désormais `user_id`, `tenant_id`, `roles`, `bearer_token`, `cluster`,
  `request_id`, `trace_id` comme headers et champs `RequestContext`. Les
  policies Rego voient les vraies valeurs issues du M1 JWT/router.

- **M3-2 (SENTINEL)** : délègue à `armageddon_sentinel::Sentinel::inspect()`.
  Short-circuit `ctx.waf_score >= 0.9`. IPS + GeoIP + JA3/JA4 + DLP.
  Timeout 15 ms.

- **M3-3 (ARBITER)** : délègue à `armageddon_arbiter::Arbiter::inspect()`.
  Aho-Corasick + OWASP CRS v4, anomaly scoring. Flag→Allow(confidence).
  Timeout 20 ms.

- **M3-4 (ORACLE)** : délègue à `armageddon_oracle::Oracle::inspect()`.
  22-feature ONNX model. OTEL context propagé via `tracing::debug!` avec
  `trace_id`/`span_id` (full OTLP export en M6). Timeout 25 ms.

- **M3-5 (NEXUS)** : lit `ctx.waf_score` + `ctx.ai_score`, synthétise
  des `Decision` avec severity-scaling pour préserver le score dans le
  `CompositeScorer` pondéré. Multi-vector boost +0.2. Block→Deny,
  Challenge/LogOnly→Allow. Timeout 10 ms.

**Nouvelles dépendances** (ajoutées au `Cargo.toml` forge) :
`armageddon-sentinel`, `armageddon-arbiter`, `armageddon-nexus` (batch A) ;
`armageddon-ai`, `armageddon-wasm`, `async-channel = "2"` (batch B).

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

## Matrices de vérification

### Fin vague 1

| Commande | Résultat |
|---|---|
| `cargo check -p armageddon-forge` | ✅ clean (1 warning pré-existant dans `feature_flags.rs`) |
| `cargo check -p armageddon-forge --features pingora` | ✅ clean |
| `cargo test -p armageddon-forge --features pingora --lib pingora` | ✅ **94/94 passed** |
| `cargo bench --bench pingora_filter_chain_micro --features pingora --no-run` | ✅ compile |
| `bash -n benches/pingora_vs_hyper.sh` | ✅ clean |

### Fin M1 wave 2 (2026-04-24)

| Commande | Résultat |
|---|---|
| `cargo check -p armageddon-forge --features pingora` | ✅ clean (1 warning pré-existant dans `feature_flags.rs`) |
| `cargo check -p armageddon` | ✅ clean (GqlLimitError fix commit 8cef15e) |
| `cargo test -p armageddon-forge --features pingora --lib pingora` | ✅ **135/135 passed** |
| `cargo test -p armageddon-forge --lib feature_flag_filter` | ✅ **7/7 passed** (bug_005 regression) |

### Fin M2 wave 2 (2026-04-24)

| Commande | Résultat |
|---|---|
| `cargo check -p armageddon-forge --features pingora` | ✅ clean (1 warning pré-existant dans `feature_flags.rs`) |
| `cargo check -p armageddon` | ✅ clean (GqlLimitError fix inchangé) |
| `cargo test -p armageddon-forge --features pingora --lib pingora` | ✅ **186/186 passed** (+51 nouveaux tests M2) |

### Fin M3 batch A (5/7 adapters) wave 2 (2026-04-24)

| Commande | Résultat |
|---|---|
| `cargo check -p armageddon-forge --features pingora` | ✅ clean (1 warning pré-existant dans `feature_flags.rs`) |
| `cargo check -p armageddon` | ✅ clean (GqlLimitError fix inchangé) |
| `cargo test -p armageddon-forge --features pingora --lib pingora` | ✅ **213/213 passed** (+27 nouveaux tests M3 batch A) |

### Fin M3 batch B (7/7 adapters) wave 2 (2026-04-24)

| Commande | Résultat |
|---|---|
| `cargo check -p armageddon-forge --features pingora` | ✅ clean (1 warning pré-existant dans `feature_flags.rs`) |
| `cargo check -p armageddon` | ✅ clean |
| `cargo test -p armageddon-forge --features pingora --lib pingora` | ✅ **226/226 passed** (+13 nouveaux tests M3 batch B: 6 AI + 7 WASM) |

## TODOs documentés (M3 wave 2 et au-delà)

- **JWT session cache** (`jwt:session:<sha256(token)>`): le spec M1 wave 2
  mentionnait un cache par token en plus du cache JWKS. Non implémenté car
  il nécessite une stratégie de révocation cohérente (jti blacklist dans KAYA).
  Déféré à M3 wave 2 avec les adapters moteur.
- **OTEL tracing::Span guard** : le span est actuellement loggé via
  `tracing::info!` uniquement. Pour un export OTEL complet (Tempo/Jaeger),
  câbler `tracing-opentelemetry` subscriber au démarrage du serveur Pingora
  (M6 cutover tâche).
- **aegis_adapter.rs placeholder** : construit `HttpRequest` avec chaînes
  vides (TODO depuis wave 1). Peut maintenant être enrichi avec `RequestCtx`
  (user_id, tenant_id, cluster disponibles) — tâche M3 wave 2.
- **mTLS connector wire-up** (`upstream/mtls.rs:TODO(M5)`) : en Pingora 0.3
  il n'existe pas de hook `upstream_connect` exposé via `ProxyHttp`.
  `UpstreamMtlsFilter` valide le peer SPIFFE ID post-hoc via `ctx.spiffe_peer`.
  Le vrai dial mTLS (AutoMtlsDialer + tokio_rustls) sera câblé en M5 wave 2
  quand pingora-rustls exposera un custom connector.
- **gRPC health probe** (`upstream/health.rs:probe_grpc`) : actuellement
  fallback TCP. Port réel du protocole gRPC Health (grpc.health.v1.Health/Check)
  prévu en M4 wave 2 avec le module grpc_web.rs.
- **WASM plugin loading** (`engines/wasm_adapter.rs:run_plugins_sync`
  + `armageddon-wasm/src/runtime.rs:TODO`): le scan de `plugins_dir`
  et l'exécution réelle des modules `.wasm` sont marqués TODO dans
  `PluginRuntime`. L'adapter retourne actuellement `EngineVerdict::Allow
  {score:0.0}` pour toute requête (empty runtime). Implémenter en M5
  avec `PluginRuntime::load_plugin` loop + fuel/gas hérité de la config
  wasmtime existante. Référence: issue #106.
- **AI LLM provider** (`engines/ai_adapter.rs:HttpAiProvider`): le trait
  `AiProvider` est en place mais seul `NoopAiProvider` est câblé. Un
  provider HTTP (Anthropic/OpenAI) peut être ajouté en M5/M6 derrière
  un feature flag sans modifier l'adapter.
- **LB Weighted + P2C** (`upstream/lb.rs`) : `todo!()` depuis wave 1.
  Déféré à M3 wave 2.
- **Prometheus histogram** (`upstream/health.rs:emit_probe_duration`) :
  OnceLock registration non terminée. Câbler en M5 wave 2 avec le wiring
  Prometheus complet du gateway.

## Ce qui reste (wave 2, après M3 batch A)

Classement par ordre d'impact, pour reprise de session :

1. **M1 wave 2 — TERMINÉE** (commits e5ef107 → 4807944 + 8cef15e)

2. **M2 wave 2 — TERMINÉE** (commits 314a89d → 0f2ede3)
   - mtls: SpiffeChecker + UpstreamMtlsFilter + ctx.spiffe_peer_expected
   - circuit_breaker: Closed/Open/HalfOpen + DashMap + CircuitBreakerManager
   - health: PingoraHealthChecker + ArcSwap publish + Http/Tcp/Grpc probes
   - retry: PingoraRetryPolicy + armageddon-retry adapter + Retry-After

3. **M3 — TERMINÉE (7/7)** (commits 997af61 → 1004628)
   - aegis: enrichissement RequestCtx → Rego (M3-1)
   - sentinel: IPS + GeoIP + JA3/JA4 + DLP (M3-2)
   - arbiter: Aho-Corasick CRS WAF (M3-3)
   - oracle: ONNX 22-feature + OTEL propagation (M3-4)
   - nexus: aggregator brain composite scoring (M3-5)
   - ai: threat-intel IoC + prompt-injection + AiProvider trait (M3-6)
   - wasm: OS thread + async_channel + empty plugin runtime (M3-7)

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
