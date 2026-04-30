<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Runbook — Keto tuples TERROIR (multi-tenancy ABAC)

Référence : ADR-006 (multi-tenancy), ULTRAPLAN §4 P0.4.
Source de vérité OPL : `INFRA/ory/keto/config/namespaces.ts`.
Script seed : `INFRA/ory/keto/scripts/seed-terroir-tuples.sh`.

## 1. Namespaces TERROIR (4)

### `Tenant`
Coopérative cliente, union, exportateur ou bailleur (top-level).

| Relation        | Sujet  | Permits qui l'utilise               |
|-----------------|--------|-------------------------------------|
| `member`        | User   | `view`                              |
| `admin`         | User   | `view`, `manage`, `onboard_member`  |
| `agent_terrain` | User   | (réservé futurs permits)            |
| `gestionnaire`  | User   | `view`, `manage`, `onboard_member`  |
| `exporter`      | User   | `view`, `submit_dds`                |
| `bailleur`      | User   | `view`                              |

Permits : `view`, `manage`, `onboard_member`, `submit_dds`.

### `Cooperative`
Coopérative primaire, fille d'un Tenant.

| Relation          | Sujet              | Permits                                         |
|-------------------|--------------------|-------------------------------------------------|
| `parent`          | Tenant (subj-set)  | hérite `view` / `manage_members` du parent      |
| `member_producer` | User               | (visibilité via `parent.view`)                  |
| `agent_collector` | User               | `record_harvest`                                |
| `secretary`       | User               | `manage_members`                                |

Permits : `view`, `manage_members`, `record_harvest`.

### `Parcel`
Parcelle agricole rattachée à une Cooperative.

| Relation            | Sujet                  | Permits                              |
|---------------------|------------------------|--------------------------------------|
| `parent`            | Cooperative (subj-set) | hérite `view`                        |
| `owner_producer`    | User                   | `view`                               |
| `editor_agent`      | User                   | `view`, `edit_polygon`, `submit_eudr`|
| `viewer_supervisor` | User                   | `view`                               |

Permits : `view`, `edit_polygon`, `submit_eudr`.

### `HarvestLot`
Lot de récolte (cacao/café), rattaché à une Cooperative.

| Relation             | Sujet                  | Permits        |
|----------------------|------------------------|----------------|
| `parent`             | Cooperative (subj-set) | hérite `view`  |
| `creator_agent`      | User                   | `record`       |
| `approver_secretary` | User                   | `approve`      |

Permits : `view`, `record`, `approve`.

## 2. Octroi / révocation de tuples (API admin :4467)

```bash
KETO_WRITE_URL=http://localhost:4467

# Octroi : Aminata devient gestionnaire du tenant t_pilot
curl -fsS -X PUT "${KETO_WRITE_URL}/admin/relation-tuples" \
  -H 'Content-Type: application/json' \
  -d '{"namespace":"Tenant","object":"t_pilot",
       "relation":"gestionnaire","subject_id":"<uuid>"}'

# Octroi subject-set : la coop pilote a Tenant:t_pilot pour parent
curl -fsS -X PUT "${KETO_WRITE_URL}/admin/relation-tuples" \
  -H 'Content-Type: application/json' \
  -d '{"namespace":"Cooperative","object":"<coop-uuid>",
       "relation":"parent",
       "subject_set":{"namespace":"Tenant","object":"t_pilot","relation":""}}'

# Révocation : retirer un agent_collector
curl -fsS -X DELETE "${KETO_WRITE_URL}/admin/relation-tuples?\
namespace=Cooperative&object=<coop-uuid>&\
relation=agent_collector&subject_id=<uuid>"

# Lecture (read API :4466)
curl -fsS 'http://localhost:4466/relation-tuples?namespace=Tenant'
```

## 3. Pattern d'intégration côté services Rust

Inline check via `ory-keto-client` ou simple HTTP (port 4466) :

```rust
// Cargo.toml : reqwest = { version = "0.12", features = ["json"] }
async fn keto_check(
    namespace: &str, object: &str, relation: &str, subject_id: &str,
) -> anyhow::Result<bool> {
    let url = std::env::var("KETO_READ_URL")
        .unwrap_or_else(|_| "http://localhost:4466".into());
    let resp = reqwest::Client::new()
        .get(format!("{url}/relation-tuples/check"))
        .query(&[
            ("namespace", namespace), ("object", object),
            ("relation", relation), ("subject_id", subject_id),
        ])
        .send().await?
        .error_for_status()?
        .json::<serde_json::Value>().await?;
    Ok(resp.get("allowed").and_then(|v| v.as_bool()).unwrap_or(false))
}

// Usage dans terroir-eudr (handler submit_dds) :
if !keto_check("Parcel", &parcel_id, "submit_eudr", &user_id).await? {
    return Err(StatusCode::FORBIDDEN.into());
}
```

Convention : middleware `axum` extrait `tenant_id` du JWT (claim Kratos),
puis `keto_check("Tenant", &tenant_id, "view", &user_id)` avant tout
accès au schema PG `terroir_t_<slug>`.

## 4. Seed local (idempotent)

```bash
export SEED_SA_AMINATA="<uuid-aminata>"      # défaut t_pilot fourni
bash INFRA/ory/keto/scripts/seed-terroir-tuples.sh
# Vérifie ≥ 4 tuples :
curl -s 'http://localhost:4466/relation-tuples?namespace=Tenant' | jq
curl -s 'http://localhost:4466/relation-tuples?namespace=Cooperative' | jq
```

## 5. Anti-patterns

- Ne JAMAIS écrire un tuple sans namespace déclaré dans `namespaces.ts`.
- Ne JAMAIS court-circuiter Keto par un check applicatif "à la main".
- Toute nouvelle relation/permit passe par PR + ADR amendement.
- Les `subject_set` (héritage parent) ne supportent pas les cycles.
