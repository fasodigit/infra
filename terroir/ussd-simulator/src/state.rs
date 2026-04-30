// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — état partagé (KAYA pool + helpers)
//
// Le KAYA en P0 ne supporte que `GET`, `SET`, `DEL` (RESP3 string-only ;
// HASH/LIST/SCAN/EXPIRE/SETEX seront ajoutés plus tard). On encode donc :
//   - les sessions USSD comme blobs JSON sous une seule clé string ;
//   - l'historique SMS par MSISDN comme tableau JSON sous une clé string,
//     read-modify-write avec tronquage `SMS_LIST_MAX` (LTRIM-like local) ;
//   - un index global `terroir:ussd:simulator:_index` (set sérialisé JSON)
//     listant toutes les clés posées par le simulator, pour le wipe
//     `/admin/clear` (équivalent SCAN+DEL pour KAYA actuel).
//
// Le TTL est appliqué côté client (timestamp `expires_at` dans le blob
// JSON ; les readers filtrent les entrées expirées). C'est un compromis
// volontaire pour P0 — quand KAYA implémente `EXPIRE`/`SETEX` on passera
// au modèle natif sans changer la surface API.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::{KAYA_PREFIX, KAYA_URL_DEFAULT, KAYA_URL_ENV};

/// TTL par défaut d'une session USSD (5 min, plus large que les 30-60s
/// opérateur pour permettre debug humain entre étapes Playwright).
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(300);

/// TTL OTP (5 min — cohérent avec ANSSI/CNIB recommandations OTP courts).
pub const DEFAULT_OTP_TTL: Duration = Duration::from_secs(300);

/// TTL log SMS dans KAYA (24h — assez pour debug E2E sans saturer mémoire).
pub const DEFAULT_SMS_TTL: Duration = Duration::from_secs(24 * 3600);

/// Borne max éléments par liste SMS par MSISDN (LTRIM client-side).
pub const SMS_LIST_MAX: usize = 50;

/// Clé KAYA hébergeant l'index des clés posées (pour wipe).
fn index_key() -> String {
    format!("{}:_index", KAYA_PREFIX)
}

/// Wrapper TTL appliqué côté client (KAYA P0 sans EXPIRE).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TtlEnvelope<T> {
    expires_at_unix: u64,
    payload: T,
}

impl<T> TtlEnvelope<T> {
    fn new(payload: T, ttl: Duration) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            expires_at_unix: now + ttl.as_secs(),
            payload,
        }
    }

    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now >= self.expires_at_unix
    }
}

/// État partagé Axum.
///
/// `kaya` est optionnel : si la connexion échoue au démarrage le simulator
/// continue à servir mais retourne 503 sur les routes qui en dépendent
/// (utile pour `cargo check` et tests unitaires sans KAYA up).
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    kaya: Option<Mutex<ConnectionManager>>,
    session_ttl: Duration,
    otp_ttl: Duration,
    sms_ttl: Duration,
}

impl AppState {
    pub async fn from_env() -> Self {
        let url = std::env::var(KAYA_URL_ENV).unwrap_or_else(|_| KAYA_URL_DEFAULT.to_string());
        let kaya = match Self::connect(&url).await {
            Ok(cm) => {
                debug!(target: "terroir-ussd-simulator", url = %url, "KAYA connected");
                Some(Mutex::new(cm))
            }
            Err(err) => {
                warn!(
                    target: "terroir-ussd-simulator",
                    url = %url,
                    error = %err,
                    "KAYA unavailable — simulator will return 503 on stateful routes"
                );
                None
            }
        };
        Self {
            inner: Arc::new(AppStateInner {
                kaya,
                session_ttl: DEFAULT_SESSION_TTL,
                otp_ttl: DEFAULT_OTP_TTL,
                sms_ttl: DEFAULT_SMS_TTL,
            }),
        }
    }

    async fn connect(url: &str) -> anyhow::Result<ConnectionManager> {
        let client = redis::Client::open(url).context("invalid KAYA url")?;
        let cm = ConnectionManager::new(client)
            .await
            .context("failed to open KAYA ConnectionManager")?;
        Ok(cm)
    }

