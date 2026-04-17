# Schema `github.events.v1`

**Package** : `faso.github.events.v1`
**Java package** : `bf.gov.faso.github.events.v1`
**Status** : v1 — Current
**Compatibility** : `BACKWARD_TRANSITIVE` (see [POLICY-SCHEMA-REGISTRY-v3.1.md](../../../../POLICY-SCHEMA-REGISTRY-v3.1.md))

GitHub webhook events ingested by the FASO DIGITALISATION CI/CD observability
layer. Events flow from GitHub → FASO Ingestion Gateway → Redpanda topics.

---

## Kafka Topics

| Topic | Message type | Kafka key |
|---|---|---|
| `github.push.v1` | `GithubPushEvent` | `repository.full_name` |
| `github.pull_request.v1` | `GithubPullRequestEvent` | `repository.full_name` |
| `github.issue.v1` | `GithubIssueEvent` | `repository.full_name` |
| `github.workflow_run.v1` | `GithubWorkflowRunEvent` | `repository.full_name` |
| `github.release.v1` | `GithubReleaseEvent` | `repository.full_name` |
| `github.raw.v1` | `GithubRawEvent` | `repository_full_name` |

Kafka key is always `repository.full_name` to ensure deterministic partitioning:
all events for a given repository land on the same partition in order.

---

## GitHub Event → Protobuf Mapping

| X-GitHub-Event header | Protobuf message | Topic |
|---|---|---|
| `push` | `GithubPushEvent` | `github.push.v1` |
| `pull_request` | `GithubPullRequestEvent` | `github.pull_request.v1` |
| `issues` | `GithubIssueEvent` | `github.issue.v1` |
| `workflow_run` | `GithubWorkflowRunEvent` | `github.workflow_run.v1` |
| `release` | `GithubReleaseEvent` | `github.release.v1` |
| `check_run`, `deployment`, `star`, `member`, `project_card`, `registry_package`, and any unrecognised type | `GithubRawEvent` | `github.raw.v1` |

The ingestion gateway inspects the `X-GitHub-Event` HTTP header and routes to
the appropriate typed message. Unknown event types always fall through to
`GithubRawEvent` so no webhook delivery is silently discarded.

---

## Version Table

| Version | Status | Notes |
|---|---|---|
| **v1** | Current | All 6 event types; fields `delivery_id`, `received_at`, `event_type` are mandatory |
| v2 | Reserved | For future breaking changes only (new `oneof` groupings, type renames) |

---

## Field-Level Requirements

| Field | Optional / Required | Notes |
|---|---|---|
| `delivery_id` | **Required** | X-GitHub-Delivery UUID; used as idempotency key |
| `received_at` | **Required** | Gateway ingestion timestamp |
| `event_type` (`GithubRawEvent` only) | **Required** | X-GitHub-Event header value |
| All other fields | `optional` | May be absent depending on GitHub payload variant |

---

## PII Policy

These schemas comply with [POLICY-SCHEMA-REGISTRY-v3.1.md §10](../../../../POLICY-SCHEMA-REGISTRY-v3.1.md).

- `GithubUser.login`, `.name`, `.email` are **SCM-level metadata**, not civil-registry
  personal data. They identify GitHub accounts, not citizens of Burkina Faso.
- No NIP, CNIB, date de naissance, adresse postale, or numéro de téléphone are
  present in any field. The `reserved` blocks in `types.proto` explicitly forbid
  these names from future re-use.
- `GithubPullRequest.body` may be populated but **must never contain PII** — the
  CI review checklist enforces this at PR time.

---

## Evolution Guidelines

Follow [CONTRIBUTING.md](../../../../CONTRIBUTING.md) for all changes.

| Allowed | Forbidden |
|---|---|
| Adding a new `optional` field with a new tag number | Deleting any existing field |
| Adding a new `GithubXxxEvent` message | Renaming a field |
| Adding a value to an enum | Changing a field type |
| Creating a `v2` package for breaking changes | Reusing a reserved tag number |

**Suppression is INTERDITE.** Mark unwanted fields with `[deprecated = true]` and
add the tag to the `reserved` block. Never remove the field itself.

See [CONTRIBUTING.md](../../../../CONTRIBUTING.md) for the full breaking-change procedure.

---

## JSON Payload Example → Protobuf Equivalent

### push event

**Original GitHub JSON (excerpt)**

```json
{
  "delivery": "abc123-uuid",
  "ref": "refs/heads/main",
  "before": "0000000000000000000000000000000000000000",
  "after":  "a1b2c3d4e5f6...",
  "forced": false,
  "compare": "https://github.com/faso/kaya/compare/000...a1b2",
  "repository": {
    "id": 123456789,
    "full_name": "faso/kaya",
    "name": "kaya",
    "private": true,
    "default_branch": "main",
    "html_url": "https://github.com/faso/kaya"
  },
  "pusher": { "name": "ci-bot", "email": "ci@faso.bf" },
  "commits": [
    {
      "id": "a1b2c3d4e5f6",
      "message": "feat(wal): add fsync batching",
      "timestamp": "2026-04-17T08:30:00Z",
      "added": ["src/wal/batch.rs"],
      "removed": [],
      "modified": ["src/wal/mod.rs"]
    }
  ]
}
```

**Equivalent `GithubPushEvent` (text proto)**

```proto
delivery_id: "abc123-uuid"
received_at: { seconds: 1744882200 }
repository: {
  id: 123456789
  full_name: "faso/kaya"
  name: "kaya"
  private: true
  default_branch: "main"
  html_url: "https://github.com/faso/kaya"
}
ref: "refs/heads/main"
before_sha: "0000000000000000000000000000000000000000"
after_sha:  "a1b2c3d4e5f6"
forced: false
compare_url: "https://github.com/faso/kaya/compare/000...a1b2"
commits: [
  {
    sha: "a1b2c3d4e5f6"
    message: "feat(wal): add fsync batching"
    timestamp: { seconds: 1744882200 }
    added: ["src/wal/batch.rs"]
    modified: ["src/wal/mod.rs"]
  }
]
pusher: { name: "ci-bot" email: "ci@faso.bf" }
```

### raw (fallback) event

**Original GitHub JSON (star event — no typed message)**

```json
{
  "action": "created",
  "repository": { "full_name": "faso/kaya" }
}
```

**Equivalent `GithubRawEvent`**

```proto
delivery_id: "xyz789-uuid"
received_at: { seconds: 1744882500 }
event_type: "star"
repository_full_name: "faso/kaya"
payload_json: <gzip-compressed bytes of the original JSON>
```

---

## Buf Lint

All files in this package must pass:

```bash
buf lint proto/
```

Rules active (from `proto/buf.yaml`):
- `DEFAULT` — standard Protobuf style
- `COMMENTS` — all messages and fields must have doc comments
- `FILE_LOWER_SNAKE_CASE` — file names must be lower_snake_case

---

## Code Generation

### Java

Config: `proto/buf.gen.yaml` plugin `buf.build/protocolbuffers/java:v27.4`
Output: `gen/java/bf/gov/faso/github/events/v1/`
Classes: `GithubPushEvent`, `GithubPullRequestEvent`, `GithubIssueEvent`,
`GithubWorkflowRunEvent`, `GithubReleaseEvent`, `GithubRawEvent`,
`GithubRepository`, `GithubCommit`, `GithubUser`, `GithubPullRequest`, `GithubLabel`

### Rust (prost)

Config: `proto/buf.gen.yaml` plugin `buf.build/community/neoeinstein-prost`
Output: `gen/rust/faso/github/events/v1/`
Module path: `faso::github::events::v1`
