# BACKLOG-ROADMAP — Vision 6 mois, 12 sprints (Plateforme Poulets)

> Version : 1.0
> Date : 2026-04-18
> Horizon : 6 mois calendaires (2026-05 → 2026-10)
> Cadence : sprints de 2 semaines, 12 sprints au total
> Team : 4 FTE (2 frontend Angular 21, 1 backend Spring Boot Java 21, 1 QA Playwright)
> Vélocité cible : ~40 dev-jours utiles par sprint (15% overhead retranché)

## Résumé exécutif

Ce roadmap synchronise les 33 epics de `docs/BACKLOG-EPICS.md` en une séquence exécutable. L'objectif principal du pilote 6 mois est de livrer :

- **100% des 10 epics P0** (Sprints 1 à 6)
- **8/13 epics P1 haute valeur métier** (Sprints 7 à 10)
- **2-3 epics P1/P2 growth** (Sprints 11 à 12)
- Les 7 P1 restants + 10 P2 sont reportés en v1.2+ post-pilote.

Budget dev total sur le pilote : ~500 dev-jours. Avec 4 FTE et vélocité 40 dj/sprint = 480 dev-jours — cohérent avec 15% de buffer.

---

## 1. Plan sprints — vue haut niveau

| Sprint | Fenêtre            | Thème                        | Epics inclus                                 | Effort (dj) |
|--------|--------------------|------------------------------|-----------------------------------------------|-------------|
| S1     | 2026-05-04 → 05-15 | Foundation auth+paiement     | EPIC-02, EPIC-01 (demarrage)                  | 35          |
| S2     | 2026-05-18 → 05-29 | Paiement + i18n              | EPIC-01 (fin), EPIC-03 (demarrage)            | 38          |
| S3     | 2026-06-01 → 06-12 | PWA + i18n                   | EPIC-03 (fin), EPIC-05                        | 40          |
| S4     | 2026-06-15 → 06-26 | KYC + Escrow                 | EPIC-06, EPIC-07 (demarrage)                  | 42          |
| S5     | 2026-06-29 → 07-10 | Escrow + Notifs              | EPIC-07 (fin), EPIC-08                        | 38          |
| S6     | 2026-07-13 → 07-24 | Chat + Geoloc + Analytics    | EPIC-04, EPIC-09, EPIC-10                     | 44          |
| S7     | 2026-07-27 → 08-07 | Vertical metier (1/3)        | EPIC-15 (vaccins), EPIC-13 (calendrier)       | 40          |
| S8     | 2026-08-10 → 08-21 | Vertical metier (2/3)        | EPIC-17 (traçabilité), EPIC-14 (sanitaire)    | 38          |
| S9     | 2026-08-24 → 09-04 | Vertical metier (3/3) + Halal| EPIC-16 (halal), EPIC-11 (liste noire)        | 30          |
| S10    | 2026-09-07 → 09-18 | Admin + Signature + Chat+    | EPIC-20, EPIC-18, EPIC-19                     | 42          |
| S11    | 2026-09-21 → 10-02 | Growth + SLA                 | EPIC-21, EPIC-22, EPIC-23                     | 33          |
| S12    | 2026-10-05 → 10-16 | Assurance + Dark + Buffer    | EPIC-12, EPIC-33, buffer incidents            | 30          |
|        |                    |                              | **Total pilote**                              | **450**     |

Reste hors pilote (v1.2+) : EPIC-24, 25, 26, 27, 28, 29, 30, 31, 32 = 9 epics ~ 170 dev-jours.

---

## 2. Détail sprint par sprint

### Sprint 1 (S1) — Foundation auth + paiement (demarrage)

**Objectif** : débloquer authentification SMS + démarrer intégration Mobile Money pour pouvoir tester de bout en bout fin S2.

- **EPIC-02 (SMS OTP)** — 2 semaines — livré fin S1
- **EPIC-01 (Mobile Money)** — 3 semaines — démarrage, sandbox Orange Money + Moov

**Ressources** :
- Backend (1 FTE) : EPIC-02 intégration Twilio/Orange SMS, EPIC-01 sandbox OM
- Frontend (2 FTE) : pages login/register OTP, flow paiement squelette
- QA (1 FTE) : Playwright login OTP, mocks Mobile Money

**Dépendances sortantes** : débloque EPIC-06, EPIC-07, EPIC-10 et tous epics paiement aval.

**Risques S1** : Twilio approbation numéro BF (~5 jours), activation sandbox OM (variable, 3-10 jours).

