# TERROIR — Schemas Avro P0

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Convention de nommage

- Namespace : `bf.faso.terroir.events.v1`
- Format complet : `bf.faso.terroir.events.v1.<RecordName>`
- Noms de fichiers : `terroir_<domaine>_<action>.avsc` (snake_case)
- Version embarquée dans le namespace (`v1`) — un bump de version majeure
  implique un nouveau namespace (`v2`) et un nouveau sujet Schema Registry.

## Compatibilité Schema Registry

Stratégie : `BACKWARD`

Les consommateurs compilés contre un schema `v1` doivent pouvoir lire les
messages produits par des producteurs utilisant un schema `v1+` (évolution
additive uniquement) :

- Ajouter un champ : TOUJOURS avec `"default": null` (ou valeur métier
  sensée) pour rester BACKWARD-compatible.
- Supprimer un champ : INTERDIT en P0/P1 — créer un nouveau namespace v2.
- Renommer un champ : INTERDIT — utiliser `aliases` Avro + deprecation.
- Modifier le type d'un champ : INTERDIT sauf promotion (int → long, float
  → double).

Configuration Schema Registry (Redpanda) :

```bash
# Définir la compatibilité BACKWARD pour le sujet
rpk registry schema set-compatibility \
  --subject "terroir.member.created-value" \
  BACKWARD
```

## Schemas P0 (livrés)

| Fichier | Record | Topic | Retention |
|---------|--------|-------|-----------|
| `terroir_member_created.avsc` | `MemberCreated` | `terroir.member.created` | 90 j |
| `terroir_parcel_eudr_validated.avsc` | `ParcelEudrValidated` | `terroir.parcel.eudr.validated` | 1 an |
| `terroir_dds_submitted.avsc` | `DdsSubmitted` | `terroir.dds.submitted` | 7 ans |
| `terroir_tenant_provisioned.avsc` | `TenantProvisioned` | `terroir.tenant.provisioned` | 1 an |
| `terroir_payment_completed.avsc` | `PaymentCompleted` | `terroir.payment.completed` | 1 an |
| `terroir_harvest_lot_recorded.avsc` | `HarvestLotRecorded` | `terroir.harvest.lot.recorded` | 90 j |
| `terroir_ussd_otp_sent.avsc` | `UssdOtpSent` | `terroir.ussd.otp.sent` | 7 j |
| `terroir_audit_event.avsc` | `AuditEvent` | `terroir.audit.event` | 7 ans |

## Schemas P1 (planifiés)

| Record prévu | Topic | Justification |
|--------------|-------|---------------|
| `MemberUpdated` | `terroir.member.updated` | Audit modifications identité producteur |
| `MemberDeleted` | `terroir.member.deleted` | RGPD erasure + audit |
| `ParcelCreated` | `terroir.parcel.created` | Tracabilité ajout parcelle au registre |
| `ParcelUpdated` | `terroir.parcel.updated` | Modifications GPS/surface |
| `ParcelEudrRejected` | `terroir.parcel.eudr.rejected` | Cas échec validation EUDR |
| `PaymentInitiated` | `terroir.payment.initiated` | Début workflow paiement |
| `PaymentFailed` | `terroir.payment.failed` | Échec paiement Mobile Money |
| `DdsGenerated` | `terroir.dds.generated` | DDS générée avant soumission |
| `DdsRejected` | `terroir.dds.rejected` | DDS refusée par TRACES NT |
| `UssdSessionStarted` | `terroir.ussd.session.started` | Début session USSD |
| `UssdSessionEnded` | `terroir.ussd.session.ended` | Fin session USSD |
| `UssdOtpVerified` | `terroir.ussd.otp.verified` | Vérification OTP USSD |

## Schemas P2 (évaluation future)

| Record prévu | Topic | Justification |
|--------------|-------|---------------|
| `SyncConflictDetected` | `terroir.sync.conflict.detected` | CDC KAYA <-> YugabyteDB conflict |
| `SyncConflictResolved` | `terroir.sync.conflict.resolved` | Résolution conflit CRDT/LWW |

## Enregistrement dans Schema Registry

```bash
# Enregistrer un schema (exemple MemberCreated)
curl -s -X POST \
  http://localhost:8081/subjects/terroir.member.created-value/versions \
  -H "Content-Type: application/vnd.schemaregistry.v1+json" \
  -d "{\"schema\": $(cat terroir_member_created.avsc | jq -Rs .)}"

# Vérifier la compatibilité avant publication
curl -s -X POST \
  http://localhost:8081/compatibility/subjects/terroir.member.created-value/versions/latest \
  -H "Content-Type: application/vnd.schemaregistry.v1+json" \
  -d "{\"schema\": $(cat terroir_member_created.avsc | jq -Rs .)}"
```

## Validation JSON locale

```bash
python3 -c "
import json, sys, pathlib
schemas = list(pathlib.Path('.').glob('*.avsc'))
errors = []
for f in schemas:
    try:
        json.load(open(f))
        print(f'  OK {f.name}')
    except Exception as e:
        errors.append(f'{f.name}: {e}')
        print(f'  FAIL {f.name}: {e}', file=sys.stderr)
if errors:
    sys.exit(1)
print(f'All {len(schemas)} schemas valid.')
"
```

## Types logiques utilisés

| Type Avro | Logique | Usage |
|-----------|---------|-------|
| `{"type":"long","logicalType":"timestamp-millis"}` | Timestamp UTC epoch ms | Tous les champs `*At` |
| `{"type":"enum","symbols":[...]}` | Enum inline | Statuts, canaux, grades |
| `{"type":"array","items":"string"}` | Liste | `parcelIds`, `complianceFlags` |
| `["null","<type>"]` avec `"default":null` | Champ optionnel | Champs conditionnels |
