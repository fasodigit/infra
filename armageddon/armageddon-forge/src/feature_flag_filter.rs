// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Middleware ARMAGEDDON — injection `X-Faso-Features` aux requêtes upstream.
//
// Lit `X-User-Id` dans la requête entrante, interroge KAYA (RESP3) sur la clef
// `ff:prod:<sha(user_id)>`, parse le JSON en liste de flags ON, et injecte
// `X-Faso-Features: flag1,flag2` avant de passer la requête au service aval.
//
// En cas d'erreur KAYA (down, timeout) : fallback gracieux — la requête
// traverse sans header de flags. Les services aval doivent tous avoir un
// comportement par défaut sans feature-flag.
//
// Le wire-up final (Tower layer insérée dans le stack FORGE) est fait
// séparément dans `armageddon-forge/src/lib.rs` / `armageddon/src/main.rs`
// pour séparer responsabilité (ce fichier ne modifie ni l'un ni l'autre).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use http::{HeaderValue, Request};
use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use tower::{Layer, Service};
use tracing::{debug, warn};

/// Nom du header injecté upstream.
pub const FEATURE_HEADER: &str = "X-Faso-Features";
/// Header porteur de l'identité utilisateur (peuplé en amont par le filtre JWT).
pub const USER_ID_HEADER: &str = "X-User-Id";
/// Préfixe de clef KAYA (on reste alignés avec le backend Java).
pub const KEY_PREFIX: &str = "ff:prod:";

/// Source de flags abstraite — facilite les tests avec KAYA mockée.
#[async_trait::async_trait]
pub trait FlagSource: Send + Sync + 'static {
    /// Renvoie la valeur brute cachée pour la clef demandée, ou None si
    /// injoignable / miss. Doit être rapide (timeout 20 ms max).
    async fn get(&self, key: &str) -> Option<String>;
}

/// Source de flags par défaut : client KAYA via crate `redis` (RESP3).
pub struct KayaFlagSource {
    client: redis::Client,
    timeout: Duration,
}

impl KayaFlagSource {
    pub fn new(url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: redis::Client::open(url)?,
            timeout: Duration::from_millis(20),
        })
    }
}

#[async_trait::async_trait]
impl FlagSource for KayaFlagSource {
    async fn get(&self, key: &str) -> Option<String> {
        let fut = async {
            let mut con = self.client.get_multiplexed_async_connection().await.ok()?;
            let val: Option<String> = con.get(key).await.ok()?;
            val
        };
        match tokio::time::timeout(self.timeout, fut).await {
            Ok(v) => v,
            Err(_) => {
                warn!("KAYA flag lookup timed out after {:?}", self.timeout);
                None
            }
        }
    }
}

/// Le filtre lui-même, réutilisable au sein d'une `tower::ServiceBuilder`.
#[derive(Clone)]
pub struct FeatureFlagFilter<S> {
    inner: S,
    source: Arc<dyn FlagSource>,
}

impl<S> FeatureFlagFilter<S> {
    pub fn new(inner: S, source: Arc<dyn FlagSource>) -> Self {
        Self { inner, source }
    }

    /// Construit la clef KAYA déterministe à partir du header user-id.
    pub fn cache_key(user_id: &str) -> String {
        let digest = Sha256::digest(user_id.as_bytes());
        let hex = hex::encode(&digest[..8]);
        format!("{KEY_PREFIX}{hex}")
    }

    /// Parse le payload KAYA (JSON `{"flag": bool, ...}`) → liste CSV des flags ON.
    pub fn parse_flags(raw: &str) -> String {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) else {
            return String::new();
        };
        let obj = match &val {
            serde_json::Value::Object(m) => m,
            _ => return String::new(),
        };
        let mut names: Vec<&str> = obj
            .iter()
            .filter_map(|(k, v)| if v.as_bool().unwrap_or(false) { Some(k.as_str()) } else { None })
            .collect();
        names.sort_unstable();
        names.join(",")
    }
}

/// `Layer` pour composer avec `ServiceBuilder::layer(...)`.
pub struct FeatureFlagLayer {
    source: Arc<dyn FlagSource>,
}

impl FeatureFlagLayer {
    pub fn new(source: Arc<dyn FlagSource>) -> Self {
        Self { source }
    }
}

