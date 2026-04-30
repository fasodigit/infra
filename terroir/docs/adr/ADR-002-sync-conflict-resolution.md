<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# ADR-002 — Stratégie de synchronisation et résolution de conflits

| Champ | Valeur |
|---|---|
| Statut | Proposé |
| Date | 2026-04-30 |
| Décideurs | Tech lead, agronome SME, juriste |
| Contexte | Sync app agent terrain ↔ backend après 7-14j hors-ligne |

## Contexte

L'app agent terrain accumule des écritures pendant 7 à 14 jours en brousse. Au retour de connectivité, plusieurs agents peuvent avoir édité les mêmes entités (un agent crée une livraison, un autre corrige le poids du même lot, l'union faîtière modifie le statut d'une transaction). Sans stratégie explicite, on perd de la donnée ou on en duplique.

### Entités et exigences
| Entité | Concurrent edits ? | Auditabilité requise | Politique souhaitée |
|---|---|---|---|
| Producteur (membre) | Rare | Élevée | LWW (server timestamp) |
| Parcelle (polygone GPS) | Modéré (correction terrain) | Élevée | CRDT merge + revue manuelle si geom changes |
| Livraison récolte | Très rare (1 agent / 1 livraison) | Critique | Append-only (event sourcing) |
| Paiement mobile money | Jamais (idempotency key) | Critique légale | Transaction ACID centralisée |
| Photo / pièce jointe | Jamais (UUID local) | Légale | Append-only + S3 immuable |
| Statut administratif (validé / rejeté) | Modéré | Élevée | LWW server-side avec workflow |

## Options envisagées

### Option A — Last-Write-Wins partout
**Pour** : simple, ne demande pas de bibliothèque CRDT.
**Contre** : perte silencieuse de donnée si deux agents éditent le même polygone, inacceptable pour parcelles EUDR.

### Option B — CRDT partout (Automerge / Yjs)
**Pour** : convergence garantie, pas de perte.
**Contre** : surcharge espace (history compaction nécessaire), complexité dev, mauvais fit pour transactions monétaires (auditabilité linéaire requise).

### Option C — Hybrid par type d'entité
**Pour** : chaque entité utilise la stratégie qui matche son sémantique.
**Contre** : plus complexe à concevoir mais une seule fois.

### Option D — Event sourcing global (CQRS)
**Pour** : auditabilité parfaite, replay possible.
**Contre** : courbe d'apprentissage forte pour l'équipe, overkill pour beaucoup d'entités.

## Décision

**Option C — Hybrid par type d'entité**, avec règles explicites :

1. **Append-only event log** (event sourcing localisé) pour :
   - Livraisons récolte
   - Paiements mobile money (avec idempotency key obligatoire côté client)
   - Distributions intrants
   - Photos / scans CNIB
2. **CRDT (Automerge)** pour :
   - Parcelles (polygones GPS) — coordonnées + métadonnées (cultures, surface)
   - Profil producteur étendu (notes agent, observations)
3. **LWW avec server timestamp** pour :
   - Statuts administratifs (validé / rejeté / en révision)
   - Champs simples membres (téléphone, photo)
4. **ACID centralisé** (pas d'écriture offline) pour :
   - Création comptes utilisateurs (admin)
   - Soumission DDS EUDR (un seul nœud autoritaire)
   - Contrats commerciaux (signature électronique)

### Détails techniques
- Idempotency keys : UUID v7 (timestamp + random) générés côté client à chaque création
- Conflict log : chaque conflit CRDT non-trivial est journalisé en table `terroir_conflict_log` avec snapshot avant/après → audit + rollback humain possible
- Photo immuable : UUID local, upload S3 avec ETag = hash sha256 du payload, pas d'écrasement
- Vector clock : chaque agent a un `agent_id` (issu Vault), incrémenté à chaque opération offline

### Résolution conflits parcelles (cas critique EUDR)
- Si 2 agents redessinent le même polygone offline :
  - CRDT merge produit une géométrie « mêlée »
  - Détection : `ST_HausdorffDistance(old, new) > 50m` → flag conflict
  - Workflow : alerte gestionnaire union, blocage temporaire de la parcelle pour DDS, validation manuelle en 24-72h
- Pour audit EUDR : chaque version conservée 5 ans

## Conséquences

### Positives
- Sémantique correcte par type
- Auditabilité légale préservée (paiements + DDS = log linéaire)
- Pas de perte de donnée terrain
- Photos immuables = pas de doute en cas de litige

### Négatives
- Complexité côté dev : 4 stratégies à maintenir
- Automerge ajoute ~200 KB JS au bundle mobile (acceptable)
- Coût stockage history CRDT (compaction obligatoire mensuelle)

### Mitigations
- Documenter chaque entité dans schéma `terroir-domain.md` avec sa stratégie
- Tester intensivement (property-based) la merge CRDT parcelles avec proptest
- Job nightly de compaction Automerge (history → snapshot)

## Métriques de succès

- 0 perte donnée détectée sur 1000 sync simulées multi-agents
- Latence sync delta-encoded ≤ 50 KB par sync typique
- Détection conflit polygone < 24h après sync
- Compaction history conserve < 5x taille snapshot

## Révision

À reconfirmer après 1 campagne complète (P2). Si compaction Automerge dérape, considérer Yjs ou implémentation custom (LiteFS + manual merge).
