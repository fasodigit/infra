# Playwright Full-Stack E2E — guide d'adaptation cross-projet

> **Public** : équipes qui démarrent ou refont leur stack E2E à partir de l'expérience FASO DIGITALISATION.
> **Objectif** : adapter ce squelette à votre projet en < 1 semaine, avec couverture browser→gateway→backend, observabilité Jaeger/Tempo, P50/P95/P99, et cycle-fix automatisé.

---

## TL;DR — checklist d'adaptation en 7 étapes

1. **Adopter la structure de répertoires** `tests-e2e/` (§3) et copier les page objects
2. **Adapter `fixtures/actors.ts`** au domaine (rôles, contraintes locale, format pays)
3. **Configurer `playwright.config.ts`** avec 3 projets (chromium-headless, smoke, chrome-headless-new)
4. **Adapter le `global-setup.ts`** pour wiper l'état IDP entre runs
5. **Câbler observabilité** : OTel SDK browser + BFF + backend, exporter vers OTLP collector
6. **Implémenter `cycle-fix`** + `simulation-data-real` agent (loop start→test→fix→re-run jusqu'à GREEN)
7. **Ajouter suites P99** : load test 100+ users avec extraction Tempo TraceQL

Le reste du document détaille chaque étape, avec les pièges rencontrés et les patterns réutilisables.

---

## 1. Périmètre & philosophie

### 1.1 Niveaux de test

| Niveau | Objet | Outils |
|--------|-------|--------|
| Unit | une classe / une fonction | JUnit, Jest, cargo test |
| Integration | un service + ses deps directes | Testcontainers, Spring Boot Test |
| Contract | OpenAPI / GraphQL / gRPC schema | Pact, Schemathesis |
| **E2E Full-Stack** | **browser → gateway → service mesh → backend → DB** | **Playwright + observabilité** |
| Load / P99 | E2E sous charge concurrente | Playwright workers, k6, Artillery |

Cette doc se concentre sur les deux derniers niveaux. Les autres sont des prérequis.

### 1.2 Principes directeurs FASO

- **« Real over mock »** — pas de mock côté backend, on teste contre les vrais services dans des containers.
- **« Trace-driven debug »** — chaque test échoué doit produire un trace ID Jaeger pour analyser le path complet.
- **« Browser hits the gateway »** — `baseURL` pointe sur la gateway souveraine (ARMAGEDDON / Envoy / Kong), jamais sur le frontend dev server directement.
- **« Cycle-fix loop »** — quand un test casse, l'agent le re-tente après diagnostic + fix automatique du root cause connu.
- **« Latency budget par phase »** — chaque catégorie de test a un budget P99 (signup ≤ 30s, marketplace ≤ 500ms, etc.).

---

## 2. Stack & prérequis

### 2.1 Composants attendus dans le SUT (System Under Test)

| Couche | Exemple FASO | Adapté à votre projet |
|--------|--------------|------------------------|
| Browser | Chromium headless via Playwright | identique |
| Frontend | Angular 21 dev server :4801 | React/Vue/Svelte |
| BFF | Next.js 16 :4800 | Express, Fastify, FastAPI |
| Gateway | ARMAGEDDON Pingora :8080 | Envoy, Kong, Traefik, NGINX+ |
| Authz inline | OPA-core HTTP REST :8181 | OpenFGA, Keto, Cedar |
| WAF inline | Coraza WASM dans gateway | ModSecurity, AWS WAF, Cloudflare |
| IDP | Ory Kratos :4433 | Keycloak, Auth0, FusionAuth |
| Permissions | Ory Keto Zanzibar :4466 | OpenFGA, Spicedb |
| Backend | Spring Boot Java 21 (auth-ms :8801, poulets-api :8901) | NodeJS, Go, .NET, Rails |
| DB | PostgreSQL 17 :5432 | identique |
| Cache/KV | KAYA (Redis-compat) :6380 | Redis, Valkey, DragonflyDB |
| Mail | Mailpit :8025 | MailHog, GreenMail |
| Object store | MinIO :9201 | S3, GCS, Azure Blob |
| Service registry | Consul :8500 | etcd, Eureka |
| Secrets | Vault :8200 + Vault Agent | AWS SSM, GCP Secret Manager |
| Workload identity | SPIRE :8081 (mTLS SVID) | cert-manager, mkcert (dev) |
| **Observabilité** | Jaeger :16686 + Tempo :3200 + Prometheus :9090 + Loki :3100 + Grafana :3000 + OTel Collector :4317/4318 + MinIO archives | identique (stack OTel) |

**Pré-requis machine (dev)** : Docker 24+ ou podman/podman-compose, Bun 1.3+ (ou Node 20+), 16 GB RAM, 30 GB disque libre.

### 2.2 Prérequis réseau

```
Frontend (4801) → BFF (4800) → Gateway (8080) → ext_authz OPA (8181) → backend
                                                                   ↘ JWKS auth-ms (8801) + Kratos (4433)
```

**Important** : le browser ne doit appeler QUE la gateway. Toute requête XHR/fetch directe sur :8801/:8901 invalide l'E2E souverain (cf. §11.3).

---

## 3. Structure de répertoires

```
tests-e2e/
├── playwright.config.ts            # 3 projets : chromium-headless, smoke, chrome-headless-new
├── package.json                    # bun-friendly scripts
├── fixtures/
│   ├── actors.ts                   # 25 → 500 acteurs déterministes (Faker seed=42)
│   ├── data-factory.ts             # randomEmail, randomOffer, randomDemand
│   ├── kratos.ts                   # KratosAdmin (wipeAll, listIdentities)
│   ├── mailpit.ts                  # MailpitClient (waitForOtp, waitForLink)
│   ├── session.ts                  # signupAs, loginAs, logout, ensureAuthenticated
│   ├── totp.ts                     # otplib helper (TotpGen)
│   ├── webauthn.ts                 # CDP virtual authenticator
│   ├── scenarios.ts                # quickSignup, postRandomDemand
│   ├── seed-500.ts                 # générateur 500 actors avec distribution rôles
│   ├── feature-matrix.ts           # role × feature matrix (~80 cells)
│   └── global-setup.ts             # wipe Kratos + Mailpit avant run
├── page-objects/
│   ├── SignupPage.ts               # stepper Material 4 étapes
│   ├── LoginPage.ts
│   ├── DashboardPage.ts
│   ├── ProfilePage.ts
│   ├── MarketplacePage.ts
│   ├── MessagingPage.ts
│   └── SecurityPage.ts             # MFA dialogs (TOTP, PassKey, backup codes)
├── tests/
│   ├── 01-signup/                  # 5 specs par rôle
│   ├── 02-security/                # TOTP, PassKey, backup codes
│   ├── 03-profile/                 # edit
│   ├── 04-business/                # marketplace flow (offer, demand, match, checkout)
│   ├── 05-load/                    # 1000 clients, 5000 transactions
│   ├── 06-payments/                # stub MVP F4-F10
│   ├── 07-navigation/              # 25 routes publiques + protégées
│   ├── 08-auth-flows/              # login, logout, forgot password
│   ├── 09-dashboards/              # dashboards par rôle
│   ├── 10-validation/              # XSS, SQLi, edge cases formulaires
│   ├── 11-api-health/              # 14 endpoints + Prometheus targets
│   ├── 12-latency-tracing/         # Jaeger services + p50/p95/p99 page loads
│   ├── 13-error-pages/             # 404, deep unknown routes, CORS
│   ├── 14-mobile-responsive/       # 375×812, 768×1024, touch targets
│   ├── 15-gateway/                 # routing via gateway, JWKS cache, hop-by-hop strip
│   ├── 16-authz-opa/               # role × route matrix (96 cas)
│   ├── 17-owasp-top10/             # A01-A10 via Coraza WAF
│   ├── 18-functional-matrix/       # role × feature (80 cas)
│   ├── 19-load-p99/                # 100-1000 concurrent users avec extraction P99 Tempo
│   └── 20-chaos/                   # OPA down, auth-ms down, Vault sealed, DB lag
├── scripts/
│   ├── run-simulation.ts           # orchestrateur : run → classify → pause for fix → re-run
│   ├── classify-failures.ts        # parse results.json → suggested-agent + fix hint
│   ├── analyze-timings.ts          # extraction p50/p95/p99 par suite
│   ├── seed-data.ts                # bootstrap 500 records persistants
│   └── p99-by-route.ts             # Tempo TraceQL → table latence par route
└── reports/
    ├── html/                       # Playwright HTML report
    ├── results.json                # JSON pour classify-failures
    ├── junit.xml                   # CI integration
    ├── har/trace.har               # HAR minimal pour debug network
    └── simulation-journal.md       # pass rate par itération
```

---

## 4. `playwright.config.ts` — 3 projets clés

```typescript
import { defineConfig, devices } from '@playwright/test';

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8080';  // ⚠ gateway, pas frontend
const WORKERS = Number(process.env.PW_WORKERS ?? 4);
const CI = !!process.env.CI;

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: CI,
  retries: CI ? 2 : 0,
  workers: WORKERS,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  globalSetup: './fixtures/global-setup.ts',
  reporter: [
    ['list'],
    ['html', { outputFolder: 'reports/html', open: 'never' }],
    ['json', { outputFile: 'reports/results.json' }],
    ['junit', { outputFile: 'reports/junit.xml' }],
  ],
  use: {
    baseURL: BASE_URL,
    headless: true,
    locale: 'fr-BF',                 // adapter à votre locale
    timezoneId: 'Africa/Ouagadougou',// adapter
    viewport: { width: 1440, height: 900 },
    actionTimeout: 10_000,
    navigationTimeout: 30_000,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    contextOptions: {
      recordHar: { path: 'reports/har/trace.har', mode: 'minimal' },
    },
  },
  projects: [
    { name: 'chromium-headless', use: { ...devices['Desktop Chrome'], headless: true } },
    { name: 'chrome-smoke',     use: { ...devices['Desktop Chrome'], channel: 'chrome', headless: true }, grep: /@smoke/ },
    { name: 'chrome-headless-new', use: {
        ...devices['Desktop Chrome'], channel: 'chrome', headless: true,
        launchOptions: { args: ['--headless=new', '--disable-blink-features=AutomationControlled'] },
    }},
  ],
});
```

**Pourquoi 3 projets** :
- `chromium-headless` — par défaut, Chromium bundled (CI portable)
- `chrome-smoke` — Chrome système, `@smoke` tag uniquement, exécution rapide en pre-commit
- `chrome-headless-new` — Chrome récent + flags anti-detection (utile contre WAF qui bloque le UA Chromium par défaut)

---

## 5. Pattern fixtures — actors déterministes scalables

### 5.1 Distribution 500 records

```typescript
// fixtures/seed-500.ts
import { faker, fakerFR } from '@faker-js/faker';

export const SEED_DISTRIBUTION = {
  eleveurs:      200,    // 40 % — coeur de l'offre
  clients:       200,    // 40 % — coeur de la demande
  pharmacies:     30,    //  6 %
  veterinaires:   20,    //  4 %
  aliments:       20,    //  4 %
  transporteurs:  20,    //  4 %
  vaccins:         5,    //  1 %
  admins:          5,    //  1 %
};
// = 500 actors → ~2400 artefacts cascade :
//   600 offers (3 / eleveur), 300 demandes (1.5 / client),
//   300 commandes, 600 notifications, 100 conversations, 50 reviews

export function gen500Actors(): Actor[] {
  fakerFR.seed(42);  // déterministe
  faker.seed(42);
  // ... génère par rôle
}
```

**Pourquoi 500** : avec 25 acteurs (notre baseline initial), P99 reflète le cas heureux. Avec 500, on révèle :
- Dégradations SQL (`LIMIT 25` vs `OFFSET 480 LIMIT 20`)
- Index manquants (signature N+1)
- Cache miss patterns (KAYA hot vs cold)
- Saturation des pools (Hikari, Lettuce)

### 5.2 Bootstrap idempotent

```typescript
// scripts/seed-data.ts
async function seed() {
  await kratos.wipeAll();                            // reset
  await gql.request('mutation { adminTruncateAll }');// reset DB

  for (const batch of chunk(actors, 25)) {
    await Promise.all(batch.map(a => kratos.createIdentity(a)));
  }

  for (const actor of actors) {
    if (actor.role === 'eleveur') await createOffers(actor, 0, 5);
    if (actor.role === 'client')  await createDemands(actor, 0, 3);
  }

  await runMatchingEngine();
  await generateConversations();

  fs.writeFileSync('reports/seed-state.json',
    JSON.stringify({ createdAt: Date.now(), counts: { actors: 500, ... } }));
}
```

Run avant **toute** simulation E2E : `bun run scripts/seed-data.ts --reset`.

---

## 6. Page Object pattern — sélecteurs stables

### 6.1 Règles cardinales

1. **Préférer `formcontrolname`/`data-testid`** aux textes localisés (i18n-proof)
2. **Filtrer `:visible`** sur les boutons stepper (les boutons cachés restent en DOM)
3. **Toujours `expect.poll`** sur les transitions async (RouterLink hydration, animations)
4. **Jamais `waitForLoadState('networkidle')`** seul — couplé avec le reste, devient flaky sous charge

### 6.2 Exemple SignupPage Material stepper

```typescript
export class SignupPage {
  readonly emailInput: Locator;
  readonly nextBtn: Locator;

  constructor(page: Page) {
    this.emailInput = page.locator('input[formcontrolname="email"]');
  }

  async next(): Promise<void> {
    // Filtre :visible isole le bouton de l'étape active (les autres restent en DOM)
    const btn = this.page.locator('button:visible:has-text("Continuer")').first();
    await btn.waitFor({ state: 'visible', timeout: 5_000 });
    await btn.click();
    await this.page.waitForTimeout(450);  // animation Material
  }
}
```

### 6.3 Anti-pattern à éviter

```typescript
// ❌ MAUVAIS : flaky sous workers=2 (race RouterLink hydration)
await login.forgotPasswordLink.click();
await page.waitForLoadState('networkidle');
expect(page.url()).toContain('forgot-password');

// ✅ BON : poll pour absorber le délai d'hydratation
await login.forgotPasswordLink.click();
await expect.poll(() => page.url(), { timeout: 10_000 }).toContain('forgot-password');
```

---

## 7. Test contre la gateway — pattern souverain

### 7.1 baseURL = gateway, pas frontend

```typescript
// playwright.config.ts
const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8080';
```

Le browser charge le SPA via la gateway, qui sert :
- `/` → frontend dev server (proxy)
- `/api/*` → backend (avec JWT validation, OPA authz, WAF)
- `/auth/*` → IDP (Kratos)
- `/admin/*` → admin endpoints (avec authz strict)

### 7.2 Test : aucune fuite directe

```typescript
// tests/15-gateway/no-direct-backend.spec.ts
test('100% E2E traffic through gateway, never backend ports directly', async ({ page }) => {
  const directHits = new Set<string>();
  page.on('request', r => {
    const u = new URL(r.url());
    if ([4801, 4800, 8901, 8801].includes(+u.port)) directHits.add(r.url());
  });

  await page.goto('/');
  await signupAs(page, actor);
  await page.goto('/marketplace');
  await page.waitForLoadState('networkidle');

  expect([...directHits]).toEqual([]);  // hard-fail si fuite
});
```

### 7.3 Test : JWKS caché par la gateway

```typescript
test('JWKS fetched once per 100 requests (cache TTL 10min)', async () => {
  const m0 = await fetch('http://localhost:9902/admin/metrics').then(r => r.text());
  const before = +(m0.match(/armageddon_jwks_fetches_total (\d+)/)?.[1] ?? '0');

  await Promise.all(Array.from({length: 100}, () => fetch('http://localhost:8080/api/health')));

  const m1 = await fetch('http://localhost:9902/admin/metrics').then(r => r.text());
  const after = +(m1.match(/armageddon_jwks_fetches_total (\d+)/)?.[1] ?? '0');

  expect(after - before).toBeLessThanOrEqual(1);
});
```

### 7.4 Test : OPA décide chaque requête /api/*

```typescript
test('OPA receives 1 decision per /api request', async ({ page }) => {
  const opaBefore = await opaDecisionsCount();
  await page.goto('/marketplace');
  await page.waitForLoadState('networkidle');
  const opaAfter = await opaDecisionsCount();
  expect(opaAfter - opaBefore).toBeGreaterThanOrEqual(1);
});
```

---

## 8. Tests de matrice fonctionnelle (role × feature)

```typescript
// fixtures/feature-matrix.ts
export const FEATURE_MATRIX = [
  ['signup',           ['eleveur','client','pharmacie','veterinaire','aliments','transporteur','vaccins']],
  ['marketplace-post', ['eleveur','admin']],
  ['marketplace-demand', ['client']],
  ['halal-certify',    ['veterinaire']],
  ['vaccine-record',   ['veterinaire','vaccins']],
  ['admin-dashboard',  ['admin']],
  // ... 17 features × ~6 roles allowed/denied = ~80 active cells
];

// tests/18-functional-matrix/feature-by-role.spec.ts
for (const [feature, allowedRoles] of FEATURE_MATRIX) {
  for (const role of expandRoles(allowedRoles)) {
    test(`[${feature}] ${role} happy path`, async ({page}) => {
      const actor = pickRandomActor(actors500, role);
      await loginAs(page, actor);
      await runFeatureScenario(page, feature, actor);
    });
  }
}
```

**Bénéfice** : 1 spec génère ~80 tests Playwright via paramétrisation. La couverture passe de "ad-hoc" à "matrice exhaustive trackable dans Grafana".

---

## 9. OWASP Top 10 — couverture explicite

```typescript
// tests/17-owasp-top10/
test('A01: anonymous → /api/admin/* → 401/403', async ({request}) => { ... });
test('A02: PII columns are AES-encrypted at rest', async () => {
  const raw = await pgQuery(`SELECT email FROM auth.users LIMIT 1`);
  expect(raw[0].email).toMatch(/^[A-Za-z0-9+/=]{100,}$/);  // base64 ciphertext
});
test('A03: SQLi via Coraza WAF', async ({request}) => {
  const r = await request.get(`http://localhost:8080/api/annonces?id=' OR 1=1 --`);
  expect(r.status()).toBe(403);
});
test('A03: XSS dans description', async ({request}) => { ... });
test('A03: Command injection dans shell-out', async ({request}) => { ... });
test('A05: GraphiQL disabled in prod profile', async ({request}) => { ... });
test('A07: brute-force lockout après 5 échecs', async ({page}) => { ... });
test('A08: audit_log UPDATE rejected by trigger', async () => { ... });
test('A10: SSRF vers 169.254.169.254 bloqué', async ({request}) => { ... });
test('A10: SSRF vers 127.0.0.1 interne bloqué', async ({request}) => { ... });
```

10/10 catégories couvertes, ~30 tests par projet.

---

## 10. Mesure P50/P95/P99 — méthodologie

### 10.1 Au niveau test (latency simple)

```typescript
test('p99 < 500ms — 100 concurrent users browsing /api/annonces', async ({ browser }) => {
  const N = 100;
  const tx_per_user = 50;
  const latencies: number[] = [];

  await Promise.all(Array.from({ length: N }, async () => {
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    await loginAs(page, pickRandomActor(actors500, 'client'));

    for (let i = 0; i < tx_per_user; i++) {
      const t0 = performance.now();
      await page.evaluate(() =>
        fetch('/api/annonces?page=' + Math.floor(Math.random() * 30))
      );
      latencies.push(performance.now() - t0);
    }
    await ctx.close();
  }));

  latencies.sort((a, b) => a - b);
  const p99 = latencies[Math.floor(latencies.length * 0.99)];
  test.info().annotations.push({ type: 'p99', description: `${p99.toFixed(0)}ms` });
  expect(p99).toBeLessThan(500);
});
```

### 10.2 Au niveau trace (analyse profonde post-run)

```typescript
// scripts/p99-by-route.ts
const services = ['armageddon','poulets-bff','poulets-api','auth-ms','opa'];
for (const svc of services) {
  const traces = await fetch(`http://localhost:3200/api/search?tags=service.name=${svc}&limit=1000`)
    .then(r => r.json());
  const byRoute = groupBy(traces.spans, s => s.attributes['http.route']);
  for (const [route, spans] of Object.entries(byRoute)) {
    const ms = spans.map(s => s.durationMs);
    console.log(`${svc} ${route}: p50=${quantile(ms,0.5)}ms p99=${quantile(ms,0.99)}ms n=${ms.length}`);
  }
}
```

### 10.3 Dashboards Grafana

| Dashboard | Query Prometheus | Panels |
|-----------|-----------------|--------|
| `gateway-p99` | `histogram_quantile(0.99, sum by (le, route) (rate(armageddon_http_duration_seconds_bucket[5m])))` | P50/P95/P99 par route |
| `opa-decisions` | `histogram_quantile(0.99, opa_request_duration_seconds_bucket)` | Allow/Deny ratio + latence |
| `waf-blocks` | `sum by (rule_id) (rate(armageddon_waf_blocks_total[5m]))` | Blocks par règle CRS |
| `error-budget` | `(1 - sum(rate(armageddon_http_requests_total{status=~"5.."}[1h])) / sum(rate(armageddon_http_requests_total[1h]))) > 0.999` | SLO 99.9% |

---

## 11. Cycle-fix loop — automatisation

### 11.1 L'idée

Quand un test échoue, ne pas s'arrêter : classifier l'erreur, dispatcher au "bon agent" (frontend / backend / kaya / devops), appliquer le fix automatique connu, re-lancer le test, jusqu'à GREEN.

### 11.2 `scripts/run-simulation.ts`

```typescript
for (let iter = 1; iter <= maxIter; iter++) {
  // 1. Run Playwright
  execSync(`bunx playwright test ${scope} --reporter=json --output=reports/iter-${iter}/`);

  // 2. Classify failures
  execSync(`bun run scripts/classify-failures.ts reports/iter-${iter}/results.json`);
  const failures = JSON.parse(fs.readFileSync(`reports/iter-${iter}/failures.classified.json`));

  if (failures.length === 0) {
    console.log('✅ GREEN');
    break;
  }

  // 3. Group by suggested agent
  const byAgent = groupBy(failures, f => f.suggestedAgent);
  console.log('Bugs to fix:', Object.entries(byAgent).map(([a,l]) => `${a}: ${l.length}`));

  // 4. Pause: main Claude reads failures.classified.json and dispatches fixes
  await waitForUserKeypress();
}
```

### 11.3 `scripts/classify-failures.ts` — le pattern matching

```typescript
function classify(spec, test, result): ClassifiedFailure {
  const err = result.error?.message ?? '';

  if (/TimeoutError.*locator/i.test(err)) {
    return { errorType: 'selector-missing', suggestedAgent: 'frontend',
             suggestedFix: 'Inspecter le composant Angular, mettre à jour le page-object' };
  }
  if (/waitForOtp.*timeout/i.test(err)) {
    return { errorType: 'otp-not-received', suggestedAgent: 'backend',
             suggestedFix: 'Vérifier Kratos courrier_disable=false + notifier-ms SMTP → mailpit:1025' };
  }
  if (/protocol parse error|expected array frame/i.test(err)) {
    return { errorType: 'kaya-protocol-error', suggestedAgent: 'kaya-rust-implementer',
             suggestedFix: 'Étendre le dispatcher RESP3 dans kaya-protocol/commands' };
  }
  if (/5\d\d|InternalServerError/i.test(err)) {
    return { errorType: 'backend-5xx', suggestedAgent: 'backend',
             suggestedFix: 'Lire /tmp/auth-ms.log ou /tmp/poulets-api.log pour stacktrace' };
  }
  if (/navigation.*timeout|net::ERR_/i.test(err)) {
    return { errorType: 'navigation-timeout', suggestedAgent: 'devops',
             suggestedFix: 'Lancer /status-faso et /stack-up si besoin' };
  }
  return { errorType: 'unknown', suggestedAgent: 'manual-review' };
}
```

### 11.4 Cycle-fix skill (boot loop)

Identique pattern : si auth-ms log montre `Could not resolve placeholder 'AUTH_MS_DB_PASSWORD'`, le skill applique :

```bash
AUTH_MS_DB_PASSWORD=auth_ms_dev_pwd
docker exec faso-postgres psql -U faso -c "ALTER USER auth_ms WITH PASSWORD 'auth_ms_dev_pwd';"
# relaunch
```

Build votre propre table de pattern→fix au fur et à mesure des incidents — chaque incident résolu enrichit l'automatisation.

---

## 12. Observabilité — instrumenter les 4 niveaux

### 12.1 Browser (Angular WebTracerProvider)

```typescript
// src/instrumentation.ts (importé EN PREMIER dans main.ts)
import { WebTracerProvider, BatchSpanProcessor } from '@opentelemetry/sdk-trace-web';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http';
import { ZoneContextManager } from '@opentelemetry/context-zone';
import { FetchInstrumentation } from '@opentelemetry/instrumentation-fetch';

