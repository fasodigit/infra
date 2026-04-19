# kaya-bench — Criterion Benchmarks

Criterion-based micro-benchmarks for the KAYA sovereign in-memory database.

## Running locally

From the workspace root (`INFRA/kaya/`):

```bash
# All benchmarks
cargo bench -p kaya-bench

# Individual benchmark suites
cargo bench --bench commands    -p kaya-bench
cargo bench --bench datatypes   -p kaya-bench
cargo bench --bench persistence -p kaya-bench
cargo bench --bench pubsub      -p kaya-bench
cargo bench --bench streams     -p kaya-bench

# Bencher-compatible JSON output (suitable for local diffing)
cargo bench --bench commands -p kaya-bench -- --output-format bencher

# Filter a specific group or function
cargo bench --bench commands -p kaya-bench -- SET/256
```

HTML reports are generated under `target/criterion/` and can be opened in any browser.

## Benchmark suites

| File | Commands covered |
|------|-----------------|
| `benches/commands.rs` | SET, GET, INCR, MSET, MGET, DEL |
| `benches/datatypes.rs` | SADD, SMEMBERS, SCARD, ZADD, ZRANGE, ZRANGEBYSCORE |
| `benches/persistence.rs` | WAL record encode, batch encode, snapshot full-scan |
| `benches/pubsub.rs` | PUBLISH fanout 1–1 000 subs, payload size sweep, SUB/UNSUB roundtrip |
| `benches/streams.rs` | XADD, XADD bulk, XREAD, XREADGROUP |

## bencher.dev integration

Results are uploaded automatically on every PR and push to `main` via the
`.github/workflows/criterion.yml` workflow.

**Activating the token** (one-time, per repository):

1. Create a free account at <https://bencher.dev>.
2. Create a project named `faso-kaya`.
3. Generate an API token under *Settings > Tokens*.
4. Add it as a GitHub Actions secret named `BENCHER_API_TOKEN` in
   *Settings > Secrets and variables > Actions*.

Once the token is set, every PR will have a bencher.dev comment showing
throughput and latency deltas relative to the base branch.

## Regression threshold: 5 %

Criterion measurement noise on a stable CI machine is typically 2–3 %.
The 5 % upper boundary gives a comfortable margin before flagging a real
regression.  Snapshot scan and XREADGROUP use 8 % because those operations
touch more memory and are more susceptible to OS page-cache variance.

Thresholds are declared in `.github/bencher-config.yml`.

## Adding a new benchmark

1. Create `benches/<name>.rs` with a `criterion_group!` + `criterion_main!`.
2. Add a `[[bench]] name = "<name>" harness = false` entry in `Cargo.toml`.
3. Add matching threshold entries in `.github/bencher-config.yml`.
4. Add the name to the `matrix.bench` list in `.github/workflows/criterion.yml`.
