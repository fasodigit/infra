<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# TERROIR — gRPC schemas

Schémas Protocol Buffers du module TERROIR.

## Fichiers

| Schema | Service | Phase | Notes |
|--------|---------|-------|-------|
| `core.proto` | `terroir.core.v1.CoreService` | P0 squelette / P1.1 implémentation | Registre membres + parcelles + ménages |
| `eudr.proto` | `terroir.eudr.v1.EudrService` | P0 squelette / P1.3 implémentation | Validation Hansen GFC + JRC TMF, génération DDS |

## Politique d'évolution

- Versionnage par package (`v1`, `v2`, …) — jamais de breaking change dans une version stable.
- Champs `reserved` impérativement renseignés en cas de suppression.
- Les schemas Avro Redpanda associés sont dans `INFRA/redpanda/schemas/terroir/*.avsc` (livrable P0.E).

## Build

Les services Rust compilent les `.proto` via `tonic-build` dans leur `build.rs` (à ajouter en P1).
Le crate `prost` (workspace.dependencies) fournit l'encodage runtime.

## Références

- ULTRAPLAN P0.A : `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §4.
- Port-policy mesh : `INFRA/port-policy.yaml` (8730-8749 = `agri-services-grpc`).
