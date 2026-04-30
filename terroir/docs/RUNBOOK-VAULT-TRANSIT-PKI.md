<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# RUNBOOK — Vault Transit + PKI TERROIR (P0.B)

| Champ | Valeur |
|---|---|
| Phase | P0.B (independant de P0.A) |
| Mounts | `transit/`, `pki-terroir/` |
| Cles cibles | `terroir-pii-master`, `terroir-dek-master` |
| Role PKI | `eori-exporter` |
| Refs | ADR-005, ULTRAPLAN §4 P0.2 + §12 |

## 1. Bootstrap (operateur, premiere fois)

```bash
# Pre-requis : Vault unsealed, ~/.faso-vault-keys.json present
cd INFRA
podman-compose -f vault/podman-compose.vault.yml up -d
bash vault/scripts/init.sh                       # idempotent
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)

# TERROIR P0.B
bash vault/scripts/configure-transit.sh          # transit + 2 cles + audit
bash vault/scripts/configure-pki-terroir.sh      # intermediate CA + role EORI
bash vault/scripts/seed-admin-secrets.sh         # secrets admin + faso/terroir/*
```

Verifications attendues :

```bash
vault list transit/keys                # terroir-pii-master, terroir-dek-master, jwt-key, pii-key, persistence-key
vault list pki-terroir/roles           # eori-exporter
vault kv list faso/terroir             # 4 entrees
```

## 2. Rotation manuelle

```bash
# Rotation PII KEK (toutes les ecritures futures utilisent la nouvelle version,
# les anciennes versions restent valides en decryption tant que min_decryption_version
# n'est pas remontee)
vault write -f transit/keys/terroir-pii-master/rotate
vault read transit/keys/terroir-pii-master            # latest_version++

# Une fois tous les payloads re-chiffres (job nightly lazy migration):
vault write transit/keys/terroir-pii-master/config min_decryption_version=N
```

Auto-rotation : 90 jours (`auto_rotate_period=2160h`) — set par `configure-transit.sh`.

## 3. Revocation cert EORI exportateur

```bash
# 1) Lister les certs emis (par le mount, pas par role)
vault list pki-terroir/certs

# 2) Revoke par serial_number (recupere a l'emission ou via vault read pki-terroir/cert/<serial>)
vault write pki-terroir/revoke serial_number="<aa:bb:cc:...>"

# 3) Forcer la generation d'une nouvelle CRL et publier
vault read -field=crl pki-terroir/cert/crl > /tmp/terroir-crl.pem
# -> deposer /tmp/terroir-crl.pem sur ARMAGEDDON config/CRL distribution
```

## 4. Compromission CA (procedure d'urgence)

1. **Isolation** : revoke immediat de la chaine intermediate
   `vault write pki/root/revoke serial_number=<intermediate-serial>`.
2. **Communication** : email DPO + RSSI + ANSSI-BF + acheteurs UE (transferts EUDR pendants),
   delai legal < 72h (loi BF + RGPD art. 33).
3. **Audit** : extraction logs `/vault/audit/transit.log` + `/vault/logs/audit.log` sur
   les 90 derniers jours, hash SHA-256 puis archive WORM (MinIO bucket `audit-immutable`).
4. **Re-issue** :
   - re-run `configure-pki-terroir.sh` (qui re-genere CSR + sign-intermediate
     si `pki-terroir/cert/ca` retourne vide — sinon vider d'abord le mount :
     `vault secrets disable pki-terroir && vault secrets enable -path=pki-terroir pki`).
   - re-emettre tous les certs EORI actifs avec la nouvelle CA.
5. **Postmortem** : ADR-005 maj + DPIA refresh + retrospective dans `docs/incidents/`.

## 5. Mappings PII -> context Transit

Le contexte Transit (HKDF derivation) **DOIT** etre fourni a chaque
encrypt/decrypt — sinon Vault renverra `400 Bad Request` (la cle
`terroir-pii-master` est `derived=true`). Format conventionnel :
`tenant=<tenant_slug>|field=<field_name>`.

