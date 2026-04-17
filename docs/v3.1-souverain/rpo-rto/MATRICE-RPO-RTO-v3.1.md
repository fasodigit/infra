# MATRICE RPO/RTO v3.1 — FASO DIGITALISATION

**Version** : 3.1 (souveraine)
**Date** : 2026-04-16
**Statut** : Document contractuel — à faire signer par le métier de chaque sous-projet
**Portée** : 8 sous-projets gouvernementaux du Burkina Faso (ÉTAT-CIVIL, E-TICKET, VOUCHERS, E-SCHOOL, SOGESY, HOSPITAL, ALT-MISSION, FASO-KALAN)

---

## Préambule — Les 3 niveaux de vérité

L'écosystème FASO DIGITALISATION distingue **3 niveaux de vérité** pour toute donnée métier. Le RPO/RTO d'un flux dépend du niveau où réside la donnée authoritative et des exigences de réversibilité.

### Niveau 1 — Vérité opérationnelle : **KAYA** (souverain Rust)

- **Rôle** : cache chaud, sessions, compteurs, Bloom filters, streams temps réel, déduplication idempotente.
- **Durabilité** : configurable par tenant via `persistence.mode` (`sync` = `fsync always`, `async-1s` = `fsync everysec`).
- **Ports** : `6380` (RESP3), `6381` (gRPC).
- **Scripting** : **Rhai** (pas Lua), EVALSHA uniquement en production (EVAL désactivé via ACL).
- **Persistance** : LUKS, rétention configurable, clés gérées via Vault.
- **Réplication** : 3 réplicas asynchrones (hors-site pour les projets RPO=0).

KAYA est **autoritative pour les flux éphémères** (sessions, rate-limiting, déduplication, Bloom) et **cache accélérateur** pour les flux persistants.

### Niveau 2 — Vérité légale : **Redpanda** (RF=3 RAFT synchrone)

- **Rôle** : journal d'événements métier immuable, source of truth pour les événements signés, opposables juridiquement.
- **Durabilité** : RAFT synchrone, `acks=all`, RF=3 sur 3 zones de disponibilité souveraines.
- **Rétention** : 10 ans pour actes d'état civil et dossiers médicaux, 7 ans pour événements comptables vouchers, 3 ans pour logs techniques.
- **Usage** : tout événement métier (acte signé, prescription validée, boarding pass émis, paiement confirmé) est publié ici avant tout accusé de réception métier.

### Niveau 3 — Vérité durable : **YugabyteDB** (SQL distribué)

- **Rôle** : vue matérialisée durable, requêtes analytiques, reconstitution d'état, restauration après sinistre.
- **Durabilité** : RAFT synchrone, réplication multi-zone, backups chiffrés quotidiens.
- **Usage** : reconstruction de l'état KAYA après incident via replay Redpanda → projection vers Yugabyte → warmup KAYA.

### Règle d'or

> Un flux **RPO=0** exige que la donnée soit **acquittée en niveau 2 (Redpanda RAFT)** avant ACK métier, ET que la copie KAYA soit en `fsync always`.
> Un flux **RPO≤1s** tolère un acquittement uniquement niveau 1 (KAYA `fsync everysec`), avec propagation asynchrone vers niveaux 2/3.
> Un flux **RPO≤5s** (interne, re-soumissible) tolère une perte de 5 secondes de cache KAYA avec reconstruction depuis le niveau 3.

---

## ENCADRÉ CRITIQUE

> **Ce tableau doit être validé et signé par le métier de chaque projet.**
>
> Le choix `fsync always` vs `everysec` découle de cette décision métier, **pas l'inverse**.
>
> Toute demande technique d'assouplissement (ex: passer un flux de `always` à `everysec` pour des raisons de performance) doit faire l'objet d'un **avenant signé** par le signataire métier référencé ci-dessous et archivé avec la présente matrice.
>
> Tout flux non listé dans la matrice est par défaut classé **RPO≤5s / fsync everysec**. Toute montée en criticité ultérieure exige une réévaluation formelle.