---

### Sprint 2 (S2) — Paiement + i18n (demarrage)

**Objectif** : MVP paiement fonctionnel (3 opérateurs), commencer i18n FR+3 langues.

- **EPIC-01 (Mobile Money)** — finalisation Wave + Moov + webhook
- **EPIC-03 (i18n)** — démarrage extraction chaînes + setup ICU

**Risques S2** : webhooks signature HMAC à durcir, traduction Mooré demande locuteurs natifs identifiés en S1.

---

### Sprint 3 (S3) — PWA + i18n (fin)

**Objectif** : app utilisable en rural (offline-first) + multilangue complet.

- **EPIC-03 (i18n)** — finalisation 4 locales, SMS + emails traduits
- **EPIC-05 (PWA offline)** — service worker, IndexedDB, queue sync

**Risques S3** : quotas IndexedDB varient selon browser/version, Service Worker iOS Safari instable.

---

### Sprint 4 (S4) — KYC + Escrow (demarrage)

**Objectif** : Permettre vérification identité et démarrer séquestre paiement.

- **EPIC-06 (KYC biométrique)** — 3 semaines, upload + OCR + liveness + workflow validation
- **EPIC-07 (Escrow)** — démarrage modèle state machine + API

**Risques S4** : coût OCR cloud, PII à chiffrer AES-256, CNIB anciens formats.

---

### Sprint 5 (S5) — Escrow + Notifs

**Objectif** : Escrow en production + notifications push pour réengagement.

- **EPIC-07 (Escrow)** — finalisation + procédure dispute + médiation
- **EPIC-08 (Push PWA)** — VAPID + opt-in + templates 4 langues

**Risques S5** : complexité comptable OHADA escrow, iOS push limité.

---

### Sprint 6 (S6) — Chat + Geoloc + Analytics (trio P0)

**Objectif** : Finaliser les 10 P0 avec triade UX essentielle.

- **EPIC-04 (Chat temps réel)** — WebSocket STOMP, persistance, présence
- **EPIC-09 (Géoloc)** — PostGIS, Leaflet OSM, recherche par rayon
- **EPIC-10 (Vendor analytics)** — dashboard 8 KPIs, materialized views

Sprint chargé (44 dj), 3 epics en parallèle. Risque charge, à surveiller.

**Fin S6 = MVP P0 complet**. Pilote ouvrable à cercle fermé (100 éleveurs + 500 clients beta).

---

### Sprint 7 (S7) — Vertical métier (1/3) : vaccins + calendrier

**Objectif** : Transformer plateforme e-commerce générique en outil métier avicole.

- **EPIC-15 (Marketplace vaccins ordonnance)** — 3 semaines, workflow vétérinaire
- **EPIC-13 (Calendrier cycles)** — templates chair 42j + ponte 18 semaines

**Risques S7** : réglementation DGSV vaccins stricte, chaîne froid logistique = engagement partenaires grossistes.

---

### Sprint 8 (S8) — Vertical métier (2/3) : traçabilité + sanitaire

**Objectif** : Confiance urbaine (QR lot) + obligation CILSS.

- **EPIC-17 (Traçabilité lot)** — 3 semaines, QR + timeline append-only
- **EPIC-14 (Alerte sanitaire aviaire)** — 1 semaine, ingestion flux CILSS/MinEleve

**Risques S8** : éleveurs réticents à saisie détaillée traçabilité (former via support), flux ministère irrégulier.

---

### Sprint 9 (S9) — Vertical métier (3/3) : halal + liste noire

**Objectif** : Segment premium urbain (halal) + durcissement anti-fraude.

- **EPIC-16 (Certificat halal)** — 2 semaines, badge + filtre catalogue
- **EPIC-11 (Liste noire partagée)** — 1 semaine, 4 vecteurs blocage

Sprint léger (30 dj), permet résorber dette technique accumulée S4-S8.

---

### Sprint 10 (S10) — Admin + Signature + Chat enrichi

**Objectif** : Équipement équipe ops + B2B contrat + UX chat complète.

- **EPIC-20 (Admin panel)** — 3 semaines, 6 modules (KYC queue, disputes, etc.)
- **EPIC-18 (Signature électronique)** — 2 semaines, OTP SMS + PDF scellé
- **EPIC-19 (Chat pièces jointes)** — 1 semaine, images + PDF + audio

Sprint très chargé (42 dj) à la limite. Prévoir buffer S12.

---

