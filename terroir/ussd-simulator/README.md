<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# terroir-ussd-simulator

Mock loopback (port `127.0.0.1:1080`) des trois providers USSD/SMS retenus
par [ADR-003](../docs/adr/ADR-003-ussd-provider.md) : **Hub2**, **Africa's
Talking**, **Twilio**. Permet de développer et tester
(`tests-e2e/19-terroir/`) les flows USSD producteur sans avoir à intégrer
les SDKs cloud foreign avant P3+ (décision utilisateur Q7 — souveraineté
incrémentale).

## Surface API mockée

| Route                                  | Provider réel                               | Notes                                                              |
| -------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------ |
| `POST /hub2/ussd/push`                 | Hub2 USSD push                              | Body `{ msisdn, session_id, text, level }`                         |
| `POST /hub2/sms/send`                  | Hub2 SMS                                    | Body `{ msisdn, message }`                                         |
| `POST /africastalking/ussd/menu`       | Africa's Talking USSD callback              | Form ou JSON `sessionId, phoneNumber, networkCode, text`           |
| `POST /africastalking/sms/send`        | Africa's Talking Bulk SMS                   | Form ou JSON `username, to, message`                               |
| `POST /twilio/sms/send`                | Twilio Messages REST                        | Form ou JSON `To, From, Body`                                      |
| `GET /admin/last-sms?msisdn=...`       | (utilitaire test)                           | Renvoie le dernier SMS reçu + OTP extrait (`\b\d{8}\b`)            |
| `GET /admin/sms-history?msisdn=...`    | (utilitaire test)                           | Historique SMS (limit ≤ 50)                                        |
| `GET /admin/sessions/{provider}/{id}`  | (utilitaire test)                           | Inspecte une session USSD persistée KAYA                           |
| `POST /admin/clear`                    | (utilitaire test)                           | Wipe `terroir:ussd:simulator:*` (réinit entre specs Playwright)    |
| `GET /health/ready`, `/health/live`    | —                                           | Health endpoints                                                   |

## Flows métier mockés

1. **`producer-signup`** — `*XXX#` → menu → CNIB → Nom → OTP 8 chiffres
   (envoyé en SMS via le mock même provider) → vérification → `END`.
2. **`payment-confirmation`** — code transaction → OTP → `END`.

L'OTP est stocké dans KAYA `terroir:ussd:otp:{msisdn}` (TTL 5 min, exact
match, suppression au premier match valide pour empêcher replay). Les
événements `terroir.ussd.otp.sent` / `terroir.ussd.otp.verified` sont
loggés via `tracing::info!` (P3 : remplaçable par la lib KAYA producer
quand elle existera).

## Configuration

| Variable env          | Défaut                       | Rôle                                       |
| --------------------- | ---------------------------- | ------------------------------------------ |
| `TERROIR_KAYA_URL`    | `redis://127.0.0.1:6380/0`   | Connection string KAYA (RESP3 sur :6380)   |
| `RUST_LOG`            | `info`                       | Filtre `tracing-subscriber`                |

Le simulator se connecte à KAYA en mode `ConnectionManager` (reconnect
auto). Si KAYA est indisponible, les routes stateful renvoient `503
{ code: "KAYA_UNAVAILABLE" }` mais le binaire continue à tourner pour
permettre `cargo check` et health checks.

### Modèle de données KAYA (P0 string-only)

KAYA en P0 ne supporte que `GET`/`SET`/`DEL` (pas encore `HASH`, `LIST`,
`SCAN`, `EXPIRE`, `SETEX`). Pour préserver la sémantique demandée par
ULTRAPLAN P0.6 (HASH session, LIST SMS, TTL, wipe-by-prefix), on encode :

- **Sessions USSD** : blob JSON sérialisé `SessionState` sous une clé
  STRING (`{prefix}:{provider}:session:{session_id}`).
- **Historique SMS** : tableau JSON `Vec<SmsRecord>` sous une clé STRING
  (`{prefix}:sms:by_msisdn:{msisdn}`), tronqué à 50 éléments lors de
  chaque LPUSH (read-modify-write).
- **TTL** : appliqué côté client via wrapper `TtlEnvelope { expires_at_unix,
  payload }` ; les readers filtrent les entrées expirées et émettent un
  `DEL` best-effort.
- **Wipe** : le simulator maintient un index global
  `{prefix}:_index` (set JSON des clés posées) ; `/admin/clear` itère
  l'index et supprime chaque clé.

Quand KAYA implémentera `EXPIRE`/`HASH`/`LIST`/`SCAN`, on basculera vers
le modèle natif sans changer la surface API exposée par les routers.

## Lancement local

```bash
cd INFRA/terroir
cargo run -p terroir-ussd-simulator
# → écoute sur 127.0.0.1:1080
```

Avec KAYA local :

```bash
cd INFRA/docker/compose
podman-compose -f podman-compose.yml up -d kaya
cd ../../terroir
TERROIR_KAYA_URL="redis://127.0.0.1:6380/0" cargo run -p terroir-ussd-simulator
```

## Validation rapide (curl)

```bash
# Étape 1 — racine flow signup
curl -fsS -X POST http://localhost:1080/hub2/ussd/push \
  -H 'Content-Type: application/json' \
  -d '{"msisdn":"+22670111111","session_id":"sim-1","text":"","level":1}'

# Étape 2 — choix "1" (signup)
curl -fsS -X POST http://localhost:1080/hub2/ussd/push \
  -H 'Content-Type: application/json' \
  -d '{"msisdn":"+22670111111","session_id":"sim-1","text":"1","level":2}'

# Étape 3 — saisie NIN, puis nom → OTP 8 chiffres envoyé en SMS
# … (cf. spec Playwright pour le replay complet)

# Récupère l'OTP capturé (équivalent MailpitClient.waitForOtp)
curl -fsS 'http://localhost:1080/admin/last-sms?msisdn=%2B22670111111' | jq .otp_extracted
```

## Sécurité

- Bind **loopback only** (`127.0.0.1`) — jamais public.
- AGPL-3.0-or-later sur tous les fichiers.
- Aucun secret en config : pas de credentials providers car c'est un mock.
- Pas de tests Cargo dans ce crate — la couverture E2E est portée par la
  spec Playwright `tests-e2e/19-terroir/terroir-ussd-simulator-roundtrip.spec.ts`
  (P0.I).

## Mapping vers les flows Playwright

La spec consomme `/admin/last-sms` exactement comme les autres specs FASO
consomment `MailpitClient.waitForOtp` :

```ts
const sms = await fetch(`http://localhost:1080/admin/last-sms?msisdn=${msisdn}`);
const { otp_extracted } = await sms.json();
expect(otp_extracted).toMatch(/^\d{8}$/);
// → utilisable pour le step "saisie OTP" du flow signup
```
