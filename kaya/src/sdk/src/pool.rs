//! Connection pool for KAYA clients.


use tokio::sync::Semaphore;

use crate::client::KayaClient;
use crate::{ClientConfig, SdkError};

/// A simple connection pool.
pub struct ConnectionPool {
    config: ClientConfig,
    semaphore: Semaphore,
}

impl ConnectionPool {
    pub fn new(config: ClientConfig) -> Self {
        let pool_size = config.pool_size;
        Self {
            config,
            semaphore: Semaphore::new(pool_size),
        }
    }

    /// Acquire a connection from the pool.
    pub async fn acquire(&self) -> Result<PooledConnection, SdkError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| SdkError::PoolExhausted)?;

        let client = KayaClient::connect(&self.config).await?;
        Ok(PooledConnection { client })
    }
}

/// A connection borrowed from the pool.
pub struct PooledConnection {
    pub client: KayaClient,
}
