# BACKLOG-EPICS — Plateforme Poulets (FASO DIGITALISATION)

> Version : 1.0
> Date : 2026-04-18
> Auteur : Team Product FASO DIGITALISATION
> Cible : MVP pilote Burkina Faso — éleveurs + clients + vendeurs vaccins + transporteurs
> Périmètre : 33 epics (10 P0, 13 P1, 10 P2) répartis sur 12 sprints

## Contexte produit

- **Devise unique** : FCFA (XOF), aucune conversion multi-devise en v1
- **Langues cibles** : Français (par défaut), Mooré, Dioula, Fulfulde (4 locales)
- **Lois applicables** : Loi 010-2004 (protection données Burkina), OHADA (actes uniformes), CILSS (sécurité sanitaire régionale), RGS télécoms ARCEP-BF
- **Paiement** : Orange Money Burkina, Moov Africa Money, Wave (couvrent 75%+ du volume mobile money national)
- **Public** : 60% éleveurs ruraux (connexion 3G instable, smartphones entry-level), 30% clients urbains, 10% professionnels (vétérinaires, vendeurs vaccins, transporteurs)
- **Infra** : 3G instable dans 40% des régions, 15% de coupures électriques quotidiennes (PWA offline-first obligatoire)
- **Stack** : Angular 21 (frontend PWA), Spring Boot Java 21 (backend), GraphQL N-S, gRPC E-W, KAYA (cache/session), Vault (secrets), ORY Kratos/Keto (auth/authz), Prometheus/Loki/Tempo (observabilité)

## Légende priorités

- **P0** : Bloquant MVP. Sans cet epic, la plateforme ne peut pas lancer en pilote.
- **P1** : Haute valeur métier, peut être livré en v1.1 si retard mais critique à 6 mois.
- **P2** : Enrichissement / growth / conformité étendue. Peut glisser en v1.2+.

---

## EPIC-01 : Paiement Mobile Money (Orange/Moov/Wave)

**Priority**: P0 — 75%+ du volume de paiement au Burkina, sans lui aucune transaction possible.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-payment`, `role-client`, `role-eleveur`, `priority-P0`

### Problème utilisateur
Les éleveurs ruraux et clients urbains n'ont pas de carte bancaire (<5% bancarisation). Ils utilisent quotidiennement Orange Money, Moov Africa Money et Wave pour régler fournisseurs et recevoir paiements. La plateforme doit intégrer ces 3 rails, initier un paiement depuis l'app et confirmer la transaction par webhook.

### Valeur business
Sans paiement Mobile Money, 0 transaction. MoM = 100% du GMV (Gross Merchandise Value). ROI immédiat dès le premier paiement effectué.

### Critères de succès (SMART)
- [ ] Taux de succès paiement ≥ 92% sur 7 jours glissants (mesuré par Prometheus `payment_success_total / payment_attempt_total`)
- [ ] Latence médiane init paiement ≤ 3s, P95 ≤ 8s
- [ ] 3 opérateurs intégrés : Orange Money, Moov Africa Money, Wave, avec fallback automatique si un opérateur est down >5min
- [ ] Webhook de confirmation signé HMAC-SHA256, rejouable idempotent (clé unique `payment_intent_id`)
- [ ] Remboursement (refund) déclenchable depuis back-office en <24h

### Technical approach
- Stack : Spring Boot + WebClient réactif, secrets Vault par opérateur, KAYA pour cache tokens OAuth, gRPC vers `payment-ms`
- Dépendances : EPIC-07 (Escrow) pour séquestre, EPIC-02 (SMS OTP) pour confirmation client
- Risks : Sandbox Orange Money instable, rate-limits Wave (10 req/s), rotation secrets à gérer (expiration 90j)

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (scénario paiement réussi + 3 scénarios d'échec)
- [ ] Documentation FR (guide intégration, FAQ remboursements)
- [ ] i18n FR+Moore+Dioula+Fulfulde (messages UI + SMS)
- [ ] Observabilité (Prometheus metrics `payment_*`, logs structurés Loki, traces Tempo)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-01`

---

## EPIC-02 : SMS OTP (authentification + notifications)

**Priority**: P0 — Base de l'authentification, 80% utilisateurs ruraux sans email actif.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-auth`, `role-all`, `priority-P0`

### Problème utilisateur
Les éleveurs et clients n'ont pas d'email lisible ni fiable. Ils ont un numéro de téléphone (souvent partagé en famille). L'authentification passe par un OTP SMS 6 chiffres, valable 5 minutes. Les notifications critiques (confirmation paiement, livraison en route) doivent aussi arriver par SMS.

### Valeur business
Sans SMS OTP, pas d'inscription, pas de login, pas de confirmation de transaction. Onboarding bloqué à 0%.

### Critères de succès (SMART)
- [ ] Taux de livraison SMS ≥ 95% sur 24h (mesuré via webhook de delivery report)
- [ ] Latence émission → réception médiane ≤ 15s (P95 ≤ 45s)
- [ ] Rate-limit 3 OTP/numéro/5min pour éviter spam
- [ ] OTP invalide après 5 tentatives erronées (verrou 15min)
- [ ] Fournisseur SMS multi-provider : Twilio + fallback opérateur local (Orange Business)

### Technical approach
- Stack : `notifier-ms` Spring Boot, queue Kafka `sms.outbound`, Vault pour clés API, KAYA pour TTL OTP
- Dépendances : Aucune (foundation)
- Risks : Coût SMS (~25 FCFA/SMS), fraude (numéros virtuels), latence opérateurs en zone rurale

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (login OTP + échec + rate-limit)
- [ ] Documentation FR (politique OTP, guide support)
- [ ] i18n FR+Moore+Dioula+Fulfulde (gabarit SMS)
- [ ] Observabilité (Prometheus `sms_delivery_*`, alertes si taux échec >10%)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-02`

---

## EPIC-03 : Multi-langue (FR + Mooré + Dioula + Fulfulde)

**Priority**: P0 — 40% des utilisateurs ruraux parlent peu ou pas français, UX bloquante sans.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-i18n`, `role-all`, `priority-P0`

### Problème utilisateur
Le français est la langue officielle mais 40% des utilisateurs ruraux (éleveurs notamment) parlent principalement Mooré (Plateau Mossi), Dioula (ouest, sud-ouest) ou Fulfulde (nord, est). Sans localisation dans ces 3 langues, adoption rurale impossible.

### Valeur business
Adoption +60% en zones rurales selon études pilotes CILSS. Augmente la base utilisateurs cible de 250k à 400k.

