<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# `mobile-money-lib` — inventaire et plan d'extraction

**Statut** : design P0 (livrable P0.7 du module TERROIR — cf.
`INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §4 P0.7).
**Refactor effectif** : Phase TERROIR P2 (P2.1).
**Pas de code Java ici en P0** — uniquement inventaire + design.

## 1. Périmètre

Bibliothèque Java partagée entre :

- `INFRA/poulets-platform/backend` (poulets-api) — paiement éleveur ↔ client.
- `INFRA/terroir/payment` (Spring Boot, P2) — paiement coopérative ↔
  producteur (mobile money).

Providers Mobile Money cibles (Burkina Faso M1, ouest-africain M5+) :

| Provider | Code | Statut M1 | Note |
|----------|------|-----------|------|
| Orange Money BF | `orange_money` | actif | leader marché BF |
| Moov Africa | `moov_africa` | actif | ex-Telecel |
| Wave | `wave` | actif | low-fee, populaire |
| MTN MoMo | `mtn_momo` | M5+ (CI/SN/GH) | hors BF |

## 2. Inventaire actuel — `poulets-platform`

### 2.1 Backend Java — Temporal stub

L'intégration Mobile Money **n'existe pas encore en Java natif** dans
`poulets-platform/backend/`. La seule trace est un *placeholder* dans le
moteur de workflows Temporal :

| Fichier | Méthode | Rôle |
|---|---|---|
| `INFRA/poulets-platform/backend/workflow-engine/src/main/java/bf/gov/faso/workflow/activities/PouletsActivities.java` | `String chargePayment(String orderId, String paymentMethod, long amountFcfa)` | Activity Temporal — declared, **pas d'implémentation provider concrète**. |
| `INFRA/poulets-platform/backend/workflow-engine/src/main/java/bf/gov/faso/workflow/impl/OrderWorkflowImpl.java` | `OrderWorkflowImpl#processOrder` (l. 75) | Appel `chargePayment(input.orderId(), input.paymentMethod(), input.amountFcfa())`. |
| `INFRA/poulets-platform/backend/workflow-engine/src/main/java/bf/gov/faso/workflow/workflows/OrderWorkflow.java` | `record OrderInput(... String paymentMethod ...)` | Le `paymentMethod` est passé en string libre — pas de typage. |
| `INFRA/poulets-platform/backend/workflow-engine/src/main/java/bf/gov/faso/workflow/workflows/DisputeSaga.java` | `refundPayment(...)` | Appelle `activities.refundPayment(chargeId, amountFcfa)` — stub. |

→ **Constat** : pas de classe `*MobileMoney*`, `*OrangeMoney*`, `*Wave*`,
`*Moov*`, `*MTN*` en Java actuellement. Le mot-clé "payment" apparaît
uniquement dans les workflows Temporal et templates Handlebars de contrats.

### 2.2 BFF Next.js — implémentation de référence

L'intégration **réelle** vit aujourd'hui en TypeScript côté BFF :

**`INFRA/poulets-platform/bff/src/app/api/payments/mobile-money/route.ts`** :

```typescript
const SUPPORTED_PROVIDERS = new Set(['orange_money', 'moov_africa', 'wave']);
const MOMO_GATEWAY_URL = process.env['MOMO_GATEWAY_URL'];
const BF_PHONE_REGEX = /^\+?226\d{8}$|^\d{8}$/;

export async function POST(request: NextRequest) { /* ... */ }
```

**Comportements clés observés** :

- Authentification via headers `X-User-Id` + `X-Tenant-Id` injectés par
  `middleware.ts` (Kratos session validée).
- Validation provider stricte (whitelist).
- Validation MSISDN BF (`+226XXXXXXXX` ou 8 chiffres).
- `reference` **dérivée serveur** : `order-${tenantId}-${userId}-${txSuffix}`
  (anti-hijack — fix HIGH 2026-04-20).
- `txId` aléatoire : `momo-${Date.now()}-${random}`.
- Mode dégradé : si `MOMO_GATEWAY_URL` absent → réponse stub `PENDING`
  locale. Si gateway timeout → idem stub avec `status: 202`.
- Pas d'idempotency-key client visible.
- Pas de webhook callback persisté (stub).
- Pas de KAYA / Vault dans le path actuel — secrets côté gateway externe.

**`INFRA/poulets-platform/frontend/src/app/features/payments/mobile-money.service.ts`** (Angular client) :

```typescript
export type MobileMoneyProvider = 'orange_money' | 'moov_africa' | 'wave';
export interface MobileMoneyInitiateRequest {
  provider: MobileMoneyProvider; phone: string; amount: number; reference: string;
}
initiate(req): Observable<MobileMoneyInitiateResponse>;
```

→ Service Angular reactive qui POST `/api/payments/mobile-money`. Fallback
local-stub côté frontend si erreur HTTP/réseau.

### 2.3 Pattern actuel — synthèse

| Aspect | Implémentation actuelle |
|--------|------------------------|
| Transport | HTTP REST (Next.js BFF → upstream `MOMO_GATEWAY_URL`) |
| Client HTTP | `fetch` natif Node.js (Next.js Edge runtime) |
| Auth provider | Délégué à un gateway externe (`MOMO_GATEWAY_URL`) — credentials non gérés dans le repo |
| OAuth/API key | Pas implémenté côté plateforme — gateway externe |
| Idempotency | **Manquant** (gap critique pour P2) |
| Webhook | **Non géré** (réponse upstream relayée 1-shot) |
| Reconciliation | **Manquant** |
| SDK officiel | Aucun (Orange/Wave/Moov n'exposent pas de SDK Java mature) |
| Persistance | Aucune côté plateforme |
| Tracing | Headers `X-User-Id` / `X-Tenant-Id` propagés |

→ **Conclusion P0** : il n'y a **rien à extraire en Java** ; il y a un
**design net à porter** depuis le BFF TypeScript vers une lib Java native
réutilisable, avec ajout des capacités manquantes (idempotency, webhook,
reconciliation, secrets Vault).

## 3. API cible (proposée P2)

### 3.1 Interface principale

```java
package bf.gov.faso.shared.mobilemoney;

public interface MobileMoneyClient {

    /**
     * Initie un paiement Mobile Money. Idempotent : un même
     * idempotencyKey est garanti d'effectuer **une seule** charge réelle
     * (réplay protégé via KAYA `terroir:idempotent:payment:{key}` TTL 24h).
     *
     * @param provider      Provider ciblé (Orange, Wave, Moov, MTN)
     * @param msisdn        MSISDN E.164 (+226XXXXXXXX) — validé strict
     * @param amountFcfa    Montant en FCFA (entier, > 0)
     * @param idempotencyKey Clé fournie par l'appelant (workflowId+step recommandé)
     * @param callbackUrl   URL HTTPS publique pour notification asynchrone
     *                      (`POST {callbackUrl}` avec body MoMoWebhookEvent)
     * @return PaymentInitiation contenant txId, status initial, pollUrl
     * @throws MobileMoneyException si provider down / validation KO
     */
    PaymentInitiation requestPayment(
        Provider provider,
        Msisdn msisdn,
        long amountFcfa,
        IdempotencyKey idempotencyKey,
        URI callbackUrl
    ) throws MobileMoneyException;

    /** Récupère l'état d'une transaction (poll-mode, complément du webhook). */
    PaymentStatus getStatus(TxId txId);

    /** Remboursement total ou partiel (refund). */
    RefundResult refund(TxId txId, long amountFcfa, IdempotencyKey idempotencyKey);
}
```

### 3.2 Types support

```java
public enum Provider { ORANGE_MONEY, MOOV_AFRICA, WAVE, MTN_MOMO }
public record Msisdn(String e164) { /* validation +226|+225|+221|... */ }
public record TxId(String value) {}
public record IdempotencyKey(String value) {}
public record PaymentInitiation(TxId txId, PaymentPhase phase, URI pollUrl) {}
public enum PaymentPhase { PENDING, SUCCESS, FAILED, CANCELED, REFUNDED }
public record PaymentStatus(TxId txId, PaymentPhase phase, Optional<String> failureCode);
public record MoMoWebhookEvent(TxId txId, PaymentPhase phase, Instant occurredAt, String signature);
```

### 3.3 Provider adapter pattern

```java
interface ProviderAdapter {
    Provider id();
    PaymentInitiation initiate(/* ... */) throws ProviderException;
    PaymentStatus pollStatus(TxId id) throws ProviderException;
    void verifyWebhookSignature(MoMoWebhookEvent ev) throws ProviderException;
    RefundResult refund(/* ... */) throws ProviderException;
}

@Component class OrangeMoneyAdapter implements ProviderAdapter { /* OAuth2 + REST */ }
@Component class WaveAdapter         implements ProviderAdapter { /* HMAC + REST */ }
@Component class MoovAfricaAdapter   implements ProviderAdapter { /* OAuth2 + REST */ }
@Component class MtnMomoAdapter      implements ProviderAdapter { /* OAuth2 + REST */ }
```

### 3.4 Secrets Vault

Convention de path Vault (cf. `INFRA/CLAUDE.md` §2) :

```
faso/mobile-money/orange_money/{api_key, merchant_id, oauth_secret, webhook_hmac}
faso/mobile-money/wave/{api_key, merchant_id, webhook_hmac}
faso/mobile-money/moov_africa/{api_key, merchant_id, oauth_secret, webhook_hmac}
faso/mobile-money/mtn_momo/{api_key, merchant_id, oauth_secret, webhook_hmac, country=BF|CI|SN}
```

Injection via Spring Cloud Vault (déjà en place pour `auth-ms` et
`poulets-api`).

### 3.5 Idempotency

- Clé KAYA : `terroir:idempotent:payment:{idempotencyKey}` TTL 24h.
- Si la clé existe → renvoyer la `PaymentInitiation` mémorisée.
- Si la clé absente → poser le lock atomique (KAYA `SET NX EX 86400`) ;
  effectuer l'appel provider ; persister le résultat.

### 3.6 Reconciliation

- CDC Redpanda : topic `terroir.payment.completed` (déjà prévu en P0.E).
- Batch nightly : appel `getStatus(txId)` pour toutes les transactions
  `PENDING` > 1h.
- Si écart provider ↔ DB local → publier `terroir.payment.reconciliation.discrepancy`.

## 4. Plan d'extraction (Phase P2.1)

| Étape | Description | Sortie |
|---|---|---|
| **E1** | Créer module Maven `INFRA/shared/mobile-money-lib/` (`pom.xml` aggregator parent `INFRA/shared/pom.xml`). | Squelette compilable. |
| **E2** | Définir `MobileMoneyClient` + types support + exception hierarchy. | API publique stable. |
| **E3** | Implémenter `ProviderAdapter` Orange Money (BF) en premier. | 1 provider complet end-to-end. |
| **E4** | Tests intégration : sandbox Orange Money → assertion phase `SUCCESS`. | JUnit + Testcontainers. |
| **E5** | Ajouter Wave + Moov + MTN. | 4 providers actifs. |
| **E6** | Brancher `terroir-payment` (P2.2) dessus — endpoints `POST /payments`. | Premier consommateur. |
| **E7** | Migrer `poulets-platform/backend/workflow-engine/PouletsActivities#chargePayment` vers `MobileMoneyClient`. | Deuxième consommateur, suppression duplication. |
| **E8** | Retirer la logique BFF Next.js (`route.ts`) → délégation `terroir-payment`. | Centralisation. |
| **E9** | Tests Playwright `tests-e2e/19-terroir/terroir-payment-mobile-money-idempotent.spec.ts` (cf. ULTRAPLAN P2.5). | E2E coverage. |

## 5. Anti-patterns à éviter

- ❌ Hardcoder une URL provider dans le code — toujours via Spring Cloud
  Vault.
- ❌ Stocker un `api_key` en clair dans `application.yml`.
- ❌ Réutiliser un `idempotencyKey` au-delà de 24h (collision possible).
- ❌ Ignorer la signature HMAC d'un webhook (replay attaques mobile money
  documentées en BF en 2025).
- ❌ Charger la lib en runtime classpath des deux services sans isolation
  de version (utiliser `<dependencyManagement>` parent BOM).

## 6. Références

- ULTRAPLAN TERROIR : `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md`
  §4 P0.7, §7 P2.1-P2.2, §12.
- BFF actuel : `INFRA/poulets-platform/bff/src/app/api/payments/mobile-money/route.ts`.
- Frontend client : `INFRA/poulets-platform/frontend/src/app/features/payments/mobile-money.service.ts`.
- Vault paths : `INFRA/CLAUDE.md` §2.
- Souveraineté KAYA : `INFRA/CLAUDE.md` §3.
- Port-policy `terroir-payment: 8832` : `INFRA/port-policy.yaml`.