### Sprint 11 (S11) — Growth + SLA

**Objectif** : Capacité d'apprentissage continue (A/B) + auto-régulation qualité.

- **EPIC-21 (A/B testing GrowthBook)** — 1 semaine, self-hosted
- **EPIC-22 (SLA vendor monitoring)** — 1 semaine, score + badge
- **EPIC-23 (Wishlist favoris)** — 3 jours

---

### Sprint 12 (S12) — Assurance + Dark + Buffer

**Objectif** : Finir proprement le pilote + buffer incidents.

- **EPIC-12 (Assurance transaction)** — 2 semaines, si partenaire assureur signé
- **EPIC-33 (Dark mode)** — 3 jours
- **Buffer** : 7-10 dj pour incidents, polish, documentation finale

Si partenariat assureur pas finalisé, EPIC-12 glisse en v1.2 et remplacement par hot-fixes + consolidation.

---

## 3. Dépendances inter-epics — DAG textuel

Le DAG ci-dessous liste les arêtes (A -> B signifie "A doit être livré avant que B démarre").

```
EPIC-02 (SMS OTP)
  ├── EPIC-01 (Mobile Money)           # OTP confirmation paiement
  ├── EPIC-06 (KYC)                    # validation numero
  ├── EPIC-18 (Signature electronique)
  └── EPIC-30 (Delegation compte)      # invitation SMS

EPIC-01 (Mobile Money)
  ├── EPIC-07 (Escrow)                 # sequestre
  ├── EPIC-10 (Vendor analytics)       # CA base paiement
  ├── EPIC-12 (Assurance)              # paiement prime
  ├── EPIC-15 (Vaccins marketplace)    # paiement
  ├── EPIC-22 (SLA)                    # taux annulation via paiement
  ├── EPIC-24 (Abonnements)            # pay on file
  ├── EPIC-27 (Factures)               # donnees paiement
  └── EPIC-31 (Parrainage)             # credit bonus

EPIC-03 (i18n)
  └── [transverse, tous les epics UI]

EPIC-05 (PWA offline)
  ├── EPIC-04 (Chat temps reel)        # queue offline
  ├── EPIC-08 (Push PWA)               # Service Worker base
  ├── EPIC-10 (Vendor analytics)       # dashboard offline
  ├── EPIC-09 (Geoloc)                 # tuiles cache
  └── EPIC-28 (Tracking livraison)     # WS resilience

EPIC-06 (KYC)
  ├── EPIC-07 (Escrow)                 # prerequis confiance
  ├── EPIC-11 (Liste noire)            # CNIB
  ├── EPIC-12 (Assurance)
  ├── EPIC-15 (Vaccins)                # KYC veterinaire
  ├── EPIC-16 (Halal)
  ├── EPIC-18 (Signature)
  ├── EPIC-22 (SLA)
  ├── EPIC-27 (Factures)               # mentions legales
  └── EPIC-30 (Delegation)

EPIC-07 (Escrow)
  ├── EPIC-12 (Assurance)
  └── EPIC-24 (Abonnements)

EPIC-08 (Push PWA)
  ├── EPIC-13 (Calendrier cycles)      # rappels
  ├── EPIC-14 (Alerte sanitaire)       # alertes rayon
  ├── EPIC-22 (SLA dégrade)
  ├── EPIC-23 (Wishlist notifs prix)
  └── EPIC-28 (Tracking livraison)

EPIC-09 (Geoloc)
  ├── EPIC-14 (Alerte sanitaire)       # rayon foyer
  ├── EPIC-26 (Reco IA)
  ├── EPIC-28 (Tracking livraison)
  └── EPIC-29 (Calcul frais livraison)

EPIC-10 (Vendor analytics)
  ├── EPIC-11 (Liste noire)            # detecter fraude via stats
  ├── EPIC-21 (A/B testing)            # metrique cible
  ├── EPIC-22 (SLA)                    # data source
  └── EPIC-26 (Reco IA)                # features utilisateurs

EPIC-04 (Chat)
  ├── EPIC-19 (Pieces jointes)
  └── EPIC-22 (SLA reponse chat)

EPIC-13 (Calendrier)
  ├── EPIC-17 (Tracabilite)
  └── EPIC-25 (Estimation poids CV)

EPIC-15 (Vaccins)
  └── EPIC-17 (Tracabilite)            # vaccination par lot

EPIC-17 (Tracabilite)
  └── [terminal]

EPIC-20 (Admin panel)
  ├── EPIC-06 (KYC)                    # queue validation
  ├── EPIC-07 (Escrow)                 # disputes
  ├── EPIC-11 (Liste noire)
  ├── EPIC-16 (Halal)                  # validation cert
  └── EPIC-03 (i18n)                   # gestion traductions

EPIC-21 (A/B testing)
  └── EPIC-26 (Reco IA)                # test versus baseline

EPIC-22 (SLA)
  └── EPIC-26 (Reco IA)                # feature score

EPIC-28 (Tracking)
  └── EPIC-29 (Frais livraison)        # distance reelle
```

