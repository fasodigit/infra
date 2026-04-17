# POLITIQUE SCHEMA REGISTRY — FASO DIGITALISATION v3.1

**Version** : 3.1
**Date** : 2026-04-16
**Portée** : 8 sous-projets de l'écosystème souverain FASO DIGITALISATION
(ÉTAT-CIVIL, HOSPITAL, E-TICKET, VOUCHERS, SOGESY, E-SCHOOL, ALT-MISSION, FASO-KALAN)
**Auteur** : Direction technique — Plateforme souveraine
**Statut** : Normatif — obligatoire sur la couche 2 (Redpanda)

---

## 1. Principes souverains

FASO DIGITALISATION déploie une pile 100 % souveraine :

- **Couche 1 — In-memory** : KAYA (moteur Rust souverain). Aucune dépendance DragonflyDB, aucune dépendance Redis.
- **Couche 2 — Durabilité légale** : Redpanda RF=3 RAFT + **Redpanda Schema Registry**.
- **Couche 3 — Vérité persistante** : YugabyteDB chiffré (PostgreSQL distribué).

La couche 2 est la **couche d'audit et de rejeu légal**. Un évènement publié en 2026 doit rester relisible en 2036 pour contentieux, contrôle de la Cour des comptes ou inspection administrative. Cette exigence impose un format strictement versionné : **Protobuf proto3 + Schema Registry**.

---

## 2. Sérialisation — Règle normative

| Règle | Valeur |
|---|---|
| Format obligatoire sur topics durables | **Protobuf proto3** |
| JSON sur couche 2 (Redpanda) | **INTERDIT** |
| Avro | Non retenu (écosystème Java-centric, verbosité) |
| Encodage payload | `application/x-protobuf` + header `schema-id` |
| Enregistrement du schéma | Redpanda Schema Registry (compatible Confluent API) |

### Justifications

1. **Backward / forward compatibility native** : proto3 garantit qu'un champ ajouté avec un nouveau tag est ignoré par un lecteur ancien.
2. **Taille binaire** : typiquement 3 à 10× plus petit que JSON, critique pour la rétention 10 ans de l'état civil.
3. **Cohérence gRPC E-W** : les mêmes `.proto` servent la communication inter-service gRPC et les events Redpanda → un seul catalogue source de vérité.
4. **Schema Registry natif** : Redpanda expose l'API Schema Registry sans broker additionnel.
5. **Pas de parsing ambigu** : les types sont stricts (`int64`, `bytes`, `Timestamp`), contre JSON qui confond `number` avec `int64` au-delà de 2^53.

---

## 3. Nommage des topics

### Convention

```
{projet}.{aggregat}.{event}.v{N}
```

- `{projet}` : code court du sous-projet (voir tableau ci-dessous)
- `{aggregat}` : nom de l'agrégat DDD (singulier, kebab-case)
- `{event}` : verbe au participe passé (`created`, `validated`, `sealed`, …)
- `v{N}` : version majeure du schéma, entier ≥ 1

### Codes projets

| Code | Sous-projet |
|---|---|
| `ec` | ÉTAT-CIVIL |
| `hosp` | HOSPITAL |
| `eticket` | E-TICKET |
| `vouchers` | VOUCHERS |
| `sogesy` | SOGESY |
| `eschool` | E-SCHOOL |
| `alt` | ALT-MISSION |
| `kalan` | FASO-KALAN |

### Exemples canoniques

- `ec.demande.created.v1`
- `ec.acte.signed.v1`
- `hosp.admission.registered.v1`
- `hosp.prescription.validated.v1`
- `eticket.ticket.purchased.v1`
- `sogesy.boarding-pass.issued.v1`
- `vouchers.transaction.confirmed.v1`
- `eschool.inscription.validated.v1`
- `alt.mission.approved.v1`
- `kalan.session.completed.v1`

Le subject associé dans le Schema Registry est `{topic}-value` (stratégie **TopicNameStrategy**).

---

## 4. Règles d'évolution des schémas

| Opération | Autorisée ? | Précision |
|---|---|---|
| Ajout d'un champ `optional` | **OUI** | Tag numérique nouveau, jamais réutilisé |
| Ajout d'une valeur enum à la fin | **OUI** | Obligatoirement en dernière position numérique |
| Renommage d'un champ | **NON** | Créer un nouveau champ, marquer l'ancien `[deprecated = true]` |
| Suppression d'un champ | **NON directement** | Marquer `reserved` le tag et le nom ; conservé en lecture |
| Changement de type d'un champ | **NON** | Casse la compatibilité binaire |
| Changement sémantique d'un champ existant | **NON** | Création d'un nouveau topic `.v2` |
| Renommage d'une valeur enum | **NON** | Garder l'identifiant historique |
| Passage `optional` → `required` | **NON applicable** | proto3 n'a pas `required` |
| Ajout d'un `oneof` groupant des champs existants | **NON** | Casse la compatibilité |

### Règle d'or

> Toute modification qui n'est pas dans la colonne « OUI » **exige** un nouveau topic `{topic}.v{N+1}`.

