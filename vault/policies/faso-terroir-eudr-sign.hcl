# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Policy Vault TERROIR — terroir-eudr DDS signature
# Permet à terroir-eudr d'émettre/révoquer des certificats EORI exportateur
# pour signer les Due Diligence Statements (EUDR Règlement UE 2023/1115).
#
# Pattern signature DDS :
# 1. terroir-eudr appelle pki-terroir/issue/eori-exporter avec common_name=<exporter-domain>
# 2. Reçoit certificate + private_key (TTL 24h, ne JAMAIS persister la private key au-delà)
# 3. Signe le DDS PDF/JSON avec la clé privée
# 4. Stocke uniquement le certificat (public) + signature dans terroir_t_<slug>.dds
# 5. Audit append-only via terroir_t_<slug>.audit_log

# Issue cert EORI exportateur (le rôle est défini par configure-pki-terroir.sh)
path "pki-terroir/issue/eori-exporter" {
  capabilities = ["update"]
}

# Sign-only : utiliser sign/verify pour les DDS sans émettre de cert (alternative)
path "pki-terroir/sign/eori-exporter" {
  capabilities = ["update"]
}

# Lecture metadata + CA chain pour validation côté acheteur
path "pki-terroir/cert/ca" {
  capabilities = ["read"]
}

path "pki-terroir/cert/ca_chain" {
  capabilities = ["read"]
}

path "pki-terroir/ca/pem" {
  capabilities = ["read"]
}

# Listing certs (pour audit + dashboard expirations)
path "pki-terroir/certs" {
  capabilities = ["list"]
}

path "pki-terroir/cert/+" {
  capabilities = ["read"]
}

# Revocation : terroir-eudr peut révoquer un cert EORI compromis
path "pki-terroir/revoke" {
  capabilities = ["update"]
}

# CRL generation pour distribuer la liste des certs révoqués
path "pki-terroir/crl/rotate" {
  capabilities = ["update"]
}

path "pki-terroir/crl" {
  capabilities = ["read"]
}

# Refus explicite : pas de modification de la CA root/intermediate
path "pki-terroir/root/*" {
  capabilities = ["deny"]
}

path "pki-terroir/intermediate/*" {
  capabilities = ["deny"]
}

path "pki-terroir/config/*" {
  capabilities = ["deny"]
}

path "pki-terroir/roles/*" {
  capabilities = ["deny"]
}
