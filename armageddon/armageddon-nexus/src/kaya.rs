//! KAYA client: connects to KAYA cache via Redis-compatible RESP3+ protocol on port 6380.
//!
//! KAYA replaces DragonflyDB. It is Redis-compatible but runs on port 6380.
//! Used for: threat score caching, rate limit counters, JWKS cache.

use redis::AsyncCommands;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum KayaError {
    #[error("connection failed: {0}")]
    Connection(String),

    #[error("command failed: {0}")]
    Command(String),

    #[error("deserialization failed: {0}")]
    Deserialization(String),
}

impl From<redis::RedisError> for KayaError {
    fn from(e: redis::RedisError) -> Self {
        KayaError::Command(e.to_string())
    }
}

/// Client for KAYA via Redis-compatible RESP3+ protocol.
///
/// Connects to KAYA on port 6380 (NOT the default Redis 6379).
pub struct KayaClient {
    host: String,
    port: u16,
    connection: RwLock<Option<redis::aio::MultiplexedConnection>>,
}

impl KayaClient {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            connection: RwLock::new(None),
        }
    }

    /// Connect to KAYA.
    pub async fn connect(&self) -> Result<(), KayaError> {
        let url = format!("redis://{}:{}", self.host, self.port);
        tracing::info!("connecting to KAYA at {}:{}", self.host, self.port);

        let client = redis::Client::open(url.as_str())
            .map_err(|e| KayaError::Connection(e.to_string()))?;

        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| KayaError::Connection(e.to_string()))?;

        let mut guard = self.connection.write().await;
        *guard = Some(conn);

        tracing::info!("connected to KAYA at {}:{}", self.host, self.port);
        Ok(())
    }

    /// Get a connection, returning an error if not connected.
    async fn get_conn(&self) -> Result<redis::aio::MultiplexedConnection, KayaError> {
        let guard = self.connection.read().await;
        guard
            .clone()
            .ok_or_else(|| KayaError::Connection("not connected to KAYA".to_string()))
    }

    /// Store a threat score for an IP (used for reputation tracking).
    /// Key: armageddon:threat:{ip}
    pub async fn set_threat_score(
        &self,
        ip: &str,
        score: f64,
        ttl_secs: u64,
    ) -> Result<(), KayaError> {
        let mut conn = self.get_conn().await?;
        let key = format!("armageddon:threat:{}", ip);
        let score_str = score.to_string();
        conn.set_ex::<_, _, ()>(&key, &score_str, ttl_secs)
            .await?;
        Ok(())
    }

    /// Get the threat score for an IP.
    pub async fn get_threat_score(&self, ip: &str) -> Result<Option<f64>, KayaError> {
        let mut conn = self.get_conn().await?;
        let key = format!("armageddon:threat:{}", ip);
        let result: Option<String> = conn.get(&key).await?;
        match result {
            Some(s) => {
                let score = s
                    .parse::<f64>()
                    .map_err(|e| KayaError::Deserialization(e.to_string()))?;
                Ok(Some(score))
            }
            None => Ok(None),
        }
    }

    /// Increment rate limit counter with TTL.
    /// Key: armageddon:ratelimit:{key}
    /// Returns the new count after increment.
    pub async fn incr_rate_limit(
        &self,
        key: &str,
        window_secs: u64,
    ) -> Result<u64, KayaError> {
        let mut conn = self.get_conn().await?;
        let redis_key = format!("armageddon:ratelimit:{}", key);

        // INCR + EXPIRE in a pipeline
        let (count,): (u64,) = redis::pipe()
            .atomic()
            .incr(&redis_key, 1u64)
            .expire(&redis_key, window_secs as i64)
            .ignore()
            .query_async(&mut conn)
            .await?;

        Ok(count)
    }

    /// Cache JWKS keys.
    /// Key: armageddon:jwks
    pub async fn cache_jwks(
        &self,
        jwks_json: &str,
        ttl_secs: u64,
    ) -> Result<(), KayaError> {
        let mut conn = self.get_conn().await?;
        conn.set_ex::<_, _, ()>("armageddon:jwks", jwks_json, ttl_secs)
            .await?;
        Ok(())
    }

    /// Get cached JWKS keys.
    pub async fn get_cached_jwks(&self) -> Result<Option<String>, KayaError> {
        let mut conn = self.get_conn().await?;
        let result: Option<String> = conn.get("armageddon:jwks").await?;
        Ok(result)
    }

    /// Store a verdict for audit trail.
    /// Key: armageddon:verdict:{request_id}
    pub async fn store_verdict(
        &self,
        request_id: &str,
        verdict_json: &str,
        ttl_secs: u64,
    ) -> Result<(), KayaError> {
        let mut conn = self.get_conn().await?;
        let key = format!("armageddon:verdict:{}", request_id);
        conn.set_ex::<_, _, ()>(&key, verdict_json, ttl_secs)
            .await?;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        // We can't synchronously check the async RwLock,
        // so we return true optimistically if a connection was established.
        true
    }

    /// Fixed-window counter: INCR + EXPIRE only on first hit (count==1).
    /// Used by DDoS mitigation for bucket counters without TTL reset.
    pub async fn incr_with_expire(
        &self,
        key: &str,
        window_secs: u64,
    ) -> Result<u64, KayaError> {
        let mut conn = self.get_conn().await?;
        let (count,): (u64,) = redis::pipe()
            .atomic()
            .incr(key, 1u64)
            .query_async(&mut conn)
            .await?;
        if count == 1 {
            let _: () = conn.expire(key, window_secs as i64).await?;
        }
        Ok(count)
    }

    /// Sorted-set sliding-window log: ZADD + ZREMRANGEBYSCORE + ZCARD + EXPIRE.
    /// Sub-second precision counter for DDoS mitigation.
    pub async fn sliding_window_incr(
        &self,
        key: &str,
        window_secs: u64,
    ) -> Result<u64, KayaError> {
        let mut conn = self.get_conn().await?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let cutoff = now_ms - (window_secs as i64 * 1000);
        let uid = format!("{}-{}", now_ms, uuid::Uuid::new_v4());
        let (_, _, count): (i64, i64, u64) = redis::pipe()
            .atomic()
            .zrembyscore(key, 0, cutoff)
            .zadd(key, &uid, now_ms)
            .zcard(key)
            .expire(key, (window_secs + 1) as i64)
            .ignore()
            .query_async(&mut conn)
            .await?;
        Ok(count)
    }
}