---

## Classification des flux

| Classe | RPO | Persistance KAYA | Acquittement Redpanda | Caractéristiques |
|---|---|---|---|---|
| **A — Signé / légalement opposable** | 0 s | `fsync always` | RF=3 RAFT synchrone, `acks=all` | Acte administratif produisant des effets de droit, non re-soumissible, rétention légale longue |
| **B — Transactionnel re-soumissible** | ≤ 1 s | `fsync everysec` | RF=3 RAFT, `acks=all` (asynchrone) | Donnée métier importante mais l'utilisateur peut retenter l'opération sans préjudice |
| **C — Interne / cache / analytique** | ≤ 5 s | `fsync everysec` | Asynchrone best-effort | Reconstructible depuis niveau 3, perte acceptable en cas de crash |

---

## Matrice détaillée par sous-projet

### 1. ÉTAT-CIVIL

**Signataire métier attendu** : Directeur Général de la Modernisation de l'État Civil (DGMEC), Ministère de l'Administration Territoriale.

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Acte de naissance signé électroniquement | A | 0 s | 15 min | `fsync always` | Acte authentique produisant des effets de droit (nationalité, filiation). Obligation **Code des personnes et de la famille BF**. Rétention **10 ans minimum** (archives nationales). Non re-soumissible après signature. |
| Acte de mariage signé | A | 0 s | 15 min | `fsync always` | Acte authentique, effets de droit sur état civil des époux. Même régime légal. |
| Acte de décès signé | A | 0 s | 15 min | `fsync always` | Acte authentique, déclenche succession. Obligation légale BF. |
| Demande en cours (brouillon citoyen) | B | ≤ 1 s | 15 min | `fsync everysec` | Re-soumissible par le citoyen via son espace, non encore signée. |
| Session utilisateur agent | C | ≤ 5 s | 15 min | `fsync everysec` | Session reconstructible via re-login. |
| Compteur rate-limit dépôts | C | ≤ 5 s | 15 min | `fsync everysec` | Reconstitution acceptable en quelques secondes. |

**Obligations légales Burkina Faso** :
- Code des personnes et de la famille — authenticité et intégrité des actes.
- Archives nationales — rétention **10 ans** minimum pour les actes d'état civil (cible projet : 10 ans en ligne + 50 ans archivage froid chiffré).
- Loi n° 010-2004/AN portant protection des données à caractère personnel — CIL BF.

**Stratégie de reprise (RTO 15 min)** :
1. Bascule automatique du leader KAYA via xDS Controller (< 30 s).
2. Rejeu des 5 dernières minutes d'événements Redpanda sur le nouveau leader.
3. Reconstruction de l'index Bloom `bf:actes:signes` depuis Yugabyte si nécessaire.

---

### 2. HOSPITAL

**Signataire métier attendu** : Secrétaire Général du Ministère de la Santé + Directeur du Système d'Information Hospitalier (DSIH).

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Prescription médicamenteuse signée | A | 0 s | 5 min | `fsync always` | Acte médical engageant la responsabilité du prescripteur, opposable. Perte = risque vital (double prescription, rupture de traitement). Obligation **secret médical et traçabilité**. |
| Ordonnance hospitalière validée | A | 0 s | 5 min | `fsync always` | Même régime que prescription. |
| Résultat d'examen validé par biologiste | A | 0 s | 5 min | `fsync always` | Acte diagnostique opposable, effets cliniques immédiats. |
| Admission / pré-admission patient | B | ≤ 1 s | 5 min | `fsync everysec` | Re-saisissable, mais doit être rapide pour continuité des soins. |
| Réservation de lit | B | ≤ 1 s | 5 min | `fsync everysec` | Re-soumissible, perte ≤ 1 s tolérée. |
| Session soignant | C | ≤ 5 s | 5 min | `fsync everysec` | Re-login rapide. |
| File d'attente consultations externes | C | ≤ 5 s | 5 min | `fsync everysec` | Reconstituable depuis triage papier/badge. |