const provider = new WebTracerProvider({
  resource: new Resource({
    [SemanticResourceAttributes.SERVICE_NAME]: 'my-frontend',
  }),
});
provider.addSpanProcessor(new BatchSpanProcessor(new OTLPTraceExporter({
  url: 'http://localhost:4320/v1/traces',
})));
provider.register({ contextManager: new ZoneContextManager() });

registerInstrumentations({
  instrumentations: [
    new FetchInstrumentation({
      propagateTraceHeaderCorsUrls: [/localhost:8080.*/],
    }),
  ],
});
```

⚠ **Pièges CORS** :
- Backend doit autoriser `traceparent, tracestate, baggage` dans `Access-Control-Allow-Headers`
- OTel Collector doit avoir `cors.allowed_origins: ["http://localhost:4801"]` sous `receivers.otlp.protocols.http`

### 12.2 BFF (Next.js instrumentation hook)

```typescript
// instrumentation.ts à la racine du projet Next.js
export async function register() {
  if (process.env.NEXT_RUNTIME !== 'nodejs') return;
  const { NodeSDK } = await import('@opentelemetry/sdk-node');
  const { getNodeAutoInstrumentations } = await import('@opentelemetry/auto-instrumentations-node');
  const sdk = new NodeSDK({
    resource: new Resource({ [SemanticResourceAttributes.SERVICE_NAME]: 'my-bff' }),
    traceExporter: new OTLPTraceExporter({ url: 'http://localhost:4320/v1/traces' }),
    instrumentations: [getNodeAutoInstrumentations({
      '@opentelemetry/instrumentation-fs': { enabled: false },
    })],
  });
  sdk.start();
}
```

### 12.3 Backend Java (Spring Boot Micrometer Tracing)

```yaml
# application.yml
management:
  tracing:
    sampling:
      probability: 1.0     # dev = 100%, prod = 0.1
  otlp:
    tracing:
      endpoint: http://localhost:4320/v1/traces
