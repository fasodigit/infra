<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# ADR-005 — Chiffrement des données personnelles (PII)

| Champ | Valeur |
|---|---|
| Statut | Proposé |
| Date | 2026-04-30 |
| Décideurs | Tech lead, RSSI, juriste, DPO |
| Contexte | TERROIR — protection PII producteurs (CNIB, téléphone, photo, biométrie) |

## Contexte

TERROIR collecte des données nominatives sensibles :
- **CNIB / Carte d'identité** (numéro, photo recto-verso, scan) — quasi-fortes
- **Téléphone** (MSISDN) — utilisé pour mobile money, pseudo-identifiant
- **Photo producteur** (face, parfois biométrie)
- **Géolocalisation domicile** (parfois confidentielle)
- **Données biométriques optionnelles** (template empreinte / face) — fortes

Cadre légal :
- Loi n°010-2004/AN (Burkina Faso) — données personnelles
- Convention de Malabo (UA) — cyber-criminalité et données personnelles
- RGPD UE — pertinent dès qu'un acheteur UE traite ces données (transfert)
- EUDR exige géolocalisation parcelle (publique pour la chaîne aval) mais **pas** PII producteur

### Risques
- Fuite massive de CNIB → fraude administrative, marché noir
- Fuite de biométrie → impossible à révoquer (vs mot de passe)
- Fuite GPS domicile en zone à risque sécuritaire (BF nord)

## Options envisagées

### Option A — TLS only, base de données en clair
**Pour** : simple.
**Contre** : tout DBA voit tout, dump base = catastrophe, non-conforme.

### Option B — Disk encryption (LUKS) seul
**Pour** : protège vol physique disque.
**Contre** : ne protège pas dump logique ni accès SQL — insuffisant.

### Option C — Column-level encryption avec pgcrypto + clé statique
**Pour** : chiffrement granulaire.
**Contre** : clé statique → si compromise, tout est exposé ; pas de rotation.

### Option D — pgcrypto + Vault Transit (envelope encryption + rotation)
**Pour** : rotation 90j, audit Vault, séparation clé/donnée.
**Contre** : un peu plus de complexité opérationnelle.

### Option E — Tokenisation externe (CipherCloud, ProtegRity)
**Pour** : isolation forte.
**Contre** : coût $$$, dépendance externe, latence accrue.

### Option F — Chiffrement applicatif (AES-256-GCM côté Rust avec clé Vault)
**Pour** : indépendant DB, peut chiffrer photos S3 aussi.
**Contre** : plus de code à maintenir.

## Décision

**Combinaison D + F (envelope encryption hybride) selon classe de donnée.**

### Classification des PII

| Classe | Donnée | Chiffrement | Lieu |
|---|---|---|---|
| **Quasi-fort** | CNIB numéro, MSISDN, GPS domicile | pgcrypto + Vault Transit | PostgreSQL colonne |
| **Fort** | Biométrie (template), photo CNIB | AES-256-GCM appli + DEK chiffrée par Vault Transit | MinIO + clé chiffrée stockée en PG |
| **Public-après-anonymisation** | Stats agrégées, géoloc parcelle | Aucun (déjà publique pour DDS) | PG + ClickHouse |

### Schéma envelope encryption

```
┌──────────────────────────────────────────────────────┐
│ Vault Transit                                        │
│  - key: terroir-pii-master                           │
│  - rotation: 90 jours, conservation min 5 versions   │
│  - permissions: terroir-core (decrypt only on demand)│
└────────────┬─────────────────────────────────────────┘
             │ DEK encrypt/decrypt
             ▼
┌──────────────────────────────────────────────────────┐
│ Application (terroir-core, terroir-mobile-bff)       │
│  - DEK généré par requête write (AES-256-GCM)        │
│  - DEK encrypté par Vault Transit                    │
│  - DEK_chiffré stocké en PG (colonne `pii_dek`)      │
│  - Données chiffrées avec DEK clair (in-memory only) │
└─────────────┬────────────────────────────────────────┘
              ▼
┌──────────────────────────────────────────────────────┐
│ PostgreSQL                                           │
│  - cnib_encrypted bytea                              │
│  - msisdn_encrypted bytea                            │
│  - pii_dek bytea  (chiffré par Vault)                │
│  - pii_kek_version int  (version clé Vault au moment)│
└──────────────────────────────────────────────────────┘
```

### Photos / scans (volumineux)
- Chiffrement applicatif AES-256-GCM avant upload S3
- DEK par photo, stocké chiffré en PG
- MinIO bucket : object-lock 5 ans + politiques accès strictes
- Pas de re-chiffrement S3-managed (on contrôle nos clés)

### Rotation
- Vault Transit auto-rotate clé toutes les 90 jours
- Re-chiffrement DEK à la lecture si version désuète (lazy migration)
- Job nightly forcé une fois la version dépréciée depuis 30j

### Recherche / index
- Champs chiffrés : pas indexables directement
- Pour MSISDN : ajout colonne `msisdn_hash` (HMAC-SHA256 avec sel partagé Vault) → recherche exacte possible
- Pour CNIB : aucune recherche directe, accès toujours par `member_id`
- Photos : référencées par UUID, jamais par contenu

### Suppression / droit à l'oubli
- Suppression = effacement DEK (cryptoshredding) + tombstone PG
- Données chiffrées restent (irrécupérables sans DEK)
- Métadonnées audit conservées 5 ans (obligation EUDR + comptable)

## Conséquences

### Positives
- Conformité loi BF + RGPD + Convention Malabo
- Compromise DBA seul ≠ exposition (clé Vault séparée)
- Rotation transparente
- Cryptoshredding rapide (RTBF en secondes vs scan lourd)

### Négatives
- Latence read +5-10ms (call Vault unwrap DEK)
- Code applicatif plus complexe (helpers obligatoires, pas de SQL direct)
- Cache local interdit pour PII en clair → recharge à chaque requête

### Mitigations
- Helpers Rust + macro `#[encrypted]` pour réduire la friction
- Cache DEK en mémoire (60s, invalidation rotation)
- Bench latence dès P1 (cible p95 read membre ≤ 100ms)

## Sécurité opérationnelle

- Aucun log applicatif ne contient de PII en clair (filter Loki obligatoire, sanitization tracée en CI)
- Audit Vault Transit (`terroir-pii-master`) hebdomadaire — alerte si > X% reads par utilisateur unique
- Backup PG + MinIO : chiffré avec pgbackrest + clé Vault dédiée (pas la même que pii-master)
- Restore test trimestriel — rotation clé pendant restore = procédure documentée
- Accès aux clés Vault : MFA + approval workflow (PR-based pour modifs policies)

## Conformité

- DPIA rédigée et publiée avant pilote P1 (cf `INFRA/terroir/docs/dpia.md` à venir)
- Consentement éclairé en langue locale (audio enregistré horodaté → MinIO)
- DPO désigné (interne ou externe mutualisé)
- Procédure RTBF documentée (SLA 30j max)
- Notification breach < 72h (loi BF + RGPD)

## Métriques de succès

- 0 PII en clair en logs (audit mensuel)
- Latence read membre p95 ≤ 100 ms (avec décrypt)
- Rotation 90j sans interruption service (mesurée)
- 0 incident sécurité PII année 1

## Révision

À reconfirmer après audit RSSI externe (P1 fin). Si latence Vault Transit gêne → considérer KMS local (Tink ou OpenBao) pour réduire round-trip.