---

## 5. Mode de compatibilité

**Mode imposé** : `BACKWARD_TRANSITIVE`

### Signification

Un consumer à la version `N` du schéma doit pouvoir lire :
- les messages produits avec la version `N`,
- les messages produits avec la version `N-1`,
- les messages produits avec la version `N-2`,
- … et toutes les versions antérieures du subject.

### Pourquoi pas `BACKWARD` simple

`BACKWARD` ne vérifie que la compatibilité `N` vs `N-1`. Or nos topics d'audit conservent des messages sur 10 ans ; un consumer en 2036 relira des messages produits par la `v1` de 2026. Seul `BACKWARD_TRANSITIVE` couvre ce cas.

### Pourquoi pas `FULL_TRANSITIVE`

`FULL_TRANSITIVE` exige la compatibilité forward également, ce qui interdit l'ajout d'un nouveau champ. Trop restrictif pour l'évolution fonctionnelle.

---

## 6. Stratégie de migration (breaking change)

Quand un changement breaking est inévitable :

1. **Créer** un nouveau topic `{topic}.v{N+1}` avec le nouveau schéma.
2. **Cohabitation** : producer écrit simultanément sur `.v{N}` et `.v{N+1}` pendant **6 mois minimum**.
3. **Migration progressive** des consumers : chaque consumer bascule individuellement, monitoring du lag.
4. **Marquage** du topic `.v{N}` comme déprécié dans la table `registry.deprecated` (cf. §9).
5. **Arrêt** des producers `.v{N}` après validation de la bascule de tous les consumers.
6. **Conservation** en lecture seule de `.v{N}` jusqu'à expiration de la rétention légale.

---

## 7. Gouvernance CI/CD

### Pipeline obligatoire (GitHub Actions)

Chaque PR modifiant `proto/**/*.proto` déclenche :

```bash
buf lint
buf breaking --against '.git#branch=main'

for file in $(git diff --name-only origin/main...HEAD -- 'proto/**/*.proto'); do
  subject=$(derive_subject "$file")  # ec.demande.created.v1-value
  rpk registry schema check-compatibility \
      --subject "$subject" \
      --schema "$file" \
      --type protobuf \
      --compatibility BACKWARD_TRANSITIVE
done
```

- **Échec bloquant** : si `check-compatibility` retourne une incompatibilité, la PR ne peut pas être mergée.
- **Sur merge `main`** : publication automatique via `rpk registry schema create`.

---

## 8. Validation consumer au démarrage

Chaque microservice consumer, à son démarrage :

1. Lit la liste des subjects qu'il consomme dans sa configuration.
2. Récupère la version attendue via :
   ```bash
   rpk registry schema get --subject {topic}-value --version {N}
   ```
3. Compare au `.proto` compilé embarqué.
4. **Fail fast** si :
   - le subject n'existe pas,
   - la version majeure attendue n'est pas présente,
   - le hash du schéma local ne correspond à aucune version enregistrée.

Effet : aucun consumer ne peut démarrer sur un cluster dont le Schema Registry serait corrompu ou vide.

---

## 9. Dépréciation

Une table applicative `registry.deprecated` (dans YugabyteDB) recense :

| Colonne | Type | Description |
|---|---|---|
| `subject` | text | ex. `ec.demande.created.v1-value` |
| `deprecated_since` | timestamptz | Date d'annotation |
| `sunset_date` | timestamptz | Date d'arrêt effective des producers |
| `replacement_subject` | text | ex. `ec.demande.created.v2-value` |
| `reason` | text | Motif |
| `owner_team` | text | Équipe responsable |

Un dashboard Grafana met en évidence les topics dépréciés encore producteurs après `sunset_date`.

---

## 10. Politique PII — RÈGLE SOUVERAINE

### Principe

> **AUCUNE donnée à caractère personnel en clair** ne transite par Redpanda. Les payloads ne contiennent que des **UUID opaques**. Les PII résident uniquement dans YugabyteDB où le chiffrement au repos et l'effacement RGPD restent techniquement possibles.

### Pourquoi

- Redpanda est **append-only** : supprimer un message spécifique pour RGPD y est impossible sans compaction destructive.
- La rétention légale (5 à 10 ans) entre en conflit direct avec le droit à l'effacement si les PII sont dans le log.
- Seule solution conforme : séparation stricte **identifiant (log) / données (base chiffrée)**.

### Données autorisées dans un payload Protobuf

| Type | Autorisé | Commentaire |
|---|---|---|
| `string` UUID (tenant, agrégat, utilisateur) | OUI | Opaque, non réversible sans base |
| `google.protobuf.Timestamp` | OUI | Pas de PII |
| Énumérations de statut | OUI | `CREATED`, `VALIDATED`, etc. |
| Montants monétaires (`int64` en centimes) | OUI | Pas de PII seule |
| Clés techniques (`idempotency_key`) | OUI | UUID opaque |
| Hash / HMAC (`bytes`) | OUI | Non réversible |
| Codes géographiques (commune, région) | OUI conditionnel | Granularité ≥ commune, pas d'adresse |
| Type d'acte, catégorie | OUI | Données fonctionnelles |

