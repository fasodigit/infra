<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# ADR-003 — Choix du / des fournisseurs USSD et SMS

| Champ | Valeur |
|---|---|
| Statut | Proposé |
| Date | 2026-04-30 |
| Décideurs | Tech lead, BizDev, juridique |
| Contexte | TERROIR — canal producteur sans smartphone (~70% milieu rural BF) |

## Contexte

Une part majeure des producteurs n'a pas de smartphone (~70% en milieu rural Burkina Faso, ~50% Côte d'Ivoire). Pour leur permettre de :
- Consulter leur solde de paiements
- Recevoir notification d'un paiement reçu
- Consulter leur dernière livraison
- Voter en assemblée générale virtuelle (P3)

… il faut un canal **USSD** (sessions courtes 5-10 menus, code court genre `*144#`) et **SMS** (notifications push, OTP).

### Contraintes
- Couverture multi-pays : BF, CI, SN, ML, BJ, TG, NE, GH (à terme)
- Multi-opérateurs : Orange, Moov, Telecel, MTN, Airtel, Free, Wave (Wave SN/CI fait déjà SMS)
- Latence USSD critique (sessions auto-terminées par opérateur après 30-60s)
- Coût par session/SMS variable (0.5 à 5 FCFA)
- Souveraineté : éviter à terme dépendance providers US (Twilio)

## Options envisagées

### Option A — Fournisseur unique (Twilio)
**Pour** : doc claire, SDK qualité.
**Contre** : couverture USSD Afrique de l'Ouest limitée (Twilio fait surtout SMS), $$$ , dépendance US.

### Option B — Fournisseur unique africain (Africa's Talking)
**Pour** : excellente couverture USSD CI/KE/NG/SN, pricing local.
**Contre** : couverture Burkina Faso historiquement faible (à reconfirmer), single-point-of-failure.

### Option C — Fournisseur unique régional (Hub2)
**Pour** : très bonne couverture francophone (BF/CI/SN/ML), accords cadres avec Orange UEMOA.
**Contre** : moins mature USSD vs Africa's Talking, écosystème dev plus petit.

### Option D — Multi-provider abstrait avec routage par pays/opérateur
**Pour** : redondance, optimisation coût, pas de single-point-of-failure, négociation par fournisseur.
**Contre** : complexité (1 provider vs 3), overhead intégration.

### Option E — Direct opérateur (gateway USSD propre)
**Pour** : coût marginal le plus bas à très grand volume.
**Contre** : exige agrément ARCEP local, contrat opérateur direct (6-12 mois négociation), équipe télécom dédiée. Hors scope MVP.

## Décision

**Option D — Multi-provider abstrait, avec routage par pays/opérateur.**

### Architecture

```
┌──────────────────────────────────────────────────┐
│ terroir-ussd (Rust, Axum) — port 8834            │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ UssdRouter                                   │ │
│ │  - lookup(country, msisdn) → provider        │ │
│ │  - fallback chain (primary → secondary)      │ │
│ └────────────┬───────────────┬─────────────────┘ │
│              ▼               ▼                   │
│      ┌──────────────┐ ┌──────────────┐           │
│      │ AfricaTalking│ │ Hub2         │  + mock   │
│      │ adapter      │ │ adapter      │  (tests)  │
│      └──────────────┘ └──────────────┘           │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ Menu DSL (state machine YAML)                │ │
│ │  -> rendu localisé (FR/Mooré/Dioula/...)     │ │
│ └──────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

### Routage par défaut (proposition initiale, à confirmer P0)
| Pays | USSD primary | USSD fallback | SMS primary | SMS fallback |
|---|---|---|---|---|
| Burkina Faso | Hub2 | Africa's Talking | Hub2 | Twilio |
| Côte d'Ivoire | Africa's Talking | Hub2 | Africa's Talking | Hub2 |
| Sénégal | Africa's Talking | Hub2 | Africa's Talking | Twilio |
| Mali | Hub2 | Africa's Talking | Hub2 | Twilio |
| Bénin / Togo | Hub2 | Africa's Talking | Hub2 | Twilio |

Twilio gardé en SMS-only fallback (urgences OTP) tant qu'on n'a pas dérisqué les providers africains.

### Code court USSD partagé
- BF : tenter `*144*FASO#` (à négocier ARCEP) ; sinon code court attribué par provider
- CI : code court attribué par provider
- Stratégie initiale : codes provider, migration vers code court FASO partagé en P3

## Conséquences

### Positives
- Redondance : si un provider tombe, l'autre prend le relais (auto-failover après 3 échecs)
- Négociation : dev ARPU permet de jouer sur les coûts
- Souveraineté incrémentale : Twilio peut être retiré dès dérisquage
- Couverture maximale dès J0

### Négatives
- Coût intégration initial : ~3-4 semaines dev (3 adapters)
- Tests E2E : nécessite mocks fidèles (provider sandbox + replay)
- Réconciliation facturation : 3 factures vs 1

### Mitigations
- Adapter trait commun (`UssdProvider`) → facile à étendre
- Sandbox / mode dry-run en local
- Réconciliation via API factures (Africa's Talking, Hub2 exposent CDR JSON)

## Sécurité

- Aucun secret en config : tout en Vault `faso/terroir/ussd/{provider}/{key}`
- Rotation tokens API providers tous les 90j
- Validation IP whitelisting webhooks (chaque provider expose pool IPs)
- Signature HMAC pour les callbacks entrants (rejeter si mismatch)
- Rate limiting par MSISDN (5 sessions / minute / numéro)

## Coût estimé an 1 (hypothèse 50k producteurs actifs)

- USSD : 5 sessions/mois/producteur × 50k × 12 = 3M sessions × 1.5 FCFA ≈ **4.5M FCFA / an** (~7 k€)
- SMS : 10 SMS/mois/producteur × 50k × 12 = 6M SMS × 25 FCFA ≈ **150M FCFA / an** (~230 k€) → **gros poste**, à refacturer dans le pricing exportateur

## Métriques de succès

- USSD session success rate ≥ 95% (toutes raisons confondues)
- USSD latency p95 ≤ 2 s (par interaction utilisateur)
- SMS delivery rate ≥ 98%
- 0 secret en config statique (audit Vault mensuel)

## Révision

À reconfirmer après pilote P1. Si Hub2 sous-performe en BF, basculer sur Africa's Talking ou évaluer Option E (direct opérateur) pour P4 si volume > 200k producteurs.