| Champ PII | Context (base64 du string) | Stockage cipher |
|---|---|---|
| `producer.cnib_number` | `tenant=t_pilot\|field=nin` | `cnib_encrypted bytea` |
| `producer.msisdn` | `tenant=t_pilot\|field=msisdn` | `msisdn_encrypted bytea` |
| `producer.gps_home` | `tenant=t_pilot\|field=gps_home` | `gps_home_encrypted bytea` |
| `producer.email` | `tenant=t_pilot\|field=email` | `email_encrypted bytea` |
| `producer.photo_id_dek` | `tenant=t_pilot\|field=photo_dek` | `pii_dek bytea` (wrapped) |
| `producer.biometric_template_dek` | `tenant=t_pilot\|field=bio_dek` | `bio_dek bytea` (wrapped) |

> Hash deterministe (recherche exacte msisdn) : utiliser `transit/hmac/terroir-pii-master`
> avec le meme contexte ou un sel separe — JAMAIS le ciphertext (non deterministe par design).

## 6. Pattern KEK / DEK envelope

```
                       +---------------------------+
                       | Vault Transit             |
                       |  KEK terroir-pii-master   | (rotation 90j auto)
                       |  KEK terroir-dek-master   |
                       +-----------+---------------+
                                   |
            wrap DEK (per-record)  |   unwrap DEK (read path)
                                   |
                                   v
+----------------------------------+----------------------------------+
| Application terroir-core / mobile-bff (Rust)                        |
|                                                                     |
|  write path:                                                        |
|    1) DEK = openssl rand 32 bytes        (in-memory only)           |
|    2) cipher = AES-256-GCM(DEK, plaintext, AAD=tenant|field|id)     |
|    3) wrapped_dek = Vault transit/encrypt/terroir-dek-master(DEK)   |
|    4) PG row: { cipher, wrapped_dek, kek_version }                  |
|    5) zeroize DEK in memory                                         |
|                                                                     |
|  read path:                                                         |
|    1) load row -> { cipher, wrapped_dek, kek_version }              |
|    2) DEK = Vault transit/decrypt/terroir-dek-master(wrapped_dek)   |
|    3) plaintext = AES-256-GCM-open(DEK, cipher, AAD)                |
|    4) zeroize DEK after use                                         |
+---------------------------------------------------------------------+
                                   |
                                   v
                       +---------------------------+
                       | PostgreSQL (par tenant)   |
                       |  pii_cipher       bytea   |
                       |  pii_dek_wrapped  bytea   |
                       |  pii_kek_version  int     |
                       +---------------------------+
```

Avantages :
- **Performance** : AES-GCM local, Vault appele 1x par requete (DEK cache KAYA TTL 1h en P1).
- **Cryptoshredding RTBF** : effacer la ligne -> DEK perdu -> ciphertext irrecuperable.
- **Rotation KEK** : re-wrap des DEK (lazy migration), pas de re-chiffrement des donnees volumineuses.
- **Compromission DBA** : ciphertext + wrapped_dek seuls -> inutilisable sans Vault.

## 7. Audit & metriques

- Audit transit : `/vault/audit/transit.log` (active par `configure-transit.sh`).
- Metric Loki Recommandee : `count by (operation) (vault_audit_log{path=~"transit/encrypt/.*|transit/decrypt/.*"})`.
- Alerte > 10k decrypt/h par token unique (potentielle exfiltration) — pipeline a definir P1.
- Backups : sealed Raft snapshot quotidien + restore test trimestriel.

## 8. Liens utiles

- ADR-005 (PII encryption) — `INFRA/terroir/docs/adr/ADR-005-pii-encryption.md`
- ULTRAPLAN §4 P0.2 + §12 — `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md`
- Vault Transit doc — https://developer.hashicorp.com/vault/docs/secrets/transit
- Vault PKI doc — https://developer.hashicorp.com/vault/docs/secrets/pki
