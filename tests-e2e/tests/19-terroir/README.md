<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Suite E2E `19-terroir` — scaffolds P0.I

Suite Playwright qui valide l'**infrastructure P0** TERROIR avant
l'écriture du code métier P1. Les 4 specs hit les services réels — pas de
mocks (cf. `INFRA/CLAUDE.md` §11).

Cible : ULTRAPLAN §4 P0.10 + Gate G1 (§15) — *« 4 specs Playwright P0
GREEN + Vault Transit OK + tenant provisioning < 5min »*.

## Mapping spec ↔ module testé

| Spec | Module P0 | Endpoint(s) clés | Assertion clé | Budget P99 |
|------|-----------|------------------|---------------|------------|
| `terroir-tenant-provisioning.spec.ts` | **P0.C** `terroir-admin :9904` | `POST/GET/POST suspend /admin/tenants` | tenant onboarding < 5 min, status ACTIVE → SUSPENDED | < 5 min total |
| `terroir-ussd-simulator-roundtrip.spec.ts` | **P0.F** `terroir-ussd-simulator :1080` | Hub2 push, AT, Twilio, `/admin/last-sms` | OTP 8-digits capturé via regex, flow Hub2 5 steps `END Inscription validée` | < 5 s |
| `terroir-vault-transit-encrypt-decrypt.spec.ts` | **P0.B** Vault Transit `terroir-pii-master` | `/v1/transit/encrypt|decrypt|keys` | round-trip encrypt/decrypt avec context, key info `derived=true` `auto_rotate_period=2160h` | < 500 ms |
| `terroir-keto-tenant-namespace.spec.ts` | **P0.D** Keto namespaces TERROIR | Read :4466 + Write :4467 | tuples seedés ≥ 2, subject_set Cooperative→Tenant, write/read/delete round-trip | < 200 ms par check |

## Prérequis stack (vérifié par `cycle-fix` AVANT `playwright test`)

Cf. `INFRA/CLAUDE.md` §10 (cycle-fix avant E2E) :

```bash
# Vault + Consul
podman-compose -f INFRA/docker/compose/podman-compose.yml \
               -f INFRA/vault/podman-compose.vault.yml up -d consul vault
bash INFRA/vault/scripts/init.sh                # idempotent
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
bash INFRA/vault/scripts/seed-secrets.sh
# (P0.B) bootstrap Vault Transit + PKI :
bash INFRA/terroir/scripts/bootstrap-vault-transit.sh

# Postgres + Keto + Kratos
podman-compose -f INFRA/docker/compose/podman-compose.yml up -d \
               postgres keto-migrate keto-read keto-write kratos mailhog

# Redpanda + 22 topics + 8 schemas
podman-compose -f INFRA/docker/compose/podman-compose.yml up -d redpanda
bash INFRA/terroir/scripts/seed-topics.sh
bash INFRA/terroir/scripts/register-avro.sh

# terroir services Rust (P0.C, P0.F)
cargo run -p terroir-admin --bin terroir-admin &           # :9904
cargo run -p terroir-ussd-simulator --bin terroir-ussd-simulator &  # :1080

# ARMAGEDDON gateway (P0.H)
cargo run -p armageddon &                                    # :8080
```

État GREEN attendu (cf. `/status-faso`) :

- Vault `:8200` initialized + unsealed.
- Postgres schemas `terroir_shared` + `terroir_t_t_pilot` + `audit_t_t_pilot` créés.
- Keto Read `:4466` + Write `:4467` healthy avec namespaces enregistrés.
- terroir-admin `:9904` health `200 OK`.
- terroir-ussd-simulator `:1080` health `200 OK`.

## Variables d'environnement

| Variable | Valeur par défaut | Usage |
|----------|-------------------|-------|
| `TERROIR_ADMIN_URL` | `http://localhost:9904` | tenant-admin-client |
| `TERROIR_USSD_SIMULATOR_URL` | `http://localhost:1080` | ussd-simulator-client |
| `VAULT_ADDR` | `http://localhost:8200` | vault-transit-client |
| `VAULT_TOKEN` | *(requis)* | export `$(jq -r .root_token ~/.faso-vault-keys.json)` |
| `KETO_READ_URL` | `http://localhost:4466` | keto-client |
| `KETO_WRITE_URL` | `http://localhost:4467` | keto-client |
| `TERROIR_AMINATA_UUID` | placeholder | UUID Kratos d'Aminata seedé en P0.D |
| `TERROIR_COOP_PILOT_UUID` | placeholder | UUID coopérative pilote seedé en P0.D |

> Les UUIDs `TERROIR_AMINATA_UUID` / `TERROIR_COOP_PILOT_UUID` doivent
> matcher exactement les valeurs émises par les seeds P0.D — à passer en
> env ou ré-écrire le seeder pour produire des valeurs déterministes.

## Lancer la suite

```bash
cd INFRA/poulets-platform/e2e
bunx playwright test tests/19-terroir/
```

Sous-set d'un seul module :

```bash
bunx playwright test tests/19-terroir/terroir-vault-transit-encrypt-decrypt.spec.ts
```

## Statut P0.I

Les 4 specs sont **scaffolded mais non encore exécutées** (P0.I = écriture
seulement). L'exécution est portée par P0.J `cycle-fix` qui doit d'abord
amener tous les services TERROIR au statut GREEN. Cf. ULTRAPLAN §4 P0.10.

## Anti-patterns à éviter (CLAUDE.md §10/§11)

- Ne pas lancer la suite tant que `cycle-fix` n'a pas convergé.
- Ne pas mocker les services — les fixtures hit Vault/Keto/admin/USSD réels.
- Si une spec révèle un bug d'infra → retour cycle-fix, pas de fix dans la spec.