---

## 4. Critical path — epics bloquants > 2 autres

Les epics à traiter en priorité absolue car ils débloquent plus de 2 autres epics :

| Epic    | # d'epics dependants | Rang critique |
|---------|----------------------|---------------|
| EPIC-06 (KYC)           | 9 (07, 11, 12, 15, 16, 18, 22, 27, 30) | **P0-CRIT**   |
| EPIC-01 (Mobile Money)  | 8 (07, 10, 12, 15, 22, 24, 27, 31)     | **P0-CRIT**   |
| EPIC-05 (PWA offline)   | 5 (04, 08, 09, 10, 28)                 | **P0-CRIT**   |
| EPIC-08 (Push PWA)      | 5 (13, 14, 22, 23, 28)                 | **P0-CRIT**   |
| EPIC-09 (Geoloc)        | 4 (14, 26, 28, 29)                     | **P0-CRIT**   |
| EPIC-10 (Analytics)     | 4 (11, 21, 22, 26)                     | **P0-CRIT**   |
| EPIC-02 (SMS OTP)       | 4 (01, 06, 18, 30)                     | **P0-CRIT**   |
| EPIC-07 (Escrow)        | 2 (12, 24)                             | P0            |
| EPIC-13 (Calendrier)    | 2 (17, 25)                             | P1            |
| EPIC-03 (i18n)          | ~transverse                            | **P0-CRIT**   |
| EPIC-20 (Admin panel)   | ~transverse (ops)                      | P1-CRIT       |
| EPIC-04 (Chat)          | 2 (19, 22)                             | P0            |

**Conclusion** : la séquence S1 (EPIC-02 → EPIC-01) → S3 (EPIC-05) → S4 (EPIC-06) est le vrai chemin critique, toute dérive S1-S4 décale tout le roadmap d'autant.

---

## 5. Burndown dev-jours cumulés

| Sprint | Eff sprint (dj) | Cumulé (dj) | % roadmap pilote (/450) |
|--------|-----------------|-------------|-------------------------|
| S1     | 35              | 35          | 7.8%                    |
| S2     | 38              | 73          | 16.2%                   |
| S3     | 40              | 113         | 25.1%                   |
| S4     | 42              | 155         | 34.4%                   |
| S5     | 38              | 193         | 42.9%                   |
| S6     | 44              | 237         | 52.7%                   |
| S7     | 40              | 277         | 61.6%                   |
| S8     | 38              | 315         | 70.0%                   |
| S9     | 30              | 345         | 76.7%                   |
| S10    | 42              | 387         | 86.0%                   |
| S11    | 33              | 420         | 93.3%                   |
| S12    | 30              | 450         | 100.0%                  |