```

### 12.4 Gateway (ARMAGEDDON / Envoy)

Filter OTel inline qui propage `traceparent` et émet ses propres spans `armageddon`. Le span est racine pour toute la transaction E2E.

### 12.5 Vérification dans les tests

```typescript
test('full-stack trace visible in Jaeger', async () => {
  const r = await fetch('http://localhost:16686/api/services');
  const services = (await r.json()).data;
  expect(services).toEqual(expect.arrayContaining([
    'my-frontend', 'my-bff', 'armageddon', 'auth-ms', 'opa',
  ]));
});
```

---

## 13. CI/CD intégration

### 13.1 GitHub Actions exemple

```yaml
name: E2E Full-Stack
on: [push, pull_request]
jobs:
  e2e:
    runs-on: ubuntu-latest
    services:
      postgres: { image: postgres:17, env: { POSTGRES_PASSWORD: dev }, ports: ['5432:5432'] }
      kaya:     { image: ghcr.io/faso/kaya:latest, ports: ['6380:6380'] }
      kratos:   { image: oryd/kratos:v1.4, ports: ['4433:4433','4434:4434'] }
      mailpit:  { image: axllent/mailpit, ports: ['1025:1025','8025:8025'] }
      jaeger:   { image: jaegertracing/all-in-one, ports: ['16686:16686','4318:4318'] }
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v1
      - run: cd tests-e2e && bun install
      - run: cd tests-e2e && bunx playwright install chromium --with-deps
      - run: bash scripts/start-stack.sh   # gateway, BFF, frontend, backends
      - run: bun run scripts/seed-data.ts --reset
      - run: WIPE_IDENTITIES=true bunx playwright test --workers=2 --retries=2
      - if: failure()
        uses: actions/upload-artifact@v4
        with: { name: e2e-traces, path: tests-e2e/reports/ }
