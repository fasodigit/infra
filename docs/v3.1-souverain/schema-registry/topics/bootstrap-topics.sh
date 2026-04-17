#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# bootstrap-topics.sh - FASO DIGITALISATION sovereign Redpanda cluster
# -----------------------------------------------------------------------------
# Creates (idempotently) every durable topic defined in topics-retention.yaml.
# Requires: rpk configured with profile pointing at the Redpanda brokers.
# Usage:    ./bootstrap-topics.sh [--dry-run]
# -----------------------------------------------------------------------------
set -euo pipefail

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then DRY_RUN=1; fi

create_topic () {
  local name="$1" partitions="$2" retention_ms="$3" segment_ms="$4" cleanup="$5"
  local cmd=(rpk topic create "${name}"
    --partitions "${partitions}"
    --replicas 3
    --config "cleanup.policy=${cleanup}"
    --config "retention.ms=${retention_ms}"
    --config "segment.ms=${segment_ms}"
    --config "compression.type=zstd"
    --config "min.insync.replicas=2")
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    printf 'DRY-RUN: %s\n' "${cmd[*]}"
  else
    "${cmd[@]}" || echo "Topic ${name} already exists (skipping)"
  fi
}

# ETAT-CIVIL --------------------------------------------------------------
create_topic ec.demande.created.v1            12 157680000000 604800000   delete
create_topic ec.demande.validated.v1          12 157680000000 604800000   delete
create_topic ec.acte.signed.v1                12 157680000000 604800000   delete
create_topic ec.acte.state.v1                 12 -1           86400000    compact,delete
create_topic ec.audit-trail.v1                24 315360000000 2592000000  delete

# HOSPITAL ----------------------------------------------------------------
create_topic hosp.admission.registered.v1     12 315360000000 604800000   delete
create_topic hosp.prescription.validated.v1   12 315360000000 604800000   delete
create_topic hosp.dossier.sealed.v1           12 315360000000 604800000   delete
create_topic hosp.dossier.state.v1            12 315360000000 86400000    compact,delete
create_topic hosp.audit-trail.v1              24 315360000000 2592000000  delete

# E-TICKET ----------------------------------------------------------------
create_topic eticket.ticket.purchased.v1      24 7776000000   86400000    delete
create_topic eticket.ticket.validated.v1      24 7776000000   86400000    delete
create_topic eticket.seat.reserved.v1         12 7776000000   86400000    delete
create_topic eticket.audit-trail.v1           12 315360000000 2592000000  delete

# VOUCHERS ----------------------------------------------------------------
create_topic vouchers.voucher.emitted.v1      12 94608000000  604800000   delete
create_topic vouchers.voucher.consumed.v1     12 94608000000  604800000   delete
create_topic vouchers.transaction.confirmed.v1 24 220752000000 2592000000 delete
create_topic vouchers.audit-trail.v1          24 315360000000 2592000000  delete

# SOGESY ------------------------------------------------------------------
create_topic sogesy.boarding-pass.issued.v1   12 31536000000  604800000   delete
create_topic sogesy.boarding-pass.scanned.v1  12 31536000000  604800000   delete
create_topic sogesy.boarding-pass.sealed.v1    6 31536000000  604800000   delete
create_topic sogesy.audit-trail.v1            12 315360000000 2592000000  delete

# E-SCHOOL ----------------------------------------------------------------
create_topic eschool.inscription.created.v1   12 94608000000  604800000   delete
create_topic eschool.inscription.validated.v1 12 94608000000  604800000   delete
create_topic eschool.quota.classroom-full.v1   6 94608000000  604800000   delete
create_topic eschool.audit-trail.v1           12 315360000000 2592000000  delete

# ALT-MISSION -------------------------------------------------------------
create_topic alt.mission.created.v1            6 63072000000  604800000   delete
create_topic alt.mission.approved.v1           6 63072000000  604800000   delete
create_topic alt.mission.completed.v1          6 63072000000  604800000   delete
create_topic alt.audit-trail.v1                6 315360000000 2592000000  delete

# FASO-KALAN --------------------------------------------------------------
create_topic kalan.session.started.v1         12 31536000000  604800000   delete
create_topic kalan.session.completed.v1       12 31536000000  604800000   delete
create_topic kalan.certificate.issued.v1       6 31536000000  604800000   delete
create_topic kalan.audit-trail.v1              6 315360000000 2592000000  delete

echo "Bootstrap finished."