**Obligations légales Burkina Faso** :
- Code de déontologie médicale BF — traçabilité des actes et prescriptions.
- Rétention dossier médical : **10 ans minimum** après dernière consultation (20 ans pour mineurs jusqu'à majorité + 10 ans).
- Secret médical — chiffrement at-rest (LUKS) + chiffrement en transit (mTLS via SPIRE).

**Stratégie de reprise (RTO 5 min)** :
1. Failover KAYA vers réplica hors-site (bascule < 30 s).
2. Replay Redpanda des 2 dernières minutes pour garantir zéro perte de prescription.
3. Reconstitution de l'index Bloom `bf:prescriptions:actives` et `bf:admissions:jour`.
4. Alerting spécifique au DSIH avec SLA humain de 2 min pour validation bascule.

---

### 3. E-TICKET

**Signataire métier attendu** : Directeur Général des Transports Terrestres + représentant SOTRACO / opérateurs partenaires.

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Réservation de billet confirmée | B | ≤ 1 s | 30 min | `fsync everysec` | Re-soumissible par l'utilisateur. Paiement et billet sont deux événements séparés. |
| Validation de ticket à l'embarquement (scan QR) | B | ≤ 1 s | 30 min | `fsync everysec` | Double-scan prévenu par Bloom filter et déduplication idempotente (clé = ticket_id). |
| Déduplication scans (Bloom `bf:tickets:scanned`) | B | ≤ 1 s | 30 min | `fsync everysec` | Perte ≤ 1 s acceptable (tolérance double-scan marginale, vérifiable à posteriori). |
| Panier d'achat en cours | C | ≤ 5 s | 30 min | `fsync everysec` | Re-soumissible par l'utilisateur. |

**Obligations légales Burkina Faso** :
- Loi de finances et droit fiscal BF — conservation billetterie **5 ans** (recoupement fiscal).
- Protection des données personnelles — CIL BF.

**Stratégie de reprise (RTO 30 min)** :
1. Bascule KAYA standard (< 60 s).
2. Replay Redpanda pour reconstituer les scans manquants.
3. Procédure dégradée : validation manuelle (PIN SMS) si KAYA indisponible > 5 min.

---

### 4. VOUCHERS

**Signataire métier attendu** : Directeur Général du Trésor Public + Agence de Promotion des Petites et Moyennes Entreprises (APME).

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Paiement voucher confirmé / débit wallet | A | 0 s | 15 min | `fsync always` | Transaction financière opposable. Obligation **comptable et fiscale BF**. Perte = double débit ou perte de fonds. |
| Émission voucher signé | A | 0 s | 15 min | `fsync always` | Acte financier produisant des effets de droit. Non re-soumissible. |
| Annulation / remboursement | A | 0 s | 15 min | `fsync always` | Transaction financière opposable. |
| Consultation solde / historique | B | ≤ 1 s | 15 min | `fsync everysec` | Lecture re-demandable. |
| Recherche voucher par code | B | ≤ 1 s | 15 min | `fsync everysec` | Lecture re-demandable. |
| Session utilisateur | C | ≤ 5 s | 15 min | `fsync everysec` | Re-login. |

**Obligations légales Burkina Faso** :
- Code général des impôts BF et OHADA — rétention **comptable 10 ans** (prudence, OHADA art. 24 AUDCIF). Cible projet : **10 ans en ligne**.
- Loi anti-blanchiment CENTIF — traçabilité transactions.
- Normes UEMOA paiement électronique.

**Stratégie de reprise (RTO 15 min)** :
1. Bascule KAYA avec `fsync always` vérifié sur réplica (pas de perte).
2. Réconciliation avec Redpanda + YugabyteDB en cas de doute sur un débit.
3. Freeze des émissions pendant 5 min si divergence détectée (circuit-breaker).

---

### 5. E-SCHOOL

**Signataire métier attendu** : Secrétaire Général du Ministère de l'Éducation Nationale, de l'Alphabétisation et de la Promotion des Langues Nationales.

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Inscription / réinscription élève | B | ≤ 1 s | 1 h | `fsync everysec` | Re-soumissible par l'établissement. |
| Dépôt de notes / bulletins | B | ≤ 1 s | 1 h | `fsync everysec` | Re-saisissable par l'enseignant ; vérité légale = bulletin signé archivé Redpanda. |
| Affectation / orientation | B | ≤ 1 s | 1 h | `fsync everysec` | Re-soumissible, processus administratif. |
| Session utilisateur | C | ≤ 5 s | 1 h | `fsync everysec` | Re-login. |

**Obligations légales Burkina Faso** :
- Rétention des dossiers scolaires : **10 ans** après sortie de l'établissement.
- Archivage bulletins et diplômes : permanent (via Redpanda compact topic + archivage froid).

**Stratégie de reprise (RTO 1 h)** :
1. Bascule KAYA standard (< 2 min).
2. Replay Redpanda.
3. Procédure dégradée : saisie offline des notes (formulaire papier scanné + re-import batch).

---

### 6. SOGESY

**Signataire métier attendu** : Directeur Général de l'Aviation Civile + opérateur aéroportuaire ADB.

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Émission boarding pass signé | A | 0 s | 15 min | `fsync always` | Titre de transport opposable, contrôle sûreté aérienne. Obligation **OACI**. Non re-soumissible après émission. |
| Check-in validé | A | 0 s | 15 min | `fsync always` | Engage responsabilité opérateur (liste passagers, sûreté). |
| Déduplication scans embarquement | B | ≤ 1 s | 15 min | `fsync everysec` | Double-scan prévenu par Bloom `bf:boarding:scanned`. |
| Gestion des bagages (suivi) | B | ≤ 1 s | 15 min | `fsync everysec` | Re-synchronisable avec scan physique. |
| File d'attente comptoir | C | ≤ 5 s | 15 min | `fsync everysec` | Reconstituable. |

**Obligations légales Burkina Faso** :
- Annexe 17 OACI — sûreté aérienne, traçabilité passagers.
- Code de l'aviation civile BF.
- Rétention manifestes : **5 ans** minimum.

**Stratégie de reprise (RTO 15 min)** :
1. Bascule KAYA vers réplica hors-site aéroport.
2. Replay Redpanda pour reconstituer boarding passes émis.
3. Procédure dégradée sûreté : émission manuelle papier avec numéro de secours pré-provisionné (carnet offline signé).

---

### 7. ALT-MISSION

**Signataire métier attendu** : Directeur des Ressources Humaines de l'administration publique (fonction publique BF).

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Soumission ordre de mission | C | ≤ 5 s | 4 h | `fsync everysec` | Processus administratif interne, re-soumissible. |
| Validation hiérarchique | C | ≤ 5 s | 4 h | `fsync everysec` | Re-soumissible via workflow. |
| Décompte indemnités | C | ≤ 5 s | 4 h | `fsync everysec` | Calcul re-exécutable. |

**Obligations légales Burkina Faso** :
- Statut général de la fonction publique BF.
- Rétention pièces justificatives mission : **5 ans** (contrôle Cour des Comptes).

**Stratégie de reprise (RTO 4 h)** :
- Bascule standard, pas d'urgence opérationnelle.

---

### 8. FASO-KALAN

**Signataire métier attendu** : Directeur du projet FASO-KALAN (plateforme de formation / apprentissage).

| Flux | Classe | RPO | RTO | Persistance KAYA | Justification |
|---|---|---|---|---|---|
| Progression apprenant | C | ≤ 5 s | 4 h | `fsync everysec` | Re-calculable depuis événements d'activité. |
| Quiz / évaluation intermédiaire | C | ≤ 5 s | 4 h | `fsync everysec` | Re-soumissible. |
| Session apprenant | C | ≤ 5 s | 4 h | `fsync everysec` | Re-login. |

**Obligations légales Burkina Faso** :
- Rétention certificats délivrés : **10 ans** (via Redpanda compact topic, pas KAYA).

**Stratégie de reprise (RTO 4 h)** :
- Bascule standard.

---

## Section — Tests de validation (Chaos Engineering)

Les RPO/RTO contractuels de cette matrice doivent être **prouvés périodiquement** par une suite de scénarios chaos. Les tests suivants sont exécutés **mensuellement** en environnement de pré-production miroir, et **trimestriellement** en production (fenêtre planifiée).

### Scénarios chaos obligatoires

| ID | Scénario | Flux cible | Critère de succès |
|---|---|---|---|
| **CHAOS-001** | Kill brutal du leader KAYA pendant un flux A (fsync always) | ÉTAT-CIVIL signature, HOSPITAL prescription, VOUCHERS paiement, SOGESY boarding | Zéro perte de donnée acquittée au client — 100 % des événements ACKés présents dans Redpanda après reprise |
| **CHAOS-002** | Kill brutal du leader KAYA pendant un flux B (fsync everysec) | Admission, e-ticket, e-school, consultation voucher | Perte ≤ 1 seconde d'événements — vérifié par compteur d'écart |
| **CHAOS-003** | Partition réseau entre KAYA et Redpanda (5 min) | Tous projets classe A | Le service refuse les écritures classe A (mode read-only durcis) plutôt que d'acquitter sans persistance niveau 2 |
| **CHAOS-004** | Perte complète d'une zone de disponibilité | Tous projets | RTO respecté : ÉTAT-CIVIL/VOUCHERS/SOGESY ≤ 15 min, HOSPITAL ≤ 5 min, E-TICKET ≤ 30 min, E-SCHOOL ≤ 1 h, ALT-MISSION/FASO-KALAN ≤ 4 h |
| **CHAOS-005** | Corruption du snapshot KAYA | Tous projets | Reconstruction depuis Redpanda + YugabyteDB dans le RTO cible |
| **CHAOS-006** | Perte d'un réplica (3 → 2) puis d'un second (2 → 1) | Projets RPO=0 | Mode dégradé read-only déclenché automatiquement, alerte opérateur < 1 min |
| **CHAOS-007** | Rotation de clé LUKS en charge | Tous projets | Zéro interruption de service, zéro perte de donnée |
| **CHAOS-008** | Latence artificielle 500 ms sur fsync (disque dégradé) | Projets `fsync always` | Alerting P99 dépassé < 30 s, bascule proactive vers réplica sain |

### Livrable de validation

Chaque exécution produit un rapport signé par :
- Ingénieur SRE exécutant (FASO DIGITALISATION)
- Signataire métier du projet testé
- Archivé 10 ans avec la matrice

Un test en échec **bloque toute mise en production** des déploiements du sous-projet concerné jusqu'à remédiation.

---

## Signatures

| Sous-projet | Signataire métier | Nom | Date | Signature |
|---|---|---|---|---|
| ÉTAT-CIVIL | DGMEC / MATD | _________________ | ___/___/___ | _________ |
| HOSPITAL | SG MinSanté / DSIH | _________________ | ___/___/___ | _________ |
| E-TICKET | DGTT | _________________ | ___/___/___ | _________ |
| VOUCHERS | DG Trésor / APME | _________________ | ___/___/___ | _________ |
| E-SCHOOL | SG MENAPLN | _________________ | ___/___/___ | _________ |
| SOGESY | DG Aviation Civile | _________________ | ___/___/___ | _________ |
| ALT-MISSION | DRH Fonction Publique | _________________ | ___/___/___ | _________ |
| FASO-KALAN | Directeur FASO-KALAN | _________________ | ___/___/___ | _________ |

**Contresignataires techniques** :
- Directeur FASO DIGITALISATION : _________________
- RSSI FASO DIGITALISATION : _________________
- Architecte en chef (plateforme souveraine) : _________________

---

*Document versionné ; toute modification de RPO/RTO exige avenant signé. Version suivante v3.2 prévue après premier cycle annuel chaos (Q2 2027).*