impl<S> Layer<S> for FeatureFlagLayer {
    type Service = FeatureFlagFilter<S>;
    fn layer(&self, inner: S) -> Self::Service {
        FeatureFlagFilter::new(inner, Arc::clone(&self.source))
    }
}

impl<S, B> Service<Request<B>> for FeatureFlagFilter<S>
where
    S: Service<Request<B>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        // Security: strip any client-supplied `X-Faso-Features` header before
        // any branching. This header is a gateway-issued attestation — we must
        // never let an inbound value leak to upstream, even on bypass paths
        // (no user id, KAYA miss, empty flags, parse failure). If KAYA has
        // flags to inject, the `insert` below re-adds a trusted value.
        req.headers_mut().remove(FEATURE_HEADER);

        let source = Arc::clone(&self.source);
        // Clone l'inner pour satisfaire les contraintes de lifetime sur la future boxée.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            if let Some(user_id) = req
                .headers()
                .get(USER_ID_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_owned())
            {
                let key = FeatureFlagFilter::<S>::cache_key(&user_id);
                match source.get(&key).await {
                    Some(raw) => {
                        let csv = FeatureFlagFilter::<S>::parse_flags(&raw);
                        if !csv.is_empty() {
                            if let Ok(hv) = HeaderValue::from_str(&csv) {
                                req.headers_mut().insert(FEATURE_HEADER, hv);
                                debug!(target: "faso.flags", user = %user_id, flags = %csv, "flags injected");
                            }
                        }
                    }
                    None => {
                        debug!(target: "faso.flags", user = %user_id, "KAYA miss/unreachable, skip");
                    }
                }
            }
            inner.call(req).await
        })
    }
}

