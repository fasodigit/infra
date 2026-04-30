<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Suite E2E `19-terroir` — P0.I (4 specs) + P1.G (8 specs) = 12 specs total

Suite Playwright qui valide :
1. **P0** infrastructure TERROIR (tenant provisioning, USSD, Vault Transit,
   Keto namespaces) — 4 specs livrées en P0.I.
2. **P1** modules MVP (terroir-core, terroir-eudr, terroir-mobile-bff,
   terroir-web-admin) — 8 specs livrées en P1.G.

Toutes les specs hit les services réels — pas de mocks (cf.
`INFRA/CLAUDE.md` §11). Les seules portions "stubées" sont les providers
externes que TERROIR ne contrôle pas (TRACES NT, Hub2, AT, Twilio) — leurs
stubs sont **côté serveur** (binaire Rust), pas côté Playwright.

## Mapping spec ↔ module testé

### P0.I (4 specs — infrastructure)

| Spec | Module P0 | Endpoint(s) clés | Assertion clé | Budget P99 |
|------|-----------|------------------|---------------|------------|
| `terroir-tenant-provisioning.spec.ts` | **P0.C** `terroir-admin :9904` | `POST/GET/POST suspend /admin/tenants` | tenant onboarding < 5 min, status ACTIVE → SUSPENDED | < 5 min total |
| `terroir-ussd-simulator-roundtrip.spec.ts` | **P0.F** `terroir-ussd-simulator :1080` | Hub2 push, AT, Twilio, `/admin/last-sms` | OTP 8-digits capturé via regex, flow Hub2 5 steps `END Inscription validée` | < 5 s |
| `terroir-vault-transit-encrypt-decrypt.spec.ts` | **P0.B** Vault Transit `terroir-pii-master` | `/v1/transit/encrypt|decrypt|keys` | round-trip encrypt/decrypt avec context, key info `derived=true` `auto_rotate_period=2160h` | < 500 ms |
| `terroir-keto-tenant-namespace.spec.ts` | **P0.D** Keto namespaces TERROIR | Read :4466 + Write :4467 | tuples seedés ≥ 2, subject_set Cooperative→Tenant, write/read/delete round-trip | < 200 ms par check |

### P1.G (8 specs — modules MVP)

| # | Spec | Module(s) testé(s) | Endpoint(s) clés | Assertion clé |
|---|------|--------------------|------------------|---------------|
| 1 | `terroir-producer-create-with-pii-encryption.spec.ts` | **P1.1** core + **P1.2** Vault Transit | `POST /api/terroir/core/producers` + SQL probe `terroir_t_*.producer` | PII chiffrées en DB (`full_name_encrypted IS NOT NULL`, ciphertext ≠ plaintext), GET round-trip clair côté service |
| 2 | `terroir-parcel-polygon-crdt-merge.spec.ts` | **P1.1** core (Yjs polygon) | `POST /parcels/{id}/polygon` × 2 sessions, `GET /parcels/{id}/polygon` | yjsVersion strictement croissante, état final ≥ chaque delta (preuve fusion CRDT) |
| 3 | `terroir-eudr-validation-happy-path.spec.ts` | **P1.3** eudr + Hansen mirror | `POST /api/terroir/eudr/validate` × 2 (cache MISS → HIT) | status=VALIDATED, header `X-Eudr-Cache-Status: MISS` puis `HIT`, polygonHash stable |
| 4 | `terroir-eudr-validation-deforested-rejected.spec.ts` | **P1.3** eudr + Hansen | `POST /eudr/validate` (synthetic deforested polygon) | status=REJECTED|ESCALATED, deforestationOverlapHa > 0, ddsDraftId null |
| 5 | `terroir-dds-generation-and-submission.spec.ts` | **P1.3** eudr + Vault PKI + TRACES NT mock | `POST /eudr/dds`, `/sign`, `/submit`, `GET /download` | DDS PDF magic bytes `%PDF-`, signatureFingerprint hex, status=submitted, attemptNo ≥ 1 |
| 6 | `terroir-agent-offline-sync-roundtrip.spec.ts` | **P1.5** mobile-bff REST batch | `POST /m/sync/batch` 50 items | acks 50/50, ≥ 90% ok, latence < 2 min (budget EDGE), idempotency 409 sur replay |
| 7 | `terroir-jwt-revocation-on-sync.spec.ts` | **P1.5** mobile-bff + KAYA flag | `POST /m/sync/batch` × 2, `KAYA SET auth:agent:revoked:*` entre les 2 | sync 1 = 200, sync 2 = 401/403 (skip propre si revocation pas encore enforcée) |
| 8 | `terroir-tenant-isolation.spec.ts` | **P1.1** core + Keto + RLS + admin | `POST /admin/tenants` (B), `GET /producers/{A}` depuis B | 403/404, SQL probe `terroir_t_B.producer` retourne 0 rows pour producteur A |