```

### 13.2 Pre-commit hook (smoke uniquement)

```bash
# .git/hooks/pre-commit
cd tests-e2e && bunx playwright test --project=chrome-smoke -g @smoke --reporter=line
```

Tag `@smoke` sur 1 test par feature critique → < 90s d'exécution.

---

## 14. Patterns gotchas — pièges & solutions

| Symptom | Cause | Fix |
|---------|-------|-----|
| `503 Service Unavailable` sur health | Spring Boot agrège tous les indicators (Vault DOWN en dev casse le tout) | Configurer `management.endpoint.health.group.liveness.include=db,redis,ping` |
| `403 Forbidden` sur `/actuator/health/liveness` | Spring Security par défaut sécurise tout | Whitelist explicite : `requestMatchers("/actuator/health/liveness").permitAll()` |
| `net::ERR_FAILED` sur tous les XHR après ajout OTel | Browser envoie `traceparent` mais BFF/CORS ne l'autorise pas | `Access-Control-Allow-Headers: traceparent, tracestate, baggage` |
| `0.0.0.0:8080 is in use` retry-loop sur Pingora | Hyper bind avant Pingora même en runtime=pingora | Patch source : `if !skip_hyper_accept_loop { TcpListener::bind(...) }` |
| `parse error` JWT dans tests Rego | Mock JWT mal-formé bloque `io.jwt.decode` | Forwarder `input.jwt_payload` pré-parsé depuis la gateway, skip la décode dans Rego |
| `Found more than one migration with version 1` | Audit-lib partage V1 avec auth-ms | Renumérer audit migration → V100, et utiliser `IF NOT EXISTS` partout |
| Hibernate `wrong column type INET` | `String` Java mappé sur `INET` PostgreSQL | Soit `@JdbcTypeCode(SqlTypes.INET)`, soit `ALTER COLUMN ... TYPE TEXT` |
| `permission denied for schema audit` | Runtime user n'est pas owner du schema | `ALTER SCHEMA audit OWNER TO <runtime_user>` ou GRANT explicit |
| Cluster healthy mais `no healthy upstream` | Health checker et LB ne partagent pas le state | xDS push obligatoire OU patcher LB pour lire le health checker |
| Tests flaky sous `workers=2` | Race condition Angular RouterLink | Wrap URL assertions avec `expect.poll` |
| `manifest unknown` quay.io/openpolicyagent/opa:0.71.0-rootless | Tag inexistant | Vérifier registry, utiliser tag existant (1.15.2) |
| OPA volume mount ne reload pas | OPA charge les policies au boot uniquement | `curl -X PUT /v1/policies/...` pour push API, ou bundle service |

---

## 15. Lessons learned — résumé condensé

### 15.1 Sur la simulation E2E

1. **L'observabilité doit être en place AVANT les tests** — sinon impossible de debug P99
2. **Seed déterministe** (`faker.seed(42)`) — sinon les bugs intermittents sont impossibles à reproduire
3. **3 projets Playwright** (chromium / chrome / chrome-headless-new) — couvre 80% des cas anti-bot
4. **Wipe IDP entre runs** — sinon les retry échouent sur "user already exists"
5. **HAR minimal** — utile pour debug network sans saturer le disque
6. **Annotations `test.info().annotations.push({ type: 'p99', ... })`** — apparaissent dans le rapport HTML

### 15.2 Sur l'architecture sous test

1. **Gateway = single source of truth pour les filtres** — sinon authz/CORS/WAF dérivent service par service
2. **OPA HTTP REST first, WASM en P2** — debug 10x plus simple pour démarrer
3. **Vault Agent sidecar** — valide la rotation de creds en runtime, pas seulement au boot
4. **MinIO sovereign** — souveraineté + bucket lifecycle ILM (5y/90d/395d) = compliance
5. **audit_log partitionné mensuel** — DROP PARTITION pour la rétention, pas DELETE
6. **BlindIndexConverter HMAC** — recherche sur PII chiffrée sans casser l'index B-tree

### 15.3 Sur le cycle de vie du projet

1. **Cycle-fix loop > test runs ponctuels** — automatise le diagnostic + fix des erreurs connues
2. **Classify failures + agent dispatch** — chaque erreur a un agent responsable (frontend, backend, kaya, devops)
3. **Pass rate ≥ 95%** comme gate de merge — sinon la suite E2E devient cosmétique
4. **Latency budgets par phase** — un test qui prend 30s sur signup est OK, 30s sur browse marketplace est un bug
5. **`test.fixme()` pour features non-implémentées** — pas de skip silent, pas de TODO oublié

---

## 16. Adaptation à votre projet — checklist finale

- [ ] Copier `tests-e2e/` skeleton (config, fixtures, page-objects, scripts)
- [ ] Adapter `fixtures/actors.ts` (rôles, contraintes locale, format pays)
- [ ] Configurer `BASE_URL=http://localhost:8080` (votre gateway)
- [ ] Câbler OTel SDK : browser + BFF + backend + gateway
- [ ] Vérifier CORS preflight allow `traceparent, tracestate, baggage` partout
- [ ] Implémenter `global-setup.ts` pour wiper l'IDP
- [ ] Démarrer la stack via `cycle-fix` ou `docker compose up`
- [ ] Run `bun run scripts/seed-data.ts --reset` (votre dataset de référence)
- [ ] Run `bunx playwright test --project=chromium-headless`
- [ ] Vérifier dans Jaeger que `service.name` du browser apparaît
- [ ] Construire votre `classify-failures.ts` au fil des incidents
- [ ] Construire vos dashboards Grafana P99
- [ ] Définir vos latency budgets par suite (signup 30s, browse 500ms, ...)
- [ ] Intégrer en CI avec `--retries=2` + upload des artefacts en cas d'échec

---

## 17. Sources & références

- [Playwright Test Generator](https://playwright.dev/docs/codegen) — pour bootstrap rapide
- [OpenTelemetry Browser Instrumentation](https://opentelemetry.io/docs/instrumentation/js/automatic/)
- [OWASP Core Rule Set v4](https://github.com/coreruleset/coreruleset)
- [OPA Rego Playground](https://play.openpolicyagent.org/) — pour itérer sur les policies
- [Tempo TraceQL](https://grafana.com/docs/tempo/latest/traceql/) — extraction P99 par span attribute
- [Pingora Open Source](https://github.com/cloudflare/pingora) — base d'ARMAGEDDON
- [Coraza Proxy WASM](https://github.com/corazawaf/coraza-proxy-wasm) — WAF souverain en WASM

---

*Doc maintenue par l'équipe FASO DIGITALISATION. Version : 2026-04-28. Licence : AGPL-3.0-or-later.*
