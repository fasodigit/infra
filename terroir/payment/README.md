<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# terroir-payment

Service paiements producteurs (Java Spring Boot, port `8832`, actuator
loopback `9004`).

**Statut** : placeholder P0.A. **Implémentation Phase 4.b TERROIR P2**
(cf. `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §7 P2.1 + P2.2)
— **réutilise `INFRA/shared/mobile-money-lib/`**.

## Périmètre P2

- Endpoints REST :
  - `POST /payments` (idempotent KAYA `terroir:idempotent:payment:{orderId}`).
  - `GET /payments/{id}`.
  - `POST /payments/{id}/confirm` (callback provider Mobile Money).
- Reconciliation : nightly batch + real-time CDC Redpanda.
- Topics produits : `terroir.payment.initiated/completed/failed`.
- Notifier-ms consume → SMS/USSD via simulator (P2) ou providers réels (P3+).

## Stack cible

- Java 21 (eclipse-temurin), Spring Boot 3.4.
- Maven module dépendant de `INFRA/shared/mobile-money-lib/` (extraction P2.1).
- Audit-lib injecté pour `audit_t_<slug>.audit_log`.
- Spring Cloud Vault pour secrets MoMo providers.

## Pourquoi Java (et pas Rust)

- Réutilisation directe de `mobile-money-lib` partagée avec `poulets-api`
  (Q9 ULTRAPLAN).
- Maturité de Spring Boot Actuator pour la métrologie/observabilité
  (Prometheus + Tempo).

## Référentiel

- Inventaire MoMo existant : `INFRA/shared/mobile-money-lib/README.md`.
- Idempotency pattern : `INFRA/CLAUDE.md` §3 (KAYA), §10 (cycle-fix).