## Prérequis stack (vérifié par `cycle-fix` AVANT `playwright test`)

Cf. `INFRA/CLAUDE.md` §10 (cycle-fix avant E2E) :

```bash
# Vault + Consul + Postgres + Keto + Kratos + Mailhog + Redpanda
podman-compose -f INFRA/docker/compose/podman-compose.yml \
               -f INFRA/vault/podman-compose.vault.yml up -d

bash INFRA/vault/scripts/init.sh                 # idempotent
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
bash INFRA/vault/scripts/seed-secrets.sh
bash INFRA/terroir/scripts/bootstrap-vault-transit.sh   # P0.B Transit + PKI
bash INFRA/terroir/scripts/seed-topics.sh               # P0.E topics
bash INFRA/terroir/scripts/register-avro.sh             # P0.E schemas

# terroir services (P0.C + P0.F + P1)
cargo run -p terroir-admin --bin terroir-admin &           # :9904
cargo run -p terroir-ussd-simulator --bin terroir-ussd-simulator &  # :1080
cargo run -p terroir-core --bin terroir-core &             # :8830 + gRPC :8730
cargo run -p terroir-eudr --bin terroir-eudr &             # :8831 + gRPC :8731
cargo run -p terroir-mobile-bff --bin terroir-mobile-bff & # :8833 (REST + WS)

# ARMAGEDDON gateway (P0.H)
cargo run -p armageddon &                                  # :8080
```

État GREEN attendu (cf. `/status-faso`) :

- Vault `:8200` initialized + unsealed ; Transit key `terroir-pii-master`
  + PKI mount `pki-eudr` configurés.
- Postgres : schémas `terroir_shared`, `terroir_t_t_pilot`,
  `audit_t_t_pilot` créés ; rôle `terroir_app` opérationnel.
- Keto Read `:4466` + Write `:4467` healthy + namespaces enregistrés.
- ARMAGEDDON `:8080` route `/api/terroir/{core,eudr,mobile-bff,...}/*`.
- terroir-* services tous health `200 OK`.

## Données de cycle-fix obligatoires AVANT P1.G

Pour que les specs P1 passent, le cycle-fix doit pré-seeder :

1. **Tenant pilote** `t_pilot` provisionné via terroir-admin (P0.J).
2. **Coopérative pilote** dans `terroir_shared.cooperative` avec UUID
   passé en `TERROIR_COOP_PILOT_UUID` (sinon les specs créent producteurs
   sur un coop_id orphelin → FK violation).
3. **Hansen GFC mirror tile** :
   `gs://terroir-hansen-mirror/lossyear-2024-v1.7.tif` accessible par
   terroir-eudr (sinon `/eudr/validate` retourne 503).
4. **Rôle PG `terroir_app`** + grants RLS validés sur les schémas
   tenants (sinon `pg-probe.ts` ne peut pas lire / asserter).
5. **KAYA `:6380`** disponible pour les flags `auth:agent:revoked:*`.

## Variables d'environnement

