// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tonic client wrapper around `terroir.core.v1.CoreService`.
//!
//! Used by the EUDR validator to fetch the parcel polygon (GeoJSON / WKT)
//! when callers don't pass it inline.

use anyhow::{Context, Result};
use tonic::transport::Channel;

use crate::core_proto::{
    GetParcelPolygonRequest, ParcelPolygon, core_service_client::CoreServiceClient,
};

/// Lazily-created gRPC client to terroir-core.
#[derive(Clone)]
pub struct CoreGrpcClient {
    endpoint: String,
}

impl CoreGrpcClient {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    async fn connect(&self) -> Result<CoreServiceClient<Channel>> {
        let channel = Channel::from_shared(self.endpoint.clone())
            .context("invalid TERROIR_CORE_GRPC_URL")?
            .connect()
            .await
            .context("connect to terroir-core gRPC")?;
        Ok(CoreServiceClient::new(channel))
    }

    /// Fetch a parcel polygon — returns GeoJSON + WKT.
    pub async fn get_parcel_polygon(
        &self,
        tenant_slug: &str,
        parcel_id: &str,
    ) -> Result<ParcelPolygon> {
        let mut client = self.connect().await?;
        let req = GetParcelPolygonRequest {
            tenant_slug: tenant_slug.to_owned(),
            parcel_id: parcel_id.to_owned(),
        };
        let resp = client
            .get_parcel_polygon(req)
            .await
            .context("call CoreService.GetParcelPolygon")?;
        Ok(resp.into_inner())
    }
}
