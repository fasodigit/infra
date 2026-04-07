# ARMAGEDDON

Security gateway for the FASO DIGITALISATION project. Full Envoy replacement built in Rust.

## Architecture -- Pentagon

ARMAGEDDON is not just a WAF. It is the complete gateway handling HTTP/gRPC proxying, TLS termination, authentication, authorization, and multi-engine security analysis.

```
                                 Internet
                                    |
                              [ ARMAGEDDON ]
                                    |
                        +-----------+-----------+
                        |       F O R G E       |
                        |  (Pingora HTTP Proxy)  |
                        |  TLS / JWT / CORS /   |
                        |  Routing / H2 / gRPC  |
                        +-----------+-----------+
                                    |
                    +-------+-------+-------+-------+
                    |       |       |       |       |
                SENTINEL  ARBITER  ORACLE  AEGIS    AI
                  IPS      WAF     ML      Rego   Threat
                 GeoIP    CRS v4  ONNX   Regorus  Intel
                  DLP     Aho-C.  22feat  deny-by  Prompt
                 JA3             default  Inject
                 Rate
                    |       |       |       |       |
                    +-------+-------+-------+-------+
                                    |
                              [ N E X U S ]
                            (Brain / Scorer)
                             Aggregation &
                              Correlation
                                    |
                        +-----------+-----------+
                        |       V E I L         |
                        |   Header Masking &    |
                        |   Security Injection  |
                        +-----------+-----------+
                                    |
                               [ Upstream ]
```

### Engines

| Engine | Crate | Role |
|--------|-------|------|
| FORGE | `armageddon-forge` | HTTP/1.1, HTTP/2, gRPC, WebSocket proxy (Pingora). TLS termination, JWT authn, ext_authz, CORS, routing. |
| SENTINEL | `armageddon-sentinel` | IPS (200+ signatures), DLP, GeoIP (MaxMind), JA3, rate limiting (sliding window). |
| ARBITER | `armageddon-arbiter` | WAF with OWASP CRS v4, 4 paranoia levels, Aho-Corasick pattern matching. |
| ORACLE | `armageddon-oracle` | AI anomaly detection, 22-feature extraction, ONNX Runtime inference. |
| AEGIS | `armageddon-aegis` | Rego policy engine (Regorus), deny-by-default authorization. |
| NEXUS | `armageddon-nexus` | Brain: decision aggregation, cross-engine correlation, composite scoring. Connects to KAYA via RESP3+. |
| VEIL | `armageddon-veil` | Response header masking (strip server fingerprints), security header injection (HSTS, CSP, etc.). |
| WASM | `armageddon-wasm` | Plugin runtime via Wasmtime. Custom security plugins in sandboxed WebAssembly. |
| AI | `armageddon-ai` | Threat intelligence feeds, prompt injection detection. |

### Supporting Crates

| Crate | Role |
|-------|------|
| `armageddon-common` | Shared types, error types, `SecurityEngine` trait, `RequestContext`, `Decision`. |
| `armageddon-config` | YAML config loader, hot-reload, xDS ADS client for dynamic config from xDS Controller. |

## Gateway Responsibilities (replacing Envoy)

- **jwt_authn**: JWT ES384 validation via JWKS from auth-ms (cache 300s)
- **ext_authz**: OPA sidecar integration (gRPC, fail-closed)
- **CORS**: per-platform origin configuration
- **Rate limiting**: sliding window per IP / JWT subject
- **GraphQL routing**: `/api/graphql` -> DGS Gateway
- **Connect RPC routing**: `/api/connect/*` -> backends
- **Health checks**: HTTP + gRPC periodic per upstream
- **Circuit breakers**: per cluster upstream
- **Outlier detection**: automatic ejection on consecutive 5xx
- **xDS client**: receive dynamic config from xDS Controller via gRPC ADS

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run -- --config config/armageddon.yaml
```

## Configuration

See `config/armageddon.yaml` for the full configuration reference.

## Key Dependencies

- [Pingora](https://github.com/cloudflare/pingora) 0.8 -- HTTP proxy framework (Cloudflare)
- [Aho-Corasick](https://github.com/BurntSushi/aho-corasick) -- Multi-pattern string matching
- [MaxMind DB](https://github.com/oschwald/maxminddb-rust) -- GeoIP lookups
- [Regorus](https://github.com/microsoft/regorus) -- Rego policy engine (Microsoft)
- [Wasmtime](https://github.com/bytecodealliance/wasmtime) -- WebAssembly runtime
- [Tonic](https://github.com/hyperium/tonic) -- gRPC framework
- [jsonwebtoken](https://github.com/Keats/jsonwebtoken) -- JWT validation