| Variable | Valeur par défaut | Usage |
|----------|-------------------|-------|
| `TERROIR_GATEWAY_URL` | `http://localhost:8080` | ARMAGEDDON base URL pour core/eudr/mobile-bff clients |
| `TERROIR_TENANT_SLUG` | `t_pilot` | tenant slug par défaut (X-Tenant-Slug header) |
| `TERROIR_COOP_PILOT_UUID` | placeholder | UUID coopérative pilote seedée |
| `TERROIR_AMINATA_UUID` | placeholder | UUID Kratos d'Aminata seedée |
| `TERROIR_TEST_USER_ID` | `anonymous` | userId pour le flag KAYA `auth:agent:revoked:{userId}` |
| `TERROIR_ADMIN_URL` | `http://localhost:9904` | tenant-admin-client (P0.C) |
| `TERROIR_USSD_SIMULATOR_URL` | `http://localhost:1080` | ussd-simulator-client (P0.F) |
| `VAULT_ADDR` | `http://localhost:8200` | vault-transit-client (P0.B) |
| `VAULT_TOKEN` | *(requis)* | export `$(jq -r .root_token ~/.faso-vault-keys.json)` |
| `KETO_READ_URL` | `http://localhost:4466` | keto-client (P0.D) |
| `KETO_WRITE_URL` | `http://localhost:4467` | keto-client (P0.D) |
| `TERROIR_PG_URL` | `postgresql://terroir_app:terroir_app@localhost:5432/auth_ms` | pg-probe (PII + RLS proofs) |
| `KAYA_URL` | `redis://localhost:6380` | kaya-probe (revocation flags) |
| `E2E_AMINATA_PASSWORD` / `E2E_SOULEYMANE_PASSWORD` | dev fallback | SUPER-ADMIN passwords (CLAUDE.md §12) |

## Lancer la suite

```bash
cd INFRA/tests-e2e
bunx playwright test tests/19-terroir/

# Sub-suites par module :
bunx playwright test tests/19-terroir/terroir-producer-create-with-pii-encryption.spec.ts
bunx playwright test tests/19-terroir/terroir-eudr-*.spec.ts
bunx playwright test tests/19-terroir/terroir-agent-offline-sync-roundtrip.spec.ts
bunx playwright test tests/19-terroir/terroir-tenant-isolation.spec.ts

# Lister sans exécuter :
bunx playwright test --list tests/19-terroir/
```

## Dépendances optionnelles (probes bas-niveau)

Les fixtures `pg-probe.ts`, `kaya-probe.ts` et `mobile-bff-client.ts`
chargent dynamiquement `pg`, `redis`, `ws` (`import('pg')` etc.). Sans
ces packages installés OU si la connexion échoue, les helpers retournent
`{unavailable: true}` et les specs `test.skip` proprement (ils ne fail
pas).

Pour activer les assertions bas-niveau, installer :

```bash
cd INFRA/tests-e2e
bun add -d pg @types/pg redis ws @types/ws
```

Cf. CLAUDE.md §11 — un test qui ne peut pas hit la stack réelle ne doit
**jamais** faire un `expect(true).toBe(true)` ; le `test.skip(true,
'<raison>')` est l'option canonique.

## Statut P1.G

Les 8 specs P1 sont **livrées et compilent** mais leur GREEN dépend
strictement de l'état stack (cf. cycle-fix prerequis ci-dessus). La phase
P1.H (cycle-fix → execute → GREEN) est portée séparément.

## Anti-patterns à éviter (CLAUDE.md §11/§12)

- Ne pas mocker un endpoint backend via `page.route(...)`.
- Ne pas hardcoder un mot de passe SUPER-ADMIN dans la spec (toujours via
  env `E2E_*_PASSWORD`).
- Ne pas appeler les services directement (toujours via `:8080` ARMAGEDDON).
- Ne pas faire `expect(true).toBe(true)` quand un service est down — `test.skip`.
- Si une spec révèle un bug d'infra → retour `/cycle-fix`, pas de fix dans la spec.