### Données INTERDITES dans un payload Protobuf

| Type | Interdit | Raison |
|---|---|---|
| Nom, prénom en clair | INTERDIT | PII directe |
| NIP / NIA / CNIB | INTERDIT | Identifiant national |
| Date de naissance complète | INTERDIT | Quasi-identifiant |
| Adresse postale | INTERDIT | PII directe |
| Numéro de téléphone | INTERDIT | PII directe |
| Email | INTERDIT | PII directe |
| Données biométriques | INTERDIT | PII sensible |
| Diagnostic médical lisible | INTERDIT | PII sensible (art. 9 RGPD BF) |
| Photo / scan de document | INTERDIT | Stockage objet chiffré séparé |
| Texte libre saisi par utilisateur | INTERDIT | Risque PII incontrôlé |

### Mise en œuvre

- Le linter `buf` est configuré avec une règle custom `FASO_PII_GUARD` qui rejette tout champ dont le nom contient : `nom`, `prenom`, `adresse`, `tel`, `phone`, `email`, `nip`, `cnib`, `birth`, `dob`, `firstname`, `lastname`.
- Les revues de code exigent un contrôle humain sur tout champ `string` non suffixé par `_id`, `_key`, `_code`, `_type`, `_status`.

---

## 11. Rétention par topic

| Pattern de topic | Rétention | Justification légale / fonctionnelle |
|---|---|---|
| `*.audit-trail.v*` | **10 ans** | Obligation de conservation des actes (Code civil BF) |
| `ec.*.v*` | **5 ans** | Cycle de vie d'une demande d'acte + recours administratif |
| `hosp.*.v*` | **10 ans** | Dossier médical légal (loi santé BF) |
| `eticket.*.v*` | **90 jours** | Cycle court du ticket, pas d'enjeu contentieux long |
| `sogesy.*.v*` | **1 an** | Cycle du boarding pass, contrôle a posteriori |
| `vouchers.transaction.*` | **7 ans** | Obligation comptable et fiscale (Code général des impôts BF) |
| `eschool.*.v*` | **3 ans** | Année scolaire + archivage inscription |
| `alt.*.v*` | **2 ans** | Contrôle interne ordres de mission |
| `kalan.*.v*` | **1 an** | Service éducatif, certificats eux-mêmes stockés en base |

---

## 12. Rétention fine — compaction et segmentation

Pour les topics qui représentent **l'état courant d'un agrégat** (et non uniquement un flux d'évènements), on active la compaction logarithmique :

| Topic | `cleanup.policy` | `retention.ms` | `segment.ms` | Commentaire |
|---|---|---|---|---|
| `ec.demande.*.v*` | `delete` | 5 ans | 7 jours | Flux évènementiel pur |
| `ec.acte.state.v1` | `compact` | -1 | 1 jour | État courant par `acte_id` (clé) |
| `hosp.dossier.state.v1` | `compact,delete` | 10 ans | 1 jour | Compaction + plancher 10 ans |
| `eticket.ticket.*.v*` | `delete` | 90 jours | 1 jour | Court terme |
| `sogesy.boarding-pass.*.v*` | `delete` | 1 an | 7 jours | Chaîne hash append-only |
| `vouchers.transaction.*.v*` | `delete` | 7 ans | 30 jours | Preuve fiscale, jamais compacté |
| `eschool.inscription.*.v*` | `delete` | 3 ans | 7 jours | — |
| `alt.mission.*.v*` | `delete` | 2 ans | 7 jours | — |
| `kalan.session.*.v*` | `delete` | 1 an | 7 jours | — |
| `*.audit-trail.v*` | `delete` | 10 ans | 30 jours | Jamais compacté (non-répudiation) |

**Règle** : un topic de type `audit-trail` ou `transaction` **ne doit jamais** avoir `cleanup.policy=compact`.

---

## 13. Exemple de workflow CI (GitHub Actions)

Fichier `.github/workflows/schema-check.yml` (livré complet en Annexe A, cf. `ci/schema-check.yml`) :

- Déclenché sur PR touchant `proto/**/*.proto`.
- Installe `rpk` (Redpanda CLI) et `buf`.
- Lance `buf lint` puis `buf breaking` contre `main`.
- Boucle sur les `.proto` modifiés et appelle `rpk registry schema check-compatibility` en mode `BACKWARD_TRANSITIVE`.
- Échec bloquant en cas de breaking change détecté.
- Sur merge `main`, publication automatique via `rpk registry schema create`.

---

## 14. Références

- Section 9 du guide FASO DIGITALISATION v3.0 — Schema Registry obligatoire couche légale
- Redpanda Schema Registry — documentation v24+
- Confluent Schema Registry compatibility rules (modèle de référence)
- RGPD Burkina Faso — Loi n°010-2004/AN portant protection des données personnelles

---

*Document normatif. Toute dérogation doit faire l'objet d'un ADR (Architecture Decision Record) validé par la Direction technique.*