### Critères de succès (SMART)
- [ ] 100% des chaînes UI internationalisées dans 4 locales (FR, MOS, DYU, FUL)
- [ ] Sélecteur de langue accessible depuis l'onboarding et le menu profil
- [ ] Dates et nombres formatés par locale (ICU MessageFormat)
- [ ] SMS et emails traduits selon la langue du compte
- [ ] Traduction validée par locuteurs natifs (relecture externe contractuelle, min 2 relecteurs/langue)

### Technical approach
- Stack : Angular i18n (ICU + lazy-loading chunks par locale), backend `Accept-Language` header, table `i18n_translations` Postgres pour contenu éditable par admin
- Dépendances : EPIC-02 (SMS) pour gabarits SMS, EPIC-10 (Admin panel) pour gestion traductions
- Risks : Coût traduction (~500k FCFA par passe), dérive lexicale entre sprints, absence de standard Unicode moderne pour Mooré (à figer dès sprint 1)

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (change locale + vérif 10 écrans clés)
- [ ] Documentation FR (guide contributeur traductions)
- [ ] i18n FR+Moore+Dioula+Fulfulde (livrable lui-même)
- [ ] Observabilité (metrics `i18n_locale_selected_total` par locale)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-03`

---

## EPIC-04 : Chat temps réel éleveur <-> client

**Priority**: P0 — Négociation prix/quantité obligatoire avant commande, cœur UX marketplace.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-messaging`, `role-eleveur`, `role-client`, `priority-P0`

### Problème utilisateur
Avant de passer commande, client et éleveur doivent discuter quantité, race, âge, prix négocié, modalités de livraison. WhatsApp est utilisé aujourd'hui mais sort le client de la plateforme (pas de traçabilité paiement/litige). Le chat in-app garde la conversation attachée à la commande.

### Valeur business
+35% de conversion commande selon benchmarks marketplaces B2C en Afrique de l'Ouest (Jumia, Glovo). Réduit risque de fuite de GMV vers WhatsApp.

### Critères de succès (SMART)
- [ ] Latence médiane message ≤ 800ms
- [ ] Persistance 100% des messages (pas de perte même si destinataire offline)
- [ ] Indicateur "lu / reçu / envoyé" visible
- [ ] Historique 90 jours consultable
- [ ] Support images (EPIC-09) et texte uniquement en v1 (pas de voix/vidéo)

