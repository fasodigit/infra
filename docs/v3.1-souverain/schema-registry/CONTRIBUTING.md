# Schema Registry — Règles d'évolution Protobuf

Politique de compatibilité : **BACKWARD_TRANSITIVE**
Naming convention : `{projet}.{aggregat}.{event}.v{N}`

## Règles fondamentales

### Autorisé

| Opération | Condition |
|-----------|-----------|
| Ajouter un champ `optional` | Numéro de champ nouveau, non réutilisé |
| Ajouter un nouveau message | Doit avoir des commentaires doc obligatoires |
| Ajouter une valeur à un enum | Valeur `UNSPECIFIED = 0` doit déjà exister |
| Créer une nouvelle version (`v2`, `v3`…) | Nouveau package, ancien package conservé |

### Interdit

| Opération | Raison |
|-----------|--------|
| **Supprimer un champ** | Casse les consommateurs qui lisent ce champ |
| **Renommer un champ** | Casse la sérialisation JSON et les consommateurs |
| **Changer le type d'un champ** | Incompatible au niveau fil (wire) et JSON |
| **Changer le numéro de champ** | Binaire incompatible, données corrompues |
| **Supprimer un message ou une enum** | Casse tous les référents |
| **Changer la sémantique d'un champ** | Brise les contrats d'intégration |

## Procédure pour un changement Breaking

Si un changement breaking est inévitable (migration majeure) :

1. Créer un **nouveau package versionné** (`v2`, `v3`…) avec le nouveau schéma.
2. Maintenir l'ancien package pendant une période de dépréciation (minimum 3 sprints).
3. Ouvrir la PR avec le label `breaking-accepted`.
4. **Obligatoire** : inclure dans la description PR :
   - Justification technique
   - Services impactés
   - Plan de migration et rollback
5. Obtenir l'approbation d'au moins 2 reviewers schema-owners.

## Workflow CI

- **buf-breaking.yml** : bloque toute PR modifiant des `.proto` si un breaking change est détecté,
  sauf label `breaking-accepted` + justification dans la description.
- **buf-push.yml** : pousse les schémas validés vers `buf.build/fasodigit/faso-events` à chaque merge sur `main`.

## Références

- [Buf Breaking Change Rules](https://buf.build/docs/breaking/rules)
- [Protobuf Language Guide](https://protobuf.dev/programming-guides/proto3/)
- [POLICY-SCHEMA-REGISTRY-v3.1.md](./POLICY-SCHEMA-REGISTRY-v3.1.md)
