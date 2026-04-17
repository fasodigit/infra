# Release Guide — FASO DIGITALISATION

<!-- SPDX-License-Identifier: AGPL-3.0-only -->

Releases are **fully automated** via [semantic-release](https://semantic-release.gitbook.io/) driven by [Conventional Commits](https://www.conventionalcommits.org/).

## How It Works

```
Commit push to main/beta/alpha
        │
        ▼
semantic-release.yml (GitHub Actions)
        │
        ├─ Analyses commits since last tag
        ├─ Determines next version (semver)
        ├─ Bumps versions: Cargo.toml / pom.xml / package.json
        ├─ Updates CHANGELOG.md
        ├─ Creates git tag  v<MAJOR>.<MINOR>.<PATCH>
        ├─ Commits changes back to branch
        └─ Creates GitHub Release with generated notes
                │
                ▼
        release.yml (triggered by new tag v*)
                └─ Builds cross-compiled binaries + SBOM + attaches to Release
```

## Triggering a Release

### Automatic (recommended)

Push a conventional commit to `main`, `beta`, or `alpha`:

```bash
git commit -m "feat(kaya): add geo radius search"
git push origin main
# → semantic-release analyses commits, determines minor bump, publishes v0.2.0
```

### Manual (dry-run or forced)

```
GitHub → Actions → "Semantic Release" → Run workflow
  dry_run: true   # inspect what WOULD be released, no changes
  dry_run: false  # force run (same as push trigger)
```

## Version Bump Rules

| Commit type | Semver bump | Example |
|------------|-------------|---------|
| `feat:` | **minor** | `feat(auth-ms): OIDC pkce flow` |
| `fix:` / `perf:` / `revert:` | **patch** | `fix(kaya): wal replay on crash` |
| `BREAKING CHANGE:` footer | **major** | see below |
| `chore:` / `docs:` / `ci:` / `test:` / `style:` / `refactor:` | **none** | |

## Breaking Changes

A breaking change bumps the **major** version (e.g. `v1.x.x → v2.0.0`).

**How to mark a breaking change:**

```
feat(kaya): redesign cluster membership API

BREAKING CHANGE: ClusterConfig.peers field renamed to ClusterConfig.members.
All clients must update their configuration before upgrading.
```

The `BREAKING CHANGE:` token **must** appear in the commit footer (after a blank line).

## Skipping a Release

To push changes without triggering a release (e.g. housekeeping):

```bash
git commit -m "chore(release): skip"
# or use any type that doesn't produce a release:
git commit -m "chore: update renovate config"
git commit -m "docs: fix typo in README"
```

## Branch Channels

| Branch | Channel | Tag format | npm dist-tag |
|--------|---------|-----------|--------------|
| `main` | stable | `v1.2.3` | `latest` |
| `beta` | pre-release | `v1.3.0-beta.1` | `beta` |
| `alpha` | pre-release | `v1.3.0-alpha.1` | `alpha` |

## Multi-Package Version Synchronisation

semantic-release automatically keeps all package manifests in sync via `scripts/`:

| Script | Targets |
|--------|---------|
| `scripts/bump-versions.sh` | `kaya/Cargo.toml`, `armageddon/Cargo.toml` (`[workspace.package].version`) |
| `scripts/bump-java-versions.sh` | `auth-ms/pom.xml`, `poulets-platform/backend/pom.xml`, `notifier-ms/pom.xml` (via `mvn versions:set`) |
| `scripts/bump-node-versions.sh` | `poulets-platform/bff/package.json`, `poulets-platform/frontend/package.json` |

## Tag Policy

> **NEVER delete or alter a published tag.**

Tags represent immutable release points in the AGPL-3.0 audit trail.
If a release contains a critical bug, publish a new patch version (`fix:`) instead.

```bash
# Wrong — never do this:
git tag -d v1.2.0
git push origin :refs/tags/v1.2.0

# Correct — publish a fix:
git commit -m "fix(kaya): critical data loss on wal flush"
git push origin main
# → semantic-release publishes v1.2.1
```

## AGPL-3.0 Notice

All releases are published under the [GNU Affero General Public License v3.0](https://www.gnu.org/licenses/agpl-3.0.html).
The license notice is embedded in every generated GitHub Release body.
Source code must remain available to all users who interact with the software over a network.

## Required Repository Secrets

| Secret | Purpose |
|--------|---------|
| `GITHUB_TOKEN` | Auto-provided by GitHub Actions — creates tags, releases, comments |
| `NPM_TOKEN` | Required only if packages are published to npm registry |

## Troubleshooting

**No release published after push to main?**
- Check commit messages — they must be conventional commits (`feat:`, `fix:`, etc.)
- `chore:`, `docs:`, `ci:`, `style:`, `test:`, `refactor:` do **not** produce a release
- Check the "Semantic Release" workflow logs in GitHub Actions

**Release failed mid-way?**
- The workflow is idempotent — re-run it; it will pick up from the last published tag
- Do NOT manually create tags to "fix" the issue
