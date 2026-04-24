# ARMAGEDDON — Vague 4 Research Roadmap

> **Status**: Roadmap only — items are research/engineering projects spanning multiple weeks with external dependencies not controllable within a single development session. None of these items should be implemented until their prerequisites are met and dedicated capacity is allocated.
>
> **Tracker convention**: all items should be filed as GitHub issues with label `roadmap:vague-4`.

---

## Item 1 — Post-Quantum TLS (ML-KEM Hybrid)

### Description

Hybridise the TLS 1.3 handshake with X25519 + ML-KEM-768 (Kyber, standardised as FIPS 203). The hybrid approach keeps X25519 as the classical fallback while adding the post-quantum KEM alongside it. The combined shared secret protects against both classical and quantum adversaries simultaneously. This follows RFC 9180 (HPKE) conventions extended to TLS 1.3 key exchange.

### External dependencies

- **rustls 0.23+**: partial hybrid key exchange support via `aws-lc-rs` backend (`CryptoProvider` trait). Full `X25519Kyber768Draft00` group support is tracked in [rustls#1740](https://github.com/rustls/rustls/issues/1740).
- **aws-lc-rs 1.7+**: bundles `aws-lc` with Kyber/ML-KEM support via the `aws-lc-fips` feature.
- **boringssl post-quantum branch** (alternative): if `aws-lc-rs` is insufficient, the BoringSSL Kyber fork can be vendored, but adds significant build complexity.
- **NIST FIPS 203** (2024): ML-KEM is now a published standard — implementation risk is lower than pre-standardisation.

### Use case (FASO)

Protect ARMAGEDDON mTLS inter-service traffic and downstream TLS sessions against "Harvest Now, Decrypt Later" (HNDL) threats. An adversary who records encrypted traffic today can decrypt it once a cryptographically-relevant quantum computer (CRQC) becomes available. FASO handles healthcare/financial data in Burkina Faso — sovereign data with long confidentiality horizons.

### Risk

- **API stability**: rustls `CryptoProvider` API changed between 0.22 and 0.23; further churn before 1.0 is likely.
- **Performance regression**: ML-KEM-768 adds ~1 ms to TLS handshake on a modest CPU (measured on AWS Graviton 2). Acceptable for session establishment; negligible amortised over connection lifetime.
- **SPIRE integration**: SPIRE's SVID rotation uses rustls internally; upgrading rustls may require SPIRE version alignment.

### Effort

**L** — 8–12 weeks. Includes: rustls upgrade, `CryptoProvider` wiring, mTLS test harness update, SPIRE compatibility validation, performance benchmarking.

### Prerequisites

- rustls 0.23 stable with documented hybrid KEM support.
- `aws-lc-rs` FIPS build validated on the target Linux aarch64/x86_64 CI images.
- SPIRE 1.10+ (validates rustls 0.23 compatibility).

### Tracker suggestion

`GitHub issue: feat(armageddon-mesh): post-quantum TLS hybrid X25519+ML-KEM-768 [roadmap:vague-4]`

---

## Item 2 — FHE (Fully Homomorphic Encryption)

### Description

Enable computation on encrypted data so ARMAGEDDON policy rules can evaluate sensitive request attributes (e.g. `user_age > 18`, `credit_score > 600`) without decrypting them. The encrypted ciphertext is sent to the policy engine; the engine performs the comparison homomorphically and returns an encrypted boolean; the result is decrypted by the authorised party only.

The `tfhe-rs` library by Zama implements TFHE (Fast Fully Homomorphic Encryption over the Torus), which supports arbitrary Boolean circuits and integer arithmetic on encrypted integers up to 64 bits.

### External dependencies

- **`tfhe-rs`** (Zama, Apache-2.0): pure Rust, no C FFI. Latest stable: 0.9.x. API is maturing but not yet 1.0.
- **`concrete-core`** (Zama, deprecated in favour of `tfhe-rs`): do not use.
- **Compute capacity**: FHE operations require 10–1000x more CPU than plaintext equivalents. A single encrypted 8-bit integer addition takes ~1 ms on modern x86. Batch jobs (100 records) take seconds.

### Use case (FASO)

1. **Privacy-preserving policy**: ARMAGEDDON ARBITER evaluates `user_age >= 18` for content gating without ever seeing `user_age` in plaintext.
2. **Encrypted audit logs**: policy decisions logged as FHE ciphertexts — only the data owner can reconstruct the plaintext audit trail.
3. **Cross-shard aggregation in KAYA**: aggregate encrypted counters across shards without a trusted aggregator (G-Counter FHE extension).

### Performance constraints

FHE overhead makes it **non-viable on the hot request path** (sub-millisecond SLO). Suitable for:
- Async background policy jobs (batch risk scoring).
- Pre-computed encrypted attribute tokens cached per user session.
- Offline compliance reporting.

### Effort

**XL** — 6+ months (R&D phase). Includes: FHE scheme selection, key management design, encrypted attribute encoding format, policy DSL extension, performance profiling on representative FASO workloads.

### Prerequisites

- `tfhe-rs` 1.0 (API stability guarantee).
- Dedicated FHE compute budget (separate from gateway hot path).
- BL-3 `MultiAiProvider` merged (FHE may wrap an AI policy rule).

### Tracker suggestion

`GitHub issue: research(armageddon-arbiter): FHE policy evaluation via tfhe-rs [roadmap:vague-4]`

---

## Item 3 — ZK Proofs for Zero-Knowledge Authentication

### Description

Allow users to prove ownership of an attribute (e.g. "I am over 18", "I hold a valid FASO citizen credential", "I am a member of group X") without revealing the underlying value. Zero-knowledge proofs (ZKPs) provide cryptographic soundness: a verifier is convinced with overwhelming probability that the prover knows a witness, without learning the witness itself.

**Two implementation tracks**:

1. **zk-SNARKs via `arkworks` / `halo2`**: general-purpose ZKP circuits. High expressiveness, higher complexity.
2. **W3C Verifiable Credentials + BBS+ selective disclosure** (more realistic near-term): the user's identity provider issues a VC; the user presents a selective disclosure proof showing only the required attributes. Libraries: `bbs` (BBF), `did-key`.

Track 2 is recommended as the FASO near-term path since it builds on W3C standards already partially integrated with SPIRE SVIDs.

### External dependencies

- **`arkworks`** (arkworks-rs, MIT/Apache-2.0): modular ZKP framework. Groth16, Marlin, PLONK backends.
- **`halo2`** (ZCash/Electric Coin Co, MIT): recursive SNARKs; production-grade.
- **`bellman`** (ZCash, MIT): Groth16; mature but limited to a single proving system.
- **`bbs`** crate (BBS+ signatures for selective disclosure): early-stage.

### Use case (FASO)

1. **Age verification** at the gateway: a user proves `age >= 18` for adult-content APIs without the gateway seeing their date of birth.
2. **Organisational membership**: a contractor proves membership in an authorised organisation without revealing their full identity.
3. **Transaction privacy**: a payment initiator proves they hold sufficient balance without revealing the exact balance.

### Effort

**XL** — research phase (3–6 months for Track 1). Track 2 (W3C VC + BBS+) is **M** (~4–6 weeks) once a stable BBS+ Rust crate is available.

### Prerequisites

- Decision on Track 1 vs Track 2.
- For Track 2: SPIRE SVID extension to carry W3C VC claims (requires SPIRE 1.10+ plugin API).
- Legal/compliance review: ZKP-based age verification must comply with local Burkinabe e-ID regulations.

### Tracker suggestion

`GitHub issue: research(armageddon-veil): ZK proof-based attribute verification [roadmap:vague-4]`

---

## Item 4 — AI Multi-Model Intelligent Orchestration

### Description

Extends [`MultiAiProvider`](../armageddon-forge/src/pingora/engines/multi_ai_provider.rs) (shipped in BL-3) with intelligent routing based on real-time provider signals:

- **Latency-aware routing**: maintain an exponentially-weighted moving average (EWMA) of per-provider response latency; prefer the fastest provider for time-sensitive paths.
- **Cost-aware routing**: associate a cost-per-call with each provider (e.g. Anthropic API cost > Ollama local cost); route cheap providers first for bulk classification.
- **Confidence-based routing**: use the ensemble score variance as a confidence signal; when variance is high, route to a more expensive/accurate model for a second opinion.
- **Reinforcement-learning router**: replace the static `RequestRouter` with a bandit-style RL agent (epsilon-greedy or Thompson sampling) that learns which provider performs best for each request class.
- **Circuit breaker per provider**: integrate `armageddon-retry` circuit breaker per provider so a flapping provider is automatically excluded from the ensemble for a configurable cooldown window.

### External dependencies

- **BL-3 `MultiAiProvider`** merged (prerequisite).
- **`linfa`** (Rust ML toolkit): linear bandits, logistic regression.
- Per-provider Prometheus metrics (`armageddon_ai_provider_latency_seconds`, `armageddon_ai_provider_calls_total`) stable across restarts (already implemented in `llm_provider.rs`).

### Use case (FASO)

1. Route high-risk requests (score > 0.8) to the highest-accuracy provider (Claude), and low-risk bulk traffic to Ollama local to save API costs.
2. During Anthropic API brownouts, automatically fall to Ollama without operator intervention.
3. Track which provider catches the most confirmed attacks to continuously improve routing weights.

### Effort

**M** — 4–6 weeks post BL-3 merge. Includes: metrics stabilisation, EWMA routing implementation, circuit breaker integration, RL router prototype (epsilon-greedy), A/B test validation.

### Prerequisites

- BL-3 `MultiAiProvider` merged and deployed.
- Per-provider metrics stable for at least 2 weeks in staging.
- `armageddon-retry` circuit breaker API exposed as a library (already done in Phase 7).

### Tracker suggestion

`GitHub issue: feat(armageddon-forge,ai): intelligent multi-provider orchestration with RL router [roadmap:vague-4]`

---

## Item 5 — Byzantine Fault-Tolerant xDS (BFT xDS)

### Description

The current `xds-controller` is replicated via Raft (per-shard via `openraft` 0.10.0-alpha in the KAYA layer). Raft tolerates crash failures (f failures with 2f+1 nodes) but **not Byzantine failures** — a compromised or malicious xDS replica can push arbitrary cluster configurations to all ARMAGEDDON gateways.

BFT xDS would replicate the xDS control plane with Byzantine fault tolerance:
- **PBFT** (Practical Byzantine Fault Tolerance): O(n²) message complexity; 4 nodes tolerate 1 Byzantine fault.
- **HoneyBadger BFT** (asynchronous BFT, no timing assumptions): more robust under network partition.
- **Tendermint/CometBFT**: BFT consensus with economic finality; more complex but battle-tested.

The xDS server becomes a BFT state machine where each replica must receive a quorum of identical cluster-update proposals before forwarding to the gateway.

### External dependencies

- **No production-ready Rust BFT library** exists as of 2026-04. Options:
  - Port `hotstuff-rs` (HotStuff algorithm, MIT): experimental.
  - Wrap `tendermint-rs` (Informal Systems, Apache-2.0): requires Tendermint node deployment.
  - Build PBFT directly: well-understood algorithm, ~3k lines of Rust.
- **SPIRE SVID attestation** for xDS replica identity (already available in ARMAGEDDON).
- **gRPC streaming** between BFT replicas and gateways (already used by `armageddon-xds`).

### Use case (FASO)

Multi-region FASO deployment (Ouagadougou + Bobo-Dioulasso + Paris CDN edge): if a regional xDS controller is compromised by a supply-chain attack or insider threat, it cannot unilaterally redirect traffic. A 4-replica BFT cluster (tolerates 1 Byzantine fault) ensures configuration integrity even if one controller is actively malicious.

### Risk

- **Performance**: BFT consensus adds 1–3 round-trip latencies per config update. Acceptable for control-plane updates (seconds-scale), not for data-plane routing.
- **Operational complexity**: BFT requires 3f+1 replicas (7 for f=2); operational burden is significantly higher than Raft (2f+1).
- **Protocol selection**: no clear Rust ecosystem winner yet. Building PBFT from scratch risks subtle correctness bugs.

### Effort

**XL** — 3–6 months. Includes: BFT library evaluation/selection, protocol implementation or integration, xDS state machine adaptation, multi-region test harness, formal safety proof (or TLA+ model for the consensus protocol).

### Prerequisites

- Multi-region FASO deployment topology finalised (minimum 3 geographic regions).
- xDS controller API stabilised (no breaking changes for 6+ months).
- Dedicated security engineering review of the chosen BFT protocol.
- Threat model document confirming Byzantine failure scenarios are in scope.

### Tracker suggestion

`GitHub issue: research(armageddon-xds): Byzantine fault-tolerant xDS multi-region consensus [roadmap:vague-4]`

---

## Summary table

| # | Item | Effort | Prérequis clés | Tracker label |
|---|------|--------|---------------|---------------|
| 1 | Post-Quantum TLS (ML-KEM hybrid) | L (8–12 sem.) | rustls 0.23 stable, aws-lc-rs FIPS | `roadmap:vague-4` |
| 2 | FHE computation-on-encrypted-data | XL (6+ mois) | tfhe-rs 1.0, FHE compute budget | `roadmap:vague-4` |
| 3 | ZK proofs for auth | XL (3–6 mois) / M Track 2 | SPIRE VC extension, legal review | `roadmap:vague-4` |
| 4 | AI multi-model orchestration | M (4–6 sem.) | BL-3 merged, metrics stable | `roadmap:vague-4` |
| 5 | BFT xDS multi-region | XL (3–6 mois) | Multi-region topology, Rust BFT lib | `roadmap:vague-4` |

*Dernière mise à jour : 2026-04-24 — Session Backlog BL-4*