    pub fn session_ttl(&self) -> Duration {
        self.inner.session_ttl
    }
    pub fn otp_ttl(&self) -> Duration {
        self.inner.otp_ttl
    }
    pub fn sms_ttl(&self) -> Duration {
        self.inner.sms_ttl
    }

    fn require_kaya(&self) -> anyhow::Result<&Mutex<ConnectionManager>> {
        self.inner
            .kaya
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("KAYA unavailable"))
    }

    // ------------- KAYA primitives (GET / SET / DEL) -------------
    //
    // Each primitive retries ONCE on transient failure. KAYA closes idle
    // connections after ~10s, and the `redis` crate's ConnectionManager
    // reconnects on the *next* call but surfaces the previous failure to
    // the current caller. Re-issuing the command after a brief pause
    // hits the freshly reconnected socket.

    pub async fn kaya_set_str(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let lock = self.require_kaya()?;
        let res = {
            let mut cm = lock.lock().await;
            cm.set::<_, _, ()>(key, value).await
        };
        match res {
            Ok(()) => {}
            Err(first_err) => {
                tracing::debug!(target: "terroir-ussd-simulator", error = %first_err, "KAYA SET retrying once");
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut cm = lock.lock().await;
                cm.set::<_, _, ()>(key, value)
                    .await
                    .context("KAYA SET failed")?;
            }
        }
        self.touch_index(key).await?;
        Ok(())
    }

    pub async fn kaya_get_str(&self, key: &str) -> anyhow::Result<Option<String>> {
        let lock = self.require_kaya()?;
        let res = {
            let mut cm = lock.lock().await;
            cm.get::<_, Option<String>>(key).await
        };
        match res {
            Ok(v) => Ok(v),
            Err(first_err) => {
                tracing::debug!(target: "terroir-ussd-simulator", error = %first_err, "KAYA GET retrying once");
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut cm = lock.lock().await;
                cm.get::<_, Option<String>>(key)
                    .await
                    .context("KAYA GET failed")
            }
        }
    }

    pub async fn kaya_del(&self, key: &str) -> anyhow::Result<()> {
        let lock = self.require_kaya()?;
        let res = {
            let mut cm = lock.lock().await;
            cm.del::<_, i64>(key).await
        };
        match res {
            Ok(_) => {}
            Err(first_err) => {
                tracing::debug!(target: "terroir-ussd-simulator", error = %first_err, "KAYA DEL retrying once");
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut cm = lock.lock().await;
                cm.del::<_, i64>(key).await.context("KAYA DEL failed")?;
            }
        }
        self.untrack_index(key).await?;
        Ok(())
    }

    // --------------- High-level helpers (TTL côté client) ---------------

    /// Stocke une valeur sérialisable avec enveloppe TTL.
    pub async fn put_ttl<T: Serialize>(
        &self,
        key: &str,
        value: T,
        ttl: Duration,
    ) -> anyhow::Result<()> {
        let env = TtlEnvelope::new(value, ttl);
        let raw = serde_json::to_string(&env).context("serialize TTL envelope")?;
        self.kaya_set_str(key, &raw).await
    }

    /// Récupère une valeur, en filtrant les entrées expirées (côté client).
    pub async fn get_ttl<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<T>> {
        let Some(raw) = self.kaya_get_str(key).await? else {
            return Ok(None);
        };
        let env: TtlEnvelope<T> = serde_json::from_str(&raw).context("deserialize TTL envelope")?;
        if env.is_expired() {
            // best-effort cleanup
            let _ = self.kaya_del(key).await;
            Ok(None)
        } else {
            Ok(Some(env.payload))
        }
    }

    /// Push capped : lit, prepend, tronque, réécrit.
    pub async fn list_lpush_capped<T: Serialize + for<'de> Deserialize<'de>>(
        &self,
        key: &str,
        item: T,
        cap: usize,
        ttl: Duration,
    ) -> anyhow::Result<()> {
        let mut existing: Vec<T> = self.get_ttl(key).await?.unwrap_or_default();
        existing.insert(0, item);
        if existing.len() > cap {
            existing.truncate(cap);
        }
        self.put_ttl(key, existing, ttl).await
    }

    /// LRANGE-like : lit la liste filtrée par TTL, retourne au plus `limit`.
    pub async fn list_range<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<T>> {
        let mut all: Vec<T> = self.get_ttl(key).await?.unwrap_or_default();
        all.truncate(limit);
        Ok(all)
    }

    // -------------- Index global (pour wipe / clear admin) --------------

    /// Ajoute une clé à l'index. Best-effort : l'index est lui-même un
    /// blob JSON `Vec<String>` sous `_index`. En cas de course on perd
    /// au pire une entrée, le wipe `/admin/clear` reste robuste car il
    /// itère sur l'index ET supprime l'index.
    async fn touch_index(&self, key: &str) -> anyhow::Result<()> {
        if key.ends_with(":_index") {
            return Ok(());
        }
        let idx_key = index_key();
        let mut set: HashSet<String> = match self.kaya_get_str_raw(&idx_key).await? {
            Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            None => HashSet::new(),
        };
        if set.insert(key.to_string()) {
            let raw = serde_json::to_string(&set).context("serialize index")?;
            self.kaya_set_raw_with_retry(&idx_key, &raw).await?;
        }
        Ok(())
    }

    async fn untrack_index(&self, key: &str) -> anyhow::Result<()> {
        if key.ends_with(":_index") {
            return Ok(());
        }
        let idx_key = index_key();
        let Some(raw) = self.kaya_get_str_raw(&idx_key).await? else {
            return Ok(());
        };
        let mut set: HashSet<String> = serde_json::from_str(&raw).unwrap_or_default();
        if set.remove(key) {
            let raw = serde_json::to_string(&set).context("serialize index")?;
            self.kaya_set_raw_with_retry(&idx_key, &raw).await?;
        }
        Ok(())
    }

    /// Direct SET with retry — avoids recursion through touch_index.
    async fn kaya_set_raw_with_retry(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let lock = self.require_kaya()?;
        let res = {
            let mut cm = lock.lock().await;
            cm.set::<_, _, ()>(key, value).await
        };
        match res {
            Ok(()) => Ok(()),
            Err(first_err) => {
                tracing::debug!(target: "terroir-ussd-simulator", error = %first_err, "KAYA index SET retrying once");
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut cm = lock.lock().await;
                cm.set::<_, _, ()>(key, value)
                    .await
                    .context("KAYA index SET failed")
            }
        }
    }

    /// Helper bas niveau sans touch_index (utilisé par les méthodes
    /// d'index pour casser la récursion). Retries on transient KAYA failure.
    async fn kaya_get_str_raw(&self, key: &str) -> anyhow::Result<Option<String>> {
        let lock = self.require_kaya()?;
        let res = {
            let mut cm = lock.lock().await;
            cm.get::<_, Option<String>>(key).await
        };
        match res {
            Ok(v) => Ok(v),
            Err(first_err) => {
                tracing::debug!(target: "terroir-ussd-simulator", error = %first_err, "KAYA index GET retrying once");
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut cm = lock.lock().await;
                cm.get::<_, Option<String>>(key)
                    .await
                    .context("KAYA GET failed")
            }
        }
    }

    /// Wipe : itère l'index, supprime chaque clé, supprime l'index.
    pub async fn wipe_all(&self) -> anyhow::Result<u64> {
        let idx_key = index_key();
        let Some(raw) = self.kaya_get_str_raw(&idx_key).await? else {
            return Ok(0);
        };
        let set: HashSet<String> = serde_json::from_str(&raw).unwrap_or_default();
        let mut count: u64 = 0;
        for key in &set {
            let n = self.kaya_del_raw_with_retry(key).await?;
            count += n.max(0) as u64;
        }
        let _ = self.kaya_del_raw_with_retry(&idx_key).await?;
        Ok(count)
    }

    /// Direct DEL with retry, used inside the wipe loop.
    async fn kaya_del_raw_with_retry(&self, key: &str) -> anyhow::Result<i64> {
        let lock = self.require_kaya()?;
        let res = {
            let mut cm = lock.lock().await;
            cm.del::<_, i64>(key).await
        };
        match res {
            Ok(n) => Ok(n),
            Err(first_err) => {
                tracing::debug!(target: "terroir-ussd-simulator", error = %first_err, "KAYA DEL retrying once");
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut cm = lock.lock().await;
                cm.del::<_, i64>(key).await.context("KAYA DEL failed")
            }
        }
    }
}