Vélocité moyenne : 37.5 dj/sprint (avec 4 FTE = 9.4 dj/FTE sur 2 semaines = 4.7 dj/FTE/semaine — conforme benchmark Spring/Angular en Afrique de l'Ouest incluant meetings/support).

### Graphe burndown (ASCII)

```
Dev-jours cumules (pilote 6 mois)
450 |                                                   ###
    |                                               ####
400 |                                          ####
    |                                      ####
350 |                                  ####
    |                              ####
300 |                          ####
    |                      ####
250 |                  ####
    |              ####
200 |          ####
    |       ###
150 |      #
    |    ##
100 |   #
    |  #
 50 | #
    |#
  0 +----+----+----+----+----+----+----+----+----+----+----+----+
     S1   S2   S3   S4   S5   S6   S7   S8   S9   S10  S11  S12
```

---

## 6. Team size assumption (4 FTE)

| Rôle                          | ETP | Ratio | Charge cible |
|-------------------------------|-----|-------|--------------|
| Frontend Angular 21 + PWA     | 2   | 50%   | 80 dj/sprint (4.0 * 2 FTE * 10 j utiles) |
| Backend Spring Boot Java 21   | 1   | 25%   | 40 dj/sprint |
| QA Playwright + E2E + manuel  | 1   | 25%   | 40 dj/sprint |
| **Total brut**                | 4   | 100%  | 160 dj/sprint |
| **Overhead 15%**              |     |       | -24 dj       |
| **Utile cible**               |     |       | **136 dj**   |
| **Plan actual sprint**        |     |       | ~40 dj (Scrum cadencé sur features completes, pas tasks) |

Note : le "dev-jour" retenu ici désigne un job-point d'epic abouti (pas un micro-ticket). Un sprint = ~40 dj agrégés soit ~3-4 epics parallèles selon tailles.

---

## 7. Risques inter-epics

### Risques majeurs

1. **Sandbox Orange Money instable** (EPIC-01) : retarder S1-S2 → planifier fallback Wave/Moov en parallèle, pas séquentiel.
2. **Approbation Twilio numéro BF** (EPIC-02) : délai administratif 5-10j, commencer dès J-0 pilote.
3. **Traduction Mooré/Dioula/Fulfulde** (EPIC-03) : relecteurs natifs à contracter en S1, sinon décalage S3.
4. **OCR CNIB** (EPIC-06) : anciens formats mal reconnus, prévoir fallback saisie manuelle + validation humaine.
5. **Partenariat grossistes vaccins** (EPIC-15) : négociation contractuelle longue (3-6 mois), commencer en S3-S4 pour viser S7.
6. **Flux CILSS/Ministère Élevage** (EPIC-14) : souvent manuel ou retard, prévoir webhook + email scraping.
7. **Partenaire assureur** (EPIC-12) : si pas signé avant S11, glisser epic en v1.2.

### Risques techniques

- **Quotas IndexedDB** (EPIC-05) : taille limite ~50 MB variable selon browsers, prévoir purge intelligente.
- **iOS Safari push** (EPIC-08) : iOS 16.4+ requis, impact 30% utilisateurs iPhone. Fallback SMS actif.
- **Latence gRPC E-W** avec 107 microservices (FASO écosystème) : pas un risque direct epic-level mais à surveiller au fur et à mesure.

### Risques métier

- **Charge sprint 6** (44 dj, 3 epics P0 en parallèle) : si dette s'accumule S1-S5, couper EPIC-10 pour report S8.
- **Charge sprint 10** (42 dj, 3 epics avec Admin panel 3 semaines) : risque de glisser sur S11. Prévoir de démarrer EPIC-20 en S9.

### Mitigation generale

- **Buffer S12** dédié incidents, polish et consolidation — non-negociable.
- **Feature flags GrowthBook** (EPIC-21) dès son merge S11 pour shipper même une fonctionnalité incomplète.
- **Weekly sync team** (2h) + **monthly roadmap review** (Product + Tech Lead) pour réajustements.

---

## 8. Out-of-scope pilote (v1.2+)

Liste des epics reportés au-delà du pilote 6 mois :

| Epic   | Titre                                    | Priorité | Effort | Raison report |
|--------|------------------------------------------|----------|--------|---------------|
| EPIC-12| Assurance transaction                    | P1       | 2w     | dépend partenaire assureur signé |
| EPIC-24| Abonnements récurrents                   | P2       | 2w     | monetisation v1.2 |
| EPIC-25| Estimation poids/gain IA                 | P2       | 4w     | dataset à annoter |
| EPIC-26| Recommandation IA                        | P2       | 3w     | maturité data nécessaire |
| EPIC-27| Factures PDF                             | P2       | 1w     | conformité post-pilote |
| EPIC-28| Tracking livraison                       | P2       | 2w     | dépend transporteurs équipés |
| EPIC-29| Calcul frais livraison                   | P2       | 1w     | dépend EPIC-28 |
| EPIC-30| Délégation compte                        | P2       | 1w     | niche B2B |
| EPIC-31| Parrainage                               | P2       | 1w     | growth post-PMF |
| EPIC-32| Export RGPD                              | P2       | 1w     | démarche manuelle en v1 |

Total hors pilote : **~170 dev-jours**.

---

## 9. Liens

- Backlog épics détaillés : [BACKLOG-EPICS.md](./BACKLOG-EPICS.md)
- Script import GitHub : [../scripts/github-issues-import.sh](../scripts/github-issues-import.sh)
- Feature flags : [feature-flags.md](./feature-flags.md)
- Runbook stack local : [../RUNBOOK-LOCAL-STACK.md](../RUNBOOK-LOCAL-STACK.md)
