# FASO DIGITALISATION - Tests E2E Playwright

Tests end-to-end 100% UI headless Chromium pour l'ecosysteme FASO.

## Prerequis

- Bun >= 1.3 (ou Node.js >= 20)
- Stack FASO local demarrer (cf. `INFRA/RUNBOOK-LOCAL-STACK.md`)
- Frontend Angular sur http://localhost:4801
- Mailpit sur http://localhost:8025
- ORY Kratos sur http://localhost:4433 / admin 4434

## Installation

```bash
cd INFRA/tests-e2e
bun install
bun run install-browsers
```

## Commandes

```bash
bun run test            # Tous les tests chromium-headless
bun run test:smoke      # Tests tagges @smoke uniquement
bun run test:acceptance # 01-signup + 02-security + 03-profile + 04-business
bun run test:load       # Tests de charge (skipped par defaut)
bun run test:ui         # Mode interactif Playwright UI
bun run test:debug      # PWDEBUG=1 step-by-step
bun run report          # Ouvrir rapport HTML
bun run analyze         # Stats timings p50/p95/p99
```

## Arborescence

```
tests-e2e/
├── fixtures/       # MailpitClient, TotpGen, WebAuthn, Kratos, 25 actors Faker fr
├── page-objects/   # Signup, Login, Dashboard, Security, Profile, Marketplace, Messaging
├── tests/
│   ├── 01-signup/   # 5 roles : eleveur, pharma, aliments, vaccins, client
│   ├── 02-security/ # TOTP, PassKey, backup codes
│   ├── 03-profile/  # SIRET, AMM, licence
│   ├── 04-business/ # offers, demands, match, checkout
│   └── 05-load/     # 5000 transactions (skip par defaut)
└── scripts/         # analyze-timings.ts
```

## Variables d'environnement

Voir `.env.example`. Surcharge via `.env` locale.

## Validation installation

```bash
bunx tsc --noEmit
bunx playwright test --list
```

## Simulation complete (Chrome headless)

### Prerequis
- Google Chrome installe systeme (`bunx playwright install chrome`)
- Stack FASO tout up (voir /status-faso)

### Commandes

```bash
# Smoke Chrome (tests tagges @smoke, ~15s)
bun run test:chrome:smoke

# Simulation complete orchestree avec auto-classify des bugs
bun run simulation

# Charge petite (10 clients x 2 tx = 20 tx) - validation harness
bun run load:small

# Charge moyenne (100 x 3 = 300 tx)
bun run load:medium

# Charge full OVH A7 (1000 x 5 = 5000 tx, p95<150ms)
bun run load:full && bun run analyze
```

### Boucle d'auto-correction

`bun run simulation` lance jusqu'a 5 iterations de la suite d'acceptance, classifie
chaque echec (selector-missing, otp-not-received, kaya-protocol-error, backend-5xx,
kratos-flow-error, navigation-timeout, ...) et sort `reports/iter-N/failures.classified.json`
avec un agent cible suggere (`kaya-rust-implementer`, `general-purpose-frontend`,
`general-purpose-backend`, `devops-engineer`, `manual-review`).

En mode TTY, le script fait une pause entre iterations pour laisser Claude principal
dispatcher les fixes aux agents. En mode non-TTY (CI), il sort avec code 2 apres
la premiere iteration rouge.