### Technical approach
- Stack : WebSocket via Spring Reactive + STOMP, persistance Postgres (table `chat_messages`), KAYA pour présence/typing, fallback polling 3s si WS bloqué
- Dépendances : EPIC-05 (PWA offline) pour queue messages offline
- Risks : Connexion 3G instable → WS drop fréquent, modération automatique anti-fuite numéro téléphone

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (2 browsers, envoi/réception + offline)
- [ ] Documentation FR (guide utilisateur chat)
- [ ] i18n FR+Moore+Dioula+Fulfulde (UI chat + notifs)
- [ ] Observabilité (metrics `chat_message_total`, `chat_ws_disconnect_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-04`

---

## EPIC-05 : PWA offline-first

**Priority**: P0 — 3G instable + 15% coupures élec quotidiennes, app sans offline = inutilisable rural.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-pwa`, `role-all`, `priority-P0`

### Problème utilisateur
40% des régions ont 3G instable, 15% coupures élec quotidiennes. Un utilisateur doit pouvoir consulter son catalogue, préparer une commande, saisir un formulaire sans connexion, puis synchroniser quand réseau revient. Cache + queue + IndexedDB.

### Valeur business
Rétention D+7 passe de 35% à 65% selon études pilotes Flutterwave en zones rurales WAEMU. Réduit abandon commande de 50%.

### Critères de succès (SMART)
- [ ] App installable sur écran d'accueil (manifest + icônes + splash)
- [ ] Service Worker précache les assets critiques (<500 KB)
- [ ] 5 écrans clés consultables offline (catalogue, commande en cours, profil, chat 24h, calendrier)
- [ ] Queue d'actions offline (commandes, messages, formulaires) répliquée au retour online
- [ ] Indicateur réseau visible en permanence (online / dégradé / offline)

### Technical approach
- Stack : Angular 21 Service Worker, IndexedDB via Dexie.js, Background Sync API, Workbox strategies (stale-while-revalidate)
- Dépendances : EPIC-04 (Chat) pour queue messages, EPIC-08 (Notifs push) pour réveil sync
- Risks : Quotas IndexedDB (~50 MB typique), conflit sync si édition offline simultanée serveur

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (mode offline + sync)
- [ ] Documentation FR (guide installation PWA)
- [ ] i18n FR+Moore+Dioula+Fulfulde (messages offline)
- [ ] Observabilité (metrics `pwa_install_total`, `pwa_sync_queue_size`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-05`

---

## EPIC-06 : KYC biométrique (CNIB + photo visage)

**Priority**: P0 — Obligation Loi 010-2004 + anti-fraude escrow, condition réglementaire.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-kyc`, `role-eleveur`, `role-vendeur`, `priority-P0`

### Problème utilisateur
Pour activer l'escrow et limiter la fraude, chaque vendeur (éleveur, vendeur vaccins, transporteur) doit prouver son identité. CNIB (Carte Nationale d'Identité Burkinabè) + selfie avec vérification liveness. Conformité Loi 010-2004 Burkina + bonnes pratiques CILSS.

### Valeur business
Condition nécessaire pour escrow (EPIC-07), réduit fraude de 80% selon benchmarks Afrique de l'Ouest. Sans KYC, pas d'escrow, pas de confiance plateforme.

### Critères de succès (SMART)
- [ ] Upload recto/verso CNIB + selfie en <2 min (P95)
- [ ] OCR auto-remplit nom, prénom, date de naissance, N° CNIB (précision ≥ 90%)
- [ ] Vérification liveness (clignement, tourner la tête) obligatoire avant validation
- [ ] Délai validation humaine ≤ 24h ouvrées, SLA affiché à l'utilisateur
- [ ] Chiffrement AES-256 des images au repos, suppression après 5 ans (Loi 010-2004)

### Technical approach
- Stack : AWS Rekognition ou service souverain (ONATEL partenariat), MinIO pour stockage, pipeline workflow `kyc-ms` Spring Boot, queue Kafka `kyc.review`
- Dépendances : EPIC-03 (i18n) pour UI, EPIC-10 (Admin panel) pour validation manuelle
- Risks : Coût OCR cloud (~5 FCFA/doc), PII sensible (chiffrement obligatoire), CNIB anciens formats non reconnus

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (upload + OCR + liveness)
- [ ] Documentation FR (guide utilisateur KYC, procédure support)
- [ ] i18n FR+Moore+Dioula+Fulfulde (UI + emails validation)
- [ ] Observabilité (metrics `kyc_submitted_total`, `kyc_validation_latency_seconds`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-06`

---

## EPIC-07 : Escrow Mobile Money (séquestre paiement)

**Priority**: P0 — Protection acheteur+vendeur = condition minimale confiance marketplace.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-payment`, `role-client`, `role-eleveur`, `priority-P0`

### Problème utilisateur
L'acheteur paye avant de recevoir la marchandise (poulets vivants), le vendeur craint de ne pas être payé après livraison. L'escrow séquestre le paiement, le libère au vendeur uniquement après confirmation de livraison par l'acheteur (ou timeout 72h + médiation).

### Valeur business
+40% conversion sur panier >50k FCFA selon études marketplaces WAEMU. Réduit disputes de 70%. Protection juridique OHADA.

### Critères de succès (SMART)
- [ ] Séquestre actif à 100% des commandes >10k FCFA
- [ ] Libération automatique T+72h après confirmation livraison (configurable par catégorie)
- [ ] Procédure de dispute ouvrable en <1 clic avec upload preuves (photos, SMS)
- [ ] Médiation humaine SLA ≤ 48h ouvrées
- [ ] Remboursement intégral acheteur si dispute gagnée

### Technical approach
- Stack : `escrow-ms` Spring Boot, table `escrow_transactions` Postgres, workflow state machine (PENDING → HELD → RELEASED / REFUNDED / DISPUTED), KAYA pour locks
- Dépendances : EPIC-01 (Mobile Money), EPIC-06 (KYC), EPIC-10 (Admin panel pour médiation)
- Risks : Complexité comptable (registre écritures), obligations OHADA tenue livres, trésorerie flottante à gérer

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (escrow + confirm + dispute)
- [ ] Documentation FR (CGV escrow, procédure dispute)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `escrow_held_total`, `escrow_dispute_ratio`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-07`

---

## EPIC-08 : Notifications push PWA

**Priority**: P0 — Réengagement et alertes critiques (paiement, livraison, chat) sans SMS payant.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-notifications`, `role-all`, `priority-P0`

### Problème utilisateur
Les utilisateurs doivent être notifiés de : nouveau message chat, paiement confirmé, livraison en route, dispute ouverte. SMS = 25 FCFA chacun (cher), push PWA = gratuit et plus riche (icône, deep-link).

### Valeur business
Économise ~30k FCFA/mois SMS pour 10k utilisateurs actifs. Réengagement D+1 +50% via push vs email.

### Critères de succès (SMART)
- [ ] Opt-in push natif au premier login PWA, taux acceptation ≥ 55%
- [ ] Délai émission → réception médian ≤ 5s
- [ ] Templates notifications localisés 4 langues
- [ ] Deep-link vers écran cible (chat, paiement, commande)
- [ ] Désactivation granulaire par catégorie (marketing, transactionnel, chat)

### Technical approach
- Stack : Web Push API (VAPID), service `notifier-ms`, queue Kafka `push.outbound`, Firebase Cloud Messaging comme fallback Android natif futur
- Dépendances : EPIC-05 (PWA)
- Risks : iOS Safari push limité (iOS 16.4+), tokens révoqués silencieusement, coût VAPID négligeable

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (opt-in + réception)
- [ ] Documentation FR (politique notifs, opt-out)
- [ ] i18n FR+Moore+Dioula+Fulfulde (templates push)
- [ ] Observabilité (metrics `push_sent_total`, `push_click_through_rate`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-08`

---

## EPIC-09 : Géolocalisation matching éleveur <-> client

**Priority**: P0 — Réduire distance livraison = délai/coût/qualité poulets vivants.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-geo`, `role-client`, `role-eleveur`, `priority-P0`

### Problème utilisateur
Un client à Ouagadougou ne doit pas commander à un éleveur à Bobo-Dioulasso (distance 360km, poulets vivants ne survivent pas). Le matching doit prioriser les éleveurs à <50 km du client, rayon configurable.

### Valeur business
Réduit taux d'annulation livraison de 40% à 10%. Améliore NPS vendeur de 15 pts (moins de trajets longs).

### Critères de succès (SMART)
- [ ] Recherche catalogue filtrée par rayon (10/25/50/100 km)
- [ ] Tri par distance croissante par défaut
- [ ] Carte interactive OpenStreetMap (pas Google Maps, coût + souveraineté)
- [ ] Géoloc opt-in, fallback saisie ville/commune
- [ ] Précision ≤ 2 km en urbain, ≤ 10 km en rural

### Technical approach
- Stack : PostGIS extension Postgres, Leaflet OSM côté frontend, API `/geo/nearby` GraphQL, KAYA pour cache tuiles OSM
- Dépendances : EPIC-05 (PWA) pour cache tuiles offline
- Risks : Données OSM parfois datées en zones rurales, géoloc mobile imprécise sans GPS

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (recherche + carte + rayon)
- [ ] Documentation FR (guide éleveur : comment déclarer sa zone)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `geo_search_total`, `geo_matching_latency`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-09`

---

## EPIC-10 : Vendor analytics dashboard

**Priority**: P0 — Sans pilotage data, éleveurs abandonnent plateforme en 30j (benchmark).
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-analytics`, `role-eleveur`, `role-vendeur`, `priority-P0`

### Problème utilisateur
Un éleveur doit suivre : nombre de poulets vendus par semaine, chiffre d'affaires, panier moyen, taux de conversion visite → commande, évaluations clients. Sans ce feedback, il ne peut pas optimiser son offre.

### Valeur business
Rétention vendeurs D+30 passe de 40% à 75% quand un dashboard est disponible. Double le GMV par vendeur actif à 6 mois.

### Critères de succès (SMART)
- [ ] 8 KPIs affichés : CA jour/semaine/mois, commandes, visites, conversion, panier moyen, note moyenne, taux annulation
- [ ] Filtre période (7j/30j/90j/custom)
- [ ] Export CSV et PDF
- [ ] Mise à jour données ≤ 5 min de latence
- [ ] Dashboard consultable offline (dernière synchro cachée)

### Technical approach
- Stack : Angular charts (ngx-charts), agrégations matérialisées Postgres, refresh asynchrone Kafka + MATERIALIZED VIEW, KAYA cache 5min
- Dépendances : EPIC-01 (Paiement pour CA), EPIC-05 (PWA offline)
- Risks : Requêtes lourdes sur gros volumes (indexer `created_at`), cohérence eventual acceptée

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (dashboard + filtres + export)
- [ ] Documentation FR (guide éleveur analytics)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `dashboard_view_total`, `dashboard_export_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-10`

---

## EPIC-11 : Liste noire partagée vendeurs fraudeurs

**Priority**: P1 — Confiance écosystème, amplifie impact KYC, coût implémentation faible.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-trust`, `role-admin`, `priority-P1`

### Problème utilisateur
Un vendeur banni pour fraude ne doit pas pouvoir recréer un compte avec un autre numéro. Liste noire basée sur CNIB (EPIC-06), IP, device fingerprint, numéro de compte Mobile Money.

### Valeur business
Réduit récidives fraude de 95%. Économise ~500k FCFA/mois en pertes escrow.

### Critères de succès (SMART)
- [ ] 4 vecteurs de blocage : CNIB, IP, device FP, compte MM
- [ ] Vérif à l'inscription + à chaque KYC
- [ ] Droit d'appel utilisateur (contester blocage) en <48h
- [ ] Export quotidien liste noire vers partenaires OHADA (futur)
- [ ] Conformité RGPD Burkina (effacement sur appel validé)

### Technical approach
- Stack : Table `blacklist_entries` Postgres, service `trust-ms` Spring Boot, API interne + webhook vers auth-ms
- Dépendances : EPIC-06 (KYC), EPIC-10 (analytics pour détecter fraude)
- Risks : Faux positifs, usurpation CNIB, RGPD Burkina à respecter

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (blocage + appel)
- [ ] Documentation FR (procédure blocage/déblocage)
- [ ] i18n FR+Moore+Dioula+Fulfulde (emails notifs)
- [ ] Observabilité (metrics `blacklist_hit_total`, `blacklist_appeal_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-11`

---

## EPIC-12 : Assurance transaction (garantie commande)

**Priority**: P1 — Différenciation concurrentielle, upsell premium vendeur.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-trust`, `role-client`, `priority-P1`

### Problème utilisateur
Un client qui achète 50 poulets (>500k FCFA) veut une garantie contre perte/maladie/mortalité en transport. Une micro-assurance optionnelle (2-3% du panier) couvre jusqu'à 80% en cas de sinistre.

### Valeur business
+25% panier moyen sur transactions >200k FCFA. Revenus commission 0.5-1% reversés à partenaire assureur.

### Critères de succès (SMART)
- [ ] Opt-in checkout, prime calculée dynamiquement (2-3%)
- [ ] Souscription stockée avec police PDF téléchargeable
- [ ] Déclaration sinistre en <5 min + upload photos/vidéos
- [ ] SLA traitement ≤ 7 jours ouvrés
- [ ] Partenaire assureur intégré via API (SONAR, Allianz BF, etc.)

### Technical approach
- Stack : `insurance-ms` Spring Boot, webhook partenaire, stockage polices MinIO
- Dépendances : EPIC-01 (Paiement), EPIC-07 (Escrow), EPIC-06 (KYC)
- Risks : Négociation partenariat assureur (6-12 mois), conformité CIMA

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (souscription + déclaration)
- [ ] Documentation FR (CGV assurance, FAQ sinistres)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `insurance_subscribed_total`, `insurance_claim_ratio`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-12`

---

## EPIC-13 : Calendrier cycles production éleveurs

**Priority**: P1 — Outil métier vertical, augmente rétention et maturité éleveur.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-vertical`, `role-eleveur`, `priority-P1`

### Problème utilisateur
Un éleveur planifie : date d'entrée poussins, vaccinations (J+1, J+14, J+21), sevrage, abattage/vente (J+42 pour poulets de chair). Un calendrier auto-généré + rappels push = productivité +20%.

### Valeur business
Professionnalise les éleveurs informels. Augmente volume et qualité offre plateforme. Rétention éleveurs +15 pts.

### Critères de succès (SMART)
- [ ] Template cycles préconfigurés (chair 42j, ponte 18 semaines, etc.)
- [ ] Rappels push EPIC-08 pour chaque étape
- [ ] Intégration vaccins (EPIC-15) : scan QR = cochage auto
- [ ] Visualisation Gantt + liste
- [ ] Export PDF pour vétérinaire

### Technical approach
- Stack : `production-ms` Spring Boot, moteur de règles (Drools ou YAML), agenda Angular
- Dépendances : EPIC-08 (Push), EPIC-15 (Vaccins ordonnance), EPIC-17 (Traçabilité lot)
- Risks : Diversité cycles (races locales vs industrielles) → templates multiples

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (création cycle + rappel)
- [ ] Documentation FR (guide éleveur cycles)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `production_cycle_active_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-13`

---

## EPIC-14 : Alerte sanitaire aviaire (grippe, Newcastle)

**Priority**: P1 — Santé publique + obligation CILSS, capital image auprès gouvernement.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-vertical`, `role-eleveur`, `role-gov`, `priority-P1`

### Problème utilisateur
Quand une épidémie (grippe aviaire H5N1, Newcastle) éclate dans une région, éleveurs voisins doivent être alertés immédiatement pour confiner. Source : flux CILSS + Ministère Élevage BF.

### Valeur business
Mission de service public. Partenariat gouvernement. Crédibilité plateforme. Évite pertes massives (~200M FCFA lors épisode 2023).

### Critères de succès (SMART)
- [ ] Ingestion flux CILSS quotidien (RSS + email + API partenaire)
- [ ] Alerte push + SMS aux éleveurs dans un rayon de 50 km du foyer
- [ ] Géoloc précise par commune
- [ ] Historique alertes consultable 3 ans
- [ ] Validation humaine avant diffusion (anti-faux-positifs)

### Technical approach
- Stack : `sanitary-ms` Spring Boot, scraper + webhook partenaire ministère, jobs quartz quotidien
- Dépendances : EPIC-08 (Push), EPIC-02 (SMS), EPIC-09 (Géoloc)
- Risks : Disponibilité flux gouvernemental (souvent manuel), responsabilité juridique si manqué

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (ingestion + alerte)
- [ ] Documentation FR (partenariat CILSS, procédure)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `sanitary_alert_sent_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-14`

---

## EPIC-15 : Marketplace vaccins sur ordonnance

**Priority**: P1 — Revenu additionnel + intégration verticale éleveurs, levier cross-sell.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-marketplace`, `role-eleveur`, `role-vendeur-vaccin`, `role-veterinaire`, `priority-P1`

### Problème utilisateur
Les éleveurs ont besoin de vaccins (Gumboro, Newcastle, Marek) disponibles chez grossistes agréés. Certains vaccins sont sous ordonnance vétérinaire. La plateforme connecte éleveurs + vétérinaires + grossistes, vérifie ordonnance, facture et livre.

### Valeur business
Nouveau segment revenu (commission 5-10%). Augmente professionnalisation éleveur. GMV secondaire +20%.

### Critères de succès (SMART)
- [ ] Catalogue vaccins avec distinction avec/sans ordonnance
- [ ] Workflow ordonnance : upload → validation vétérinaire partenaire → achat
- [ ] Chaîne froid : étiquette + validation transporteur certifié
- [ ] Traçabilité lot → éleveur → poulailler (intègre EPIC-17)
- [ ] Conformité DGSV Burkina (Direction Générale des Services Vétérinaires)

### Technical approach
- Stack : `pharmacy-ms` Spring Boot, catalog séparé du catalog poulets, workflow approval Camunda ou state machine
- Dépendances : EPIC-06 (KYC vétérinaire), EPIC-01 (Paiement), EPIC-17 (Traçabilité)
- Risks : Réglementation vétérinaire stricte, chaîne froid logistique, stocks grossistes

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (commande vaccin + ordonnance)
- [ ] Documentation FR (guide vétérinaire + grossiste)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `vaccine_order_total`, `prescription_validation_latency`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-15`

---

## EPIC-16 : Certificat halal

**Priority**: P1 — Marché musulman (>60% population BF), exigence forte B2C urbain.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-trust`, `role-eleveur`, `role-client`, `priority-P1`

### Problème utilisateur
La majorité des clients urbains (Ouaga, Bobo) exigent viande halal (abattage rituel). Les éleveurs certifiés doivent pouvoir afficher le label, les clients filtrer par halal, les certificats être vérifiables.

### Valeur business
Ouvre segment premium +30% panier moyen urbain. Rassure communauté musulmane.

### Critères de succès (SMART)
- [ ] Badge "Halal certifié" visible fiche éleveur + commande
- [ ] Upload certificat délivré par Fédération Associations Islamiques (FAIB)
- [ ] Validation semestrielle automatique (renouvellement)
- [ ] Filtre catalogue "halal uniquement"
- [ ] QR code sur emballage livraison pointant vers certificat PDF

### Technical approach
- Stack : Table `halal_certificates` Postgres, workflow validation admin, intégration future API FAIB
- Dépendances : EPIC-06 (KYC), EPIC-10 (Admin panel)
- Risks : Expiration non renouvelée, diversité certifiants (3 fédérations reconnues BF)

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (upload cert + filtre catalogue)
- [ ] Documentation FR (procédure halal)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `halal_certified_vendors_total`, `halal_filter_use_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-16`

---

## EPIC-17 : Traçabilité lot poulets (bout en bout)

**Priority**: P1 — Conformité CILSS + confiance urbaine, ouvre exports futurs.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-vertical`, `role-eleveur`, `role-client`, `priority-P1`

### Problème utilisateur
Un client (ou inspecteur DGSV) doit pouvoir scanner un QR code et voir : ferme d'origine, date d'entrée poussin, vaccinations reçues, date d'abattage, transporteur. Conformité CILSS = ouvre marchés export.

### Valeur business
Premium qualité +20% prix. Ouvre exports CEDEAO. Conformité réglementaire.

### Critères de succès (SMART)
- [ ] Chaque lot a un ID unique + QR code imprimable
- [ ] Timeline complète : éclosion, livraison, vaccinations, abattage, livraison client
- [ ] Immutabilité événements (append-only, audit)
- [ ] Consultation publique via QR sans login
- [ ] Export PDF certificat traçabilité

### Technical approach
- Stack : `traceability-ms` Spring Boot, table `lot_events` append-only, QR via zxing, hash chaîné (pas blockchain — trop lourd en v1)
- Dépendances : EPIC-13 (Calendrier), EPIC-15 (Vaccins)
- Risks : Éleveurs réticents à la saisie détaillée, données incomplètes

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (création lot + timeline + QR)
- [ ] Documentation FR (guide éleveur traçabilité)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `lot_created_total`, `traceability_qr_scan_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-17`

---

## EPIC-18 : Signature électronique (contrats, mandats)

**Priority**: P1 — Conformité OHADA, permet partenariats B2B formels.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-legal`, `role-eleveur`, `role-b2b`, `priority-P1`

### Problème utilisateur
Contrats d'approvisionnement récurrent (ex : restaurant qui commande 500 poulets/mois) doivent être signés électroniquement. Mandats escrow, contrats assurance. Conformité OHADA actes uniformes.

### Valeur business
Ouvre segment B2B (restaurants, hôtels, abattoirs). Contrats récurrents = revenus prédictibles.

### Critères de succès (SMART)
- [ ] Signature OTP SMS simple + trace audit (eIDAS simple equivalent OHADA)
- [ ] Horodatage certifié (timestamping serveur)
- [ ] PDF signé téléchargeable + archivage 10 ans
- [ ] Révocation/annulation gérée
- [ ] Intégration DocuSign ou souverain à définir

### Technical approach
- Stack : `signature-ms` Spring Boot, pdfbox pour sceller PDF, horodatage via NTP certifié, stockage MinIO WORM
- Dépendances : EPIC-02 (SMS OTP), EPIC-06 (KYC)
- Risks : Conformité OHADA (signature "simple" suffit en v1), archivage 10 ans = volume

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (signer + vérifier)
- [ ] Documentation FR (valeur juridique, FAQ)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `signature_created_total`, `signature_verified_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-18`

---

## EPIC-19 : Chat pièces jointes (images, documents, audio)

**Priority**: P1 — Enrichir chat, photos poulets = 80% demande éleveurs.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-messaging`, `role-all`, `priority-P1`

### Problème utilisateur
Un client demande "envoyez-moi une photo des poulets". Sans pièce jointe, l'éleveur repasse sur WhatsApp. Support images + PDF + audio (note vocale 60s) = chat complet.

### Valeur business
+25% conversion commande. Réduit fuite vers WhatsApp.

### Critères de succès (SMART)
- [ ] Upload images JPG/PNG <5 MB
- [ ] Upload PDF <10 MB
- [ ] Note vocale max 60s (format WebM/Opus)
- [ ] Antivirus ClamAV sur tous uploads
- [ ] Preview inline dans chat

### Technical approach
- Stack : MinIO pour stockage, signed URLs 24h, ClamAV via REST, Angular MediaRecorder API pour audio
- Dépendances : EPIC-04 (Chat)
- Risks : Quotas stockage, coût transfert data sur 3G (compresser images server-side)

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (upload + AV + preview)
- [ ] Documentation FR (limites, formats)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `chat_attachment_total`, `chat_av_blocked_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-19`

---

## EPIC-20 : Admin panel modération

**Priority**: P1 — Sans admin, opérations support impossibles à scale.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-admin`, `role-admin`, `priority-P1`

### Problème utilisateur
L'équipe support (4 FTE pilote) doit : valider KYC, arbitrer disputes escrow, modérer chat (anti-fuite numéro), gérer liste noire, voir analytics globales. Un dashboard admin unifié est nécessaire.

### Valeur business
Scale support de 100 à 10k utilisateurs sans embauche proportionnelle.

### Critères de succès (SMART)
- [ ] 6 modules : KYC queue, Escrow disputes, Chat flags, Blacklist, Users search, Analytics global
- [ ] RBAC fin (Keto) : admin-super, modérateur, KYC-reviewer
- [ ] Audit log toutes actions admin (immuable)
- [ ] Export CSV des listes
- [ ] Recherche full-text utilisateurs

### Technical approach
- Stack : Angular sub-app `/admin`, ORY Keto pour RBAC, Elasticsearch pour recherche, logs audit Loki
- Dépendances : EPIC-06 (KYC), EPIC-07 (Escrow), EPIC-11 (Liste noire)
- Risks : Sécurisation stricte (endpoint séparé, 2FA admin obligatoire), volume logs audit

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (6 modules)
- [ ] Documentation FR (runbook support)
- [ ] i18n FR (admin FR only en v1)
- [ ] Observabilité (metrics `admin_action_total`, alertes actions sensibles)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-20`

---

## EPIC-21 : A/B testing GrowthBook

**Priority**: P1 — Sans expérimentation, décisions produit à l'aveugle.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-growth`, `role-product`, `priority-P1`

### Problème utilisateur
Tester : nouveau flow onboarding, prix suggérés, textes CTA, ordre catalogue. Sans A/B testing, impossible de mesurer impact changements.

### Valeur business
Chaque test A/B gagnant = +5-15% métrique cible. 4 tests/mois = +30% conversion à 6 mois.

### Critères de succès (SMART)
- [ ] Intégration GrowthBook self-hosted
- [ ] Feature flags + experiments UI
- [ ] Assignation stable (hash user_id)
- [ ] Tracking événements vers warehouse analytique
- [ ] Décision statistique (p-value <0.05 ou Bayesian)

### Technical approach
- Stack : GrowthBook Docker self-hosted, SDK Angular + Spring Boot, events vers ClickHouse ou Postgres
- Dépendances : EPIC-10 (Vendor analytics) pour données
- Risks : Souveraineté data (self-hosted obligatoire), volume events

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (feature flag check)
- [ ] Documentation FR (guide PM : comment lancer un test)
- [ ] i18n N/A (outil interne)
- [ ] Observabilité (metrics `experiment_assignment_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-21`

---

## EPIC-22 : SLA vendor monitoring

**Priority**: P1 — Qualité service mesurée = différenciateur confiance.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-trust`, `role-eleveur`, `role-admin`, `priority-P1`

### Problème utilisateur
Un éleveur doit respecter SLA : réponse chat ≤ 2h, expédition ≤ 24h après paiement, taux annulation <5%. Un badge "vendor gold/silver/bronze" récompense les meilleurs.

### Valeur business
Auto-régulation marketplace. Meilleurs vendeurs visibles en premier → meilleur parcours client → rétention.

### Critères de succès (SMART)
- [ ] 4 SLAs mesurés : temps réponse chat, temps expédition, taux annulation, note moyenne
- [ ] Score agrégé recalculé quotidiennement
- [ ] Badge visible fiche vendor
- [ ] Tri catalogue par score par défaut
- [ ] Notification vendor si SLA dégradé

### Technical approach
- Stack : Jobs Quartz quotidiens, agrégations Postgres + MATERIALIZED VIEW, table `vendor_sla_scores`
- Dépendances : EPIC-04 (Chat), EPIC-01 (Paiement), EPIC-10 (Vendor analytics)
- Risks : Seuils à calibrer, contestation vendors, gaming du système

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (score calculé + badge affiché)
- [ ] Documentation FR (charte vendor)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `vendor_sla_score`, distribution)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-22`

---

## EPIC-23 : Wishlist / favoris

**Priority**: P1 — UX classique e-commerce, faible effort + bon signal intention.
**Effort**: 3d
**Labels**: `epic`, `feature`, `domain-ux`, `role-client`, `priority-P1`

### Problème utilisateur
Un client regarde un éleveur, veut y revenir plus tard. Sans favoris, il doit rechercher à nouveau. Favoris = signal d'intention fort.

### Valeur business
+10% taux retour D+7. Source de notifs ciblées (promo éleveur favori).

### Critères de succès (SMART)
- [ ] Bouton cœur sur fiche éleveur + produit
- [ ] Vue "mes favoris" dans profil
- [ ] Sync cross-device
- [ ] Notification push si prix baisse ou nouveau produit chez favori
- [ ] Limite 100 favoris par utilisateur

### Technical approach
- Stack : Table `user_favorites` Postgres, REST endpoint, Angular localstorage sync
- Dépendances : EPIC-08 (Push)
- Risks : N/A (feature simple)

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (ajout/retrait + sync)
- [ ] Documentation FR
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `favorite_added_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-23`

---

## EPIC-24 : Abonnements récurrents (livraison programmée)

**Priority**: P2 — Monétisation B2B, effort moyen, gain long terme.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-payment`, `role-b2b`, `role-client`, `priority-P2`

### Problème utilisateur
Un restaurant commande 50 poulets chaque vendredi. Plutôt que réitérer à la main, un abonnement auto-commande + pré-autorise paiement.

### Valeur business
Revenus récurrents prédictibles. LTV x2. Ouvre segment HORECA.

### Critères de succès (SMART)
- [ ] Fréquences : hebdo, bi-mensuelle, mensuelle
- [ ] Pré-autorisation Mobile Money (token long terme)
- [ ] Skip/pause/annulation en 1 clic
- [ ] Notification 48h avant chaque prélèvement
- [ ] Résiliation sans frais à tout moment

### Technical approach
- Stack : `subscription-ms` Spring Boot, Quartz jobs, tokens pay-on-file MM
- Dépendances : EPIC-01, EPIC-07, EPIC-18
- Risks : Tokens MM durée limitée (renouveler avant expiration), obligation résiliation art. OHADA

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (cycle complet)
- [ ] Documentation FR (CGV abonnement)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `subscription_active_total`, `subscription_churn`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-24`

---

## EPIC-25 : Estimation poids/gain IA (computer vision)

**Priority**: P2 — Feature différenciante tech, nécessite maturité data préalable.
**Effort**: 4w
**Labels**: `epic`, `feature`, `domain-ai`, `role-eleveur`, `priority-P2`

### Problème utilisateur
Un éleveur prend une photo de son poulailler, un modèle CV estime poids moyen, dispersion, gain quotidien. Pilote productivité sans balance manuelle.

### Valeur business
Premium éleveurs pros. Différenciation concurrentielle forte. Data source pour analytics marché.

### Critères de succès (SMART)
- [ ] Précision ±10% sur poids moyen (validée sur 500 photos test)
- [ ] Inférence <5s sur photo 1080p
- [ ] Modèle déployé en edge (PWA) ou cloud selon latence
- [ ] Historique poids auto-enregistré
- [ ] Export CSV

### Technical approach
- Stack : Modèle YOLO + régression poids custom, TensorFlow.js edge ou Python serverless, dataset annotation partenariat 2iE/ENSTP
- Dépendances : EPIC-13 (Calendrier)
- Risks : Données d'entraînement = enjeu majeur, biais races locales, coûts GPU inference

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (upload photo + estimation)
- [ ] Documentation FR (limites modèle)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `cv_inference_total`, `cv_precision_p50`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-25`

---

## EPIC-26 : Recommandation IA matching client <-> éleveur

**Priority**: P2 — Amplifie géoloc + analytics, gain marginal.
**Effort**: 3w
**Labels**: `epic`, `feature`, `domain-ai`, `role-client`, `priority-P2`

### Problème utilisateur
Un client cherche "poulet de chair, halal, livraison 2j". Recommandation IA classe vendeurs par scoring multi-critères (distance, SLA, prix, historique client).

### Valeur business
+15% conversion catalogue → commande. Personnalisation.

### Critères de succès (SMART)
- [ ] Algorithme hybride (content-based + collaborative filtering)
- [ ] Latence <500ms
- [ ] Explainability : "pourquoi ce vendor ? 2.5 km, note 4.8, halal"
- [ ] AB test vs tri par distance (EPIC-21)
- [ ] Refresh modèle hebdo

### Technical approach
- Stack : PyTorch ou LightGBM, features Postgres + Redis/KAYA, API FastAPI ou Spring Boot
- Dépendances : EPIC-09, EPIC-10, EPIC-22, EPIC-21
- Risks : Cold start nouveaux utilisateurs, biais vendeur populaire → riche-devient-plus-riche

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright
- [ ] Documentation FR (explainability)
- [ ] i18n FR+Moore+Dioula+Fulfulde (messages explication)
- [ ] Observabilité (metrics `reco_click_through_rate`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-26`

---

## EPIC-27 : Factures PDF auto-générées

**Priority**: P2 — Conformité OHADA + usage B2B, moyennement urgent.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-billing`, `role-b2b`, `role-eleveur`, `priority-P2`

### Problème utilisateur
Un restaurant a besoin de factures officielles pour comptabilité. Éleveurs aussi (IFU si CA >25M FCFA). PDF normalisé OHADA.

### Valeur business
Débloque segment B2B formel. Conformité fiscale.

### Critères de succès (SMART)
- [ ] Génération PDF auto à chaque commande confirmée
- [ ] Mentions légales OHADA + IFU si applicable
- [ ] Numérotation séquentielle légale (sans trou)
- [ ] Téléchargement depuis espace client et espace éleveur
- [ ] Envoi email + stockage 10 ans

### Technical approach
- Stack : `billing-ms` Spring Boot, templates Jasper ou Thymeleaf + flying-saucer, numérotation atomique Postgres
- Dépendances : EPIC-01 (Paiement), EPIC-06 (KYC pour mentions légales)
- Risks : Numérotation légale = sans gap (bien gérer transactions DB)

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (génération + téléchargement)
- [ ] Documentation FR (conformité OHADA)
- [ ] i18n FR (factures en FR uniquement)
- [ ] Observabilité (metrics `invoice_generated_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-27`

---

## EPIC-28 : Tracking livraison temps réel

**Priority**: P2 — UX premium, nécessite transporteurs équipés.
**Effort**: 2w
**Labels**: `epic`, `feature`, `domain-logistics`, `role-client`, `role-transporteur`, `priority-P2`

### Problème utilisateur
Client veut voir "commande en route, arrivée estimée 14h30". Transporteur loggé via app + position GPS = carte live.

### Valeur business
Réduit appels support "où est ma commande ?" de 70%. Premium UX.

### Critères de succès (SMART)
- [ ] Position GPS transporteur mise à jour toutes les 30s
- [ ] Carte live côté client
- [ ] ETA recalculé dynamiquement (API routing)
- [ ] Notification à 10 min de l'arrivée
- [ ] Consommation data transporteur <10 MB/jour

### Technical approach
- Stack : WebSocket, OpenRouteService pour routing, Leaflet côté client, `logistics-ms` Spring Boot
- Dépendances : EPIC-05 (PWA), EPIC-09 (Géoloc)
- Risks : 3G transporteur instable, coût data, vie privée GPS

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (simulation GPS)
- [ ] Documentation FR (charte transporteur)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `delivery_gps_update_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-28`

---

## EPIC-29 : Calcul frais livraison dynamique

**Priority**: P2 — UX, évite négociations manuelles.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-logistics`, `role-client`, `role-transporteur`, `priority-P2`

### Problème utilisateur
Frais livraison varient selon distance, volume poulets, carburant. Calcul auto au checkout transparent.

### Valeur business
+10% conversion checkout (pas d'abandon dû à surprise frais).

### Critères de succès (SMART)
- [ ] Formule paramétrable (base + distance * tarif_km + volume * tarif_volume)
- [ ] Mise à jour carburant via API gouvernementale (SONABHY)
- [ ] Affichage transparent avant paiement
- [ ] Option "free shipping" si panier > seuil vendeur
- [ ] Comparaison transporteurs partenaires

### Technical approach
- Stack : Service `shipping-ms` Spring Boot, rules engine YAML, cache KAYA prix carburant
- Dépendances : EPIC-09, EPIC-28
- Risks : Volatilité carburant, négociations transporteurs

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright
- [ ] Documentation FR (formule de calcul)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `shipping_quote_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-29`

---

## EPIC-30 : Délégation compte (accès multi-utilisateurs)

**Priority**: P2 — Usage B2B et famille, demande utilisateur modeste.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-auth`, `role-b2b`, `role-famille`, `priority-P2`

### Problème utilisateur
Une famille partage un compte (parents + enfants), un restaurant délègue à plusieurs acheteurs. Sous-comptes avec permissions.

### Valeur business
Facilite adoption familiale + B2B. +10% engagement compte principal.

### Critères de succès (SMART)
- [ ] Création sous-comptes avec rôles (admin, acheteur, lecture)
- [ ] Invitation par SMS + lien d'acceptation
- [ ] Audit log par sous-compte
- [ ] Révocation immédiate
- [ ] Max 5 sous-comptes par compte principal

### Technical approach
- Stack : ORY Keto pour rôles, table `account_delegations` Postgres
- Dépendances : EPIC-02 (SMS), EPIC-06 (KYC)
- Risks : Responsabilité juridique achats sous-comptes, partage secret

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (invitation + révocation)
- [ ] Documentation FR
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `delegation_created_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-30`

---

## EPIC-31 : Parrainage / programme affiliation

**Priority**: P2 — Growth organique, ROI incertain en phase pilote.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-growth`, `role-client`, `priority-P2`

### Problème utilisateur
Utilisateur recommande plateforme à ami → bonus 2000 FCFA pour parrain + 1000 FCFA pour filleul à première commande.

### Valeur business
CAC réduit de 30-50% via viralité. Moteur de croissance organique.

### Critères de succès (SMART)
- [ ] Lien de parrainage unique par user
- [ ] Attribution robuste (device FP + IP + code promo)
- [ ] Bonus crédité après première commande payée
- [ ] Top 10 parrains visibles dans leaderboard
- [ ] Anti-fraude (mêmes devices/IPs bloqués)

### Technical approach
- Stack : Service `referral-ms` Spring Boot, lien court via URL shortener interne
- Dépendances : EPIC-01 (wallet pour crédit bonus), EPIC-11 (anti-fraude)
- Risks : Gaming utilisateurs multi-comptes, coût CAC mal maîtrisé

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright
- [ ] Documentation FR
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `referral_signup_total`, `referral_conversion_rate`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-31`

---

## EPIC-32 : Export RGPD (portabilité données)

**Priority**: P2 — Conformité Loi 010-2004, peu fréquent en pratique.
**Effort**: 1w
**Labels**: `epic`, `feature`, `domain-legal`, `role-all`, `priority-P2`

### Problème utilisateur
Un utilisateur demande export de toutes ses données (commandes, messages, KYC, paiements). Obligation Loi 010-2004 Burkina + bonne pratique.

### Valeur business
Conformité évite amendes CIL Burkina (jusqu'à 100M FCFA). Image de marque trustworthy.

### Critères de succès (SMART)
- [ ] Demande auto depuis profil utilisateur
- [ ] Génération ZIP (JSON + PDF + images) en <24h
- [ ] Lien téléchargement sécurisé 7 jours
- [ ] Effacement compte également disponible (séparé)
- [ ] Log de la demande (audit)

### Technical approach
- Stack : Job Quartz, agrégation cross-services, ZIP via java.util.zip, stockage MinIO temporaire
- Dépendances : Tous services avec PII
- Risks : Volumétrie gros comptes, données éclatées multi-services

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (demande + téléchargement)
- [ ] Documentation FR (procédure RGPD)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `gdpr_export_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-32`

---

## EPIC-33 : Dark mode

**Priority**: P2 — UX/accessibilité, nice-to-have.
**Effort**: 3d
**Labels**: `epic`, `feature`, `domain-ux`, `role-all`, `priority-P2`

### Problème utilisateur
Usage nocturne courant (éleveurs vérifient la nuit), économie batterie sur écrans OLED, préférence personnelle.

### Valeur business
+5% rétention nocturne. Accessibilité (sensibilité lumière).

### Critères de succès (SMART)
- [ ] Toggle light/dark/auto (system) accessible 1 clic
- [ ] Contraste WCAG AA minimum sur les deux thèmes
- [ ] Persistance préférence (localStorage + serveur)
- [ ] 100% des écrans couverts
- [ ] Assets (logos, icônes) adaptés dark

### Technical approach
- Stack : Angular Material theming, CSS variables, media query `prefers-color-scheme`
- Dépendances : Aucune (transverse UI)
- Risks : Effort test visuel cross-écrans, régressions accessibilité

### Definition of Done
- [ ] Code reviewed + merged
- [ ] Tests E2E Playwright (toggle + persistance)
- [ ] Documentation FR (guide UX)
- [ ] i18n FR+Moore+Dioula+Fulfulde
- [ ] Observabilité (metrics `dark_mode_toggle_total`)

### Liens
- Backlog: `docs/BACKLOG-EPICS.md#epic-33`

---

## Résumé priorités

| Priorité | Count | Effort cumulé |
|----------|-------|---------------|
| P0       | 10    | 26 semaines   |
| P1       | 13    | 24 semaines   |
| P2       | 10    | 16.5 semaines |
| **Total**| **33**| **66.5 semaines**|

Avec 4 FTE en parallèle (2 frontend + 1 backend + 1 QA) et ~15% d'overhead (réunions, refactoring, bugs), le roadmap réaliste est ~24 sprints de 2 semaines = 48 semaines calendaires. Le périmètre **pilote 6 mois (12 sprints)** cible les 10 P0 + 8 P1 clés, avec les 10 P2 reportés en v1.2+.

## Hypothèses générales

- **Documentation FR obligatoire** : chaque epic produit minimum 1 guide utilisateur + 1 runbook ops.
- **i18n FR+Mooré+Dioula+Fulfulde** : gabarit ICU, traductions externalisées (cf EPIC-03).
- **Observabilité 3-piliers** : metrics Prometheus + logs Loki + traces Tempo.
- **Tests E2E Playwright** : scénario happy-path + 2 scénarios d'échec minimum.
- **Feature flags** : chaque epic déployé derrière flag GrowthBook (EPIC-21) dès son merge.
- **Conformité** : Loi 010-2004 BF, OHADA, CIL BF, CILSS (sanitaire), RGS ARCEP-BF.
- **Team size assumption** : 4 FTE — 2 frontend Angular 21, 1 backend Spring Boot Java 21, 1 QA Playwright.
