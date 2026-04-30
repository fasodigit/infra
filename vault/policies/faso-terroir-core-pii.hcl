# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Policy Vault TERROIR — terroir-core PII encryption
# Permet à terroir-core de chiffrer/déchiffrer les champs PII producteurs
# (nom, NIN, téléphone, photo URL, GPS domicile) via Vault Transit envelope
# encryption (KEK terroir-pii-master, DEK per-record AES-256-GCM).
#
# Pattern : auth-ms (ou tout service avec policy KEK) → fetch DEK → encrypt
# côté serveur → ciphertext stocké en DB. À la lecture : fetch DEK → decrypt
# → plaintext en RAM uniquement.
#
# Audit : toute opération transit/encrypt|decrypt loggée dans
# vault audit device file:/vault/audit/transit.log

# Encrypt/decrypt sur la KEK terroir-pii-master uniquement
path "transit/encrypt/terroir-pii-master" {
  capabilities = ["update"]
}

path "transit/decrypt/terroir-pii-master" {
  capabilities = ["update"]
}

# Datakey generation pour DEK envelope
path "transit/datakey/plaintext/terroir-pii-master" {
  capabilities = ["update"]
}

path "transit/datakey/wrapped/terroir-pii-master" {
  capabilities = ["update"]
}

# Lecture metadata clé (version courante, rotation history) — pour
# choisir la bonne version DEK lors d'une lecture historique
path "transit/keys/terroir-pii-master" {
  capabilities = ["read"]
}

# Rewrap au cas où la KEK est rotée et qu'on doit ré-envelopper les DEK
path "transit/rewrap/terroir-pii-master" {
  capabilities = ["update"]
}

# Refus explicite : pas d'export, pas de delete, pas de rotate manuel
# (rotation via auto_rotate_period=2160h configurée dans configure-transit.sh)
path "transit/keys/terroir-pii-master/rotate" {
  capabilities = ["deny"]
}

path "transit/keys/terroir-pii-master/config" {
  capabilities = ["deny"]
}
