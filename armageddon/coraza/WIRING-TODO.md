# Coraza WAF wiring — TODO (next sprint, ~1-2h)

## État actuel

- ✅ Module WASM compilé : `coraza-waf.wasm` (16 MB, OWASP CRS v4.10.0)
- ✅ Config inline : `coraza.conf` (CRS PL=1 + 5 règles FASO custom)
- ✅ Build script : `build.sh` (TinyGo 0.34 + tag CORAZA_PROXY_WASM_TAG=0.6.0)
- ❌ **Wiring Rust côté ARMAGEDDON** : pas fait

## Plan d'intégration (3 étapes)

### Étape 1 — Charger le `.wasm` dans `wasm_adapter.rs`

Fichier : `INFRA/armageddon/armageddon-forge/src/pingora/engines/wasm_adapter.rs`

Ce module gère déjà les engines WASM via le proxy-wasm SDK. Ajouter :

```rust
// Au boot de l'adapter
fn load_coraza_module(path: &Path) -> Result<EngineKind> {
    let bytes = fs::read(path)?;
    let engine = WasmEngine::compile(&bytes)?;
    let coraza_cfg_path = path.parent().unwrap().join("coraza.conf");
    let coraza_cfg = fs::read_to_string(&coraza_cfg_path)?;
    Ok(EngineKind::Waf {
        engine,
        config: coraza_cfg,
    })
}
```

### Étape 2 — Créer `WafFilter` dans `filters/waf.rs`

Fichier NEW : `INFRA/armageddon/armageddon-forge/src/pingora/filters/waf.rs`

```rust
pub struct WafFilter {
    wasm: Arc<WasmAdapter>,
    fail_closed_on_load_error: bool,
    paranoia_level: u8,
}

#[async_trait]
impl ForgeFilter for WafFilter {
    fn name(&self) -> &'static str { "waf" }

    async fn on_request(&self, session: &mut Session, ctx: &mut RequestCtx) -> Decision {
        let verdict = self.wasm.evaluate(session, EngineKind::Waf).await;
        match verdict {
            Ok(EngineVerdict::Allow) => Decision::Continue,
            Ok(EngineVerdict::Deny { status, rule_id }) => {
                metrics::WAF_BLOCKS_TOTAL
                    .with_label_values(&[&rule_id.to_string()])
                    .inc();
                Decision::ShortCircuit(error_response(status, "WAF rule triggered"))
            }
            Ok(EngineVerdict::Skipped) => Decision::Continue,
            Err(_) if self.fail_closed_on_load_error =>
                Decision::ShortCircuit(error_response(503, "WAF unavailable")),
            Err(_) => Decision::Continue,
        }
    }
}
```

### Étape 3 — Charger le filtre dans `armageddon/src/main.rs`

Après la construction du RouterFilter, lire la config waf:

```rust
let waf_filter: Option<SharedFilter> = if config.gateway.waf.enabled {
    let wasm_path = PathBuf::from(&config.gateway.waf.wasm_module);
    match WasmAdapter::load_coraza(&wasm_path) {
        Ok(adapter) => {
            tracing::info!(path = %wasm_path.display(), "Coraza WAF loaded");
            Some(Arc::new(WafFilter::new(Arc::new(adapter), &config.gateway.waf)))
        }
        Err(e) => {
            tracing::error!(err = %e, "Coraza WAF failed to load");
            if config.gateway.waf.fail_closed_on_load_error {
                anyhow::bail!("WAF required but failed to load");
            }
            None
        }
    }
} else { None };

let mut filters: Vec<SharedFilter> = vec![router_filter];
if let Some(waf) = waf_filter {
    filters.insert(0, waf);  // WAF en premier (avant routing)
}

let gw_cfg = PingoraGatewayConfig {
    filters,
    ...
};
```

## Validation post-wiring

Une fois les 3 étapes faites, les 7 tests `test.fixme()` dans `tests-e2e/tests/17-owasp-top10/owasp.spec.ts` deviennent passants :

- A03: SQLi blocked → 403
- A03: XSS blocked → 403
- A03: command injection blocked → 403
- A03: NoSQL injection blocked → 403
- A10: SSRF AWS metadata blocked → 403
- A10: SSRF 127.0.0.1 blocked → 403
- A10: SSRF private CIDR blocked → 403

Latence attendue après wiring : +1-3 ms p99 sur le hot path (CRS PL=1 charge légère).

## Prérequis manquants pour tester

- `INFRA/armageddon/armageddon-forge/src/pingora/engines/wasm_adapter.rs` doit avoir une méthode `load_coraza()` ou équivalent
- La config `armageddon-config::WafConfig` n'existe peut-être pas encore (à créer)

## Effort estimé

1.5–2h de Rust + cargo build (3-4 min) + 30 min de validation E2E.