// ================================ TESTS =====================================

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, Response, StatusCode};
    use std::convert::Infallible;
    use std::sync::Mutex;
    use tower::{service_fn, ServiceExt};

    /// Mock de FlagSource : renvoie un payload pré-calculé ou None pour
    /// simuler une indisponibilité KAYA.
    struct MockSource {
        payload: Mutex<Option<String>>,
        hits: Mutex<Vec<String>>,
    }

    impl MockSource {
        fn new(payload: Option<&str>) -> Self {
            Self {
                payload: Mutex::new(payload.map(String::from)),
                hits: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl FlagSource for MockSource {
        async fn get(&self, key: &str) -> Option<String> {
            self.hits.lock().unwrap().push(key.to_owned());
            self.payload.lock().unwrap().clone()
        }
    }

    fn echo_service() -> impl Service<Request<()>, Response = Response<String>, Error = Infallible, Future = impl Send + 'static> + Clone + Send + 'static {
        service_fn(|req: Request<()>| async move {
            let features = req
                .headers()
                .get(FEATURE_HEADER)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_owned();
            Ok::<_, Infallible>(Response::builder().status(StatusCode::OK).body(features).unwrap())
        })
    }

    #[tokio::test]
    async fn injects_header_on_kaya_hit() {
        let source = Arc::new(MockSource::new(Some(
            r#"{"poulets.new-checkout":true,"auth.webauthn-beta":false,"etat-civil.pdf-v2":true}"#,
        )));
        let svc = FeatureFlagLayer::new(source.clone()).layer(echo_service());

        let req = Request::builder()
            .uri("/api/x")
            .header(USER_ID_HEADER, "eleveur-42")
            .body(())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        // Liste triée : etat-civil.pdf-v2, poulets.new-checkout
        assert_eq!(res.body(), "etat-civil.pdf-v2,poulets.new-checkout");
        assert_eq!(source.hits.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn fallback_gracefully_when_kaya_down() {
        let source = Arc::new(MockSource::new(None)); // simule KAYA unreachable
        let svc = FeatureFlagLayer::new(source).layer(echo_service());

        let req = Request::builder()
            .uri("/api/x")
            .header(USER_ID_HEADER, "u1")
            .body(())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.body().is_empty(), "no feature header on KAYA miss");
    }

    #[tokio::test]
    async fn skips_when_no_user_id() {
        let source = Arc::new(MockSource::new(Some(r#"{"x":true}"#)));
        let svc = FeatureFlagLayer::new(source.clone()).layer(echo_service());

        let req = Request::builder().uri("/api/x").body(()).unwrap();
        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.body().is_empty());
        assert_eq!(source.hits.lock().unwrap().len(), 0, "KAYA ne doit pas être appelée");
    }

    // ---------------------------------------------------------------- intégration
    //
    // Test d'intégration : au lieu de démarrer un vrai conteneur KAYA (trop lourd),
    // on utilise un implémenteur de `FlagSource` basé sur un HashMap, qui
    // reproduit le comportement attendu d'un client `redis` utilitaire de test.
    // En CI, ce test peut être réactivé avec un vrai KAYA via la feature
    // `integration-kaya` (non activée par défaut).

    #[tokio::test]
    async fn integration_kaya_mocked_via_redis_like_store() {
        use std::collections::HashMap;

        struct InMemoryKaya(Mutex<HashMap<String, String>>);
        #[async_trait::async_trait]
        impl FlagSource for InMemoryKaya {
            async fn get(&self, k: &str) -> Option<String> {
                self.0.lock().unwrap().get(k).cloned()
            }
        }

        let mut seed = HashMap::new();
        // Clef déterministe attendue côté backend Java = sha256(user_id)[..8].hex()
        let key = FeatureFlagFilter::<()>::cache_key("citoyen-01");
        seed.insert(
            key,
            r#"{"etat-civil.pdf-v2":true,"auth.webauthn-beta":true}"#.to_string(),
        );
        let source = Arc::new(InMemoryKaya(Mutex::new(seed)));

        let svc = FeatureFlagLayer::new(source).layer(echo_service());

        let req = Request::builder()
            .uri("/api/etat-civil")
            .header(USER_ID_HEADER, "citoyen-01")
            .body(())
            .unwrap();
        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.body(), "auth.webauthn-beta,etat-civil.pdf-v2");
    }

    // ---------------------------------------------------------------- security
    //
    // Regression tests : un client ne doit JAMAIS pouvoir forger le header
    // `X-Faso-Features` lui-même. Le filtre doit scrubber l'entrée avant
    // toute décision d'injection, et seules les valeurs issues de KAYA
    // doivent atteindre l'upstream.

    #[tokio::test]
    async fn scrubs_client_supplied_header_when_no_user_id() {
        let source = Arc::new(MockSource::new(Some(r#"{"x":true}"#)));
        let svc = FeatureFlagLayer::new(source.clone()).layer(echo_service());

        // Pas de X-User-Id → branche "early skip". Le header spoofé doit
        // tout de même être retiré.
        let req = Request::builder()
            .uri("/api/x")
            .header(FEATURE_HEADER, "spoofed")
            .body(())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.body().is_empty(),
            "client-supplied X-Faso-Features must be stripped, got {:?}",
            res.body()
        );
        assert_eq!(
            source.hits.lock().unwrap().len(),
            0,
            "KAYA ne doit pas être appelée sans X-User-Id"
        );
    }

    #[tokio::test]
    async fn scrubs_client_supplied_header_on_kaya_miss() {
        // KAYA simule un miss / unreachable : aucun payload à injecter.
        let source = Arc::new(MockSource::new(None));
        let svc = FeatureFlagLayer::new(source).layer(echo_service());

        let req = Request::builder()
            .uri("/api/x")
            .header(USER_ID_HEADER, "u-miss")
            .header(FEATURE_HEADER, "spoofed,evil.flag")
            .body(())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.body().is_empty(),
            "sur miss KAYA, le header spoofé doit être supprimé, got {:?}",
            res.body()
        );
    }

    #[tokio::test]
    async fn scrubs_then_reinjects_on_happy_path() {
        // KAYA a des flags → le spoof doit être écrasé par la valeur signée.
        let source = Arc::new(MockSource::new(Some(
            r#"{"poulets.new-checkout":true,"etat-civil.pdf-v2":true}"#,
        )));
        let svc = FeatureFlagLayer::new(source).layer(echo_service());

        let req = Request::builder()
            .uri("/api/x")
            .header(USER_ID_HEADER, "eleveur-42")
            .header(FEATURE_HEADER, "poulets.admin-tools,auth.bypass-mfa")
            .body(())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        // L'upstream ne doit voir QUE la valeur dérivée de KAYA, triée.
        assert_eq!(res.body(), "etat-civil.pdf-v2,poulets.new-checkout");
    }
}
