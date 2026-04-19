// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Tonic implementation of the `faso.canary.v1.CanaryService` gRPC service.
//!
//! All RPCs delegate to [`CanaryOrchestrator`] which owns the state machine
//! and the xDS `ConfigStore` integration.
//!
//! # RPC contract
//!
//! | RPC             | Idempotent | Terminal |
//! |-----------------|-----------|---------|
//! | StartCanary     | No        | No      |
//! | PauseCanary     | Yes       | No      |
//! | AbortCanary     | Yes       | Yes     |
//! | PromoteCanary   | Yes       | Yes     |
//! | GetCanaryStatus | Yes (read) | N/A    |
//! | ListCanaries    | Yes (read) | N/A    |
//!
//! # Failure modes
//!
//! * **Unknown canary_id**: returns `Status::not_found`.
//! * **Mutating a terminal canary**: returns `Status::failed_precondition`.
//! * **Prometheus unreachable**: StartCanary succeeds; ticks will skip SLO checks.

use std::sync::Arc;
use std::time::Duration;

use prost_types::Timestamp;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::canary::{CanaryEntry, CanaryOrchestrator, SloConfig, Stage};
use crate::generated::canary::v1::{
    canary_service_server::CanaryService, AbortCanaryRequest, AbortCanaryResponse,
    CanaryStatus, GetCanaryStatusRequest, ListCanariesRequest, ListCanariesResponse,
    PauseCanaryRequest, PauseCanaryResponse, PromoteCanaryRequest, PromoteCanaryResponse,
    SloCompliance, StartCanaryRequest, StartCanaryResponse,
};
use crate::generated::canary::v1::Stage as ProtoStage;

/// Tonic gRPC service wrapping the `CanaryOrchestrator`.
pub struct CanaryGrpcService {
    orchestrator: Arc<CanaryOrchestrator>,
}

impl CanaryGrpcService {
    pub fn new(orchestrator: Arc<CanaryOrchestrator>) -> Self {
        Self { orchestrator }
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn stage_to_proto(s: Stage) -> i32 {
    match s {
        Stage::Stage1Pct => ProtoStage::Stage1Pct as i32,
        Stage::Stage10Pct => ProtoStage::Stage10Pct as i32,
        Stage::Stage50Pct => ProtoStage::Stage50Pct as i32,
        Stage::Promoted => ProtoStage::Promoted as i32,
        Stage::RolledBack => ProtoStage::RolledBack as i32,
        Stage::Paused => ProtoStage::Paused as i32,
    }
}

fn dt_to_proto(dt: chrono::DateTime<chrono::Utc>) -> Timestamp {
    Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn entry_to_status(entry: &CanaryEntry) -> CanaryStatus {
    CanaryStatus {
        canary_id: entry.canary_id.clone(),
        service: entry.service.clone(),
        image_tag: entry.image_tag.clone(),
        current_stage: stage_to_proto(entry.current_stage),
        current_weight_pct: entry.effective_weight_pct(),
        started_at: Some(dt_to_proto(entry.started_at)),
        stage_started_at: Some(dt_to_proto(entry.stage_started_at)),
        slo_compliance: entry.last_compliance.as_ref().map(|c| SloCompliance {
            observed_error_rate: c.observed_error_rate,
            observed_latency_p99_ms: c.observed_latency_p99_ms,
            within_budget: c.within_budget,
            measured_at: Some(dt_to_proto(c.measured_at)),
        }),
        rollback_reason: entry.rollback_reason.clone().unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// Tonic impl
// ---------------------------------------------------------------------------

#[tonic::async_trait]
impl CanaryService for CanaryGrpcService {
    /// Start a new canary deployment.
    async fn start_canary(
        &self,
        request: Request<StartCanaryRequest>,
    ) -> Result<Response<StartCanaryResponse>, Status> {
        let req = request.into_inner();

        if req.service.is_empty() {
            return Err(Status::invalid_argument("service must not be empty"));
        }
        if req.image_tag.is_empty() {
            return Err(Status::invalid_argument("image_tag must not be empty"));
        }

        let slo = req
            .slo
            .as_ref()
            .map(|s| SloConfig {
                error_rate_max: s.error_rate_max,
                latency_p99_max_ms: s.latency_p99_max_ms,
                prometheus_endpoint: s.prometheus_endpoint.clone(),
            })
            .unwrap_or_default();

        let min_stage_duration = Duration::from_secs(
            if req.min_stage_duration_secs == 0 {
                3600
            } else {
                req.min_stage_duration_secs
            },
        );

        let canary_id =
            self.orchestrator
                .start(&req.service, &req.image_tag, slo, min_stage_duration);

        info!(
            canary_id = %canary_id,
            service = %req.service,
            image_tag = %req.image_tag,
            "StartCanary gRPC called"
        );

        Ok(Response::new(StartCanaryResponse { canary_id }))
    }

    /// Pause a canary (halt tick-driven advancement; route weights unchanged).
    async fn pause_canary(
        &self,
        request: Request<PauseCanaryRequest>,
    ) -> Result<Response<PauseCanaryResponse>, Status> {
        let canary_id = request.into_inner().canary_id;

        match self.orchestrator.pause(&canary_id) {
            Ok(entry) => {
                info!(canary_id = %canary_id, "PauseCanary gRPC called");
                Ok(Response::new(PauseCanaryResponse {
                    status: Some(entry_to_status(&entry)),
                }))
            }
            Err(e) => {
                warn!(canary_id = %canary_id, error = %e, "PauseCanary failed");
                Err(Status::failed_precondition(e))
            }
        }
    }

    /// Abort a canary — force rollback to 0 % immediately.
    async fn abort_canary(
        &self,
        request: Request<AbortCanaryRequest>,
    ) -> Result<Response<AbortCanaryResponse>, Status> {
        let req = request.into_inner();
        let reason = if req.reason.is_empty() {
            "manual abort".to_string()
        } else {
            req.reason.clone()
        };

        match self.orchestrator.abort(&req.canary_id, &reason) {
            Ok(entry) => {
                info!(canary_id = %req.canary_id, reason = %reason, "AbortCanary gRPC called");
                Ok(Response::new(AbortCanaryResponse {
                    status: Some(entry_to_status(&entry)),
                }))
            }
            Err(e) => {
                warn!(canary_id = %req.canary_id, error = %e, "AbortCanary failed");
                Err(Status::not_found(e))
            }
        }
    }

    /// Force-promote a canary to 100 % regardless of SLO.
    async fn promote_canary(
        &self,
        request: Request<PromoteCanaryRequest>,
    ) -> Result<Response<PromoteCanaryResponse>, Status> {
        let canary_id = request.into_inner().canary_id;

        match self.orchestrator.promote(&canary_id) {
            Ok(entry) => {
                info!(canary_id = %canary_id, "PromoteCanary gRPC called");
                Ok(Response::new(PromoteCanaryResponse {
                    status: Some(entry_to_status(&entry)),
                }))
            }
            Err(e) => {
                warn!(canary_id = %canary_id, error = %e, "PromoteCanary failed");
                Err(Status::failed_precondition(e))
            }
        }
    }

    /// Get the current status of a canary.
    async fn get_canary_status(
        &self,
        request: Request<GetCanaryStatusRequest>,
    ) -> Result<Response<CanaryStatus>, Status> {
        let canary_id = request.into_inner().canary_id;

        match self.orchestrator.status(&canary_id) {
            Some(entry) => Ok(Response::new(entry_to_status(&entry))),
            None => Err(Status::not_found(format!(
                "canary {canary_id} not found"
            ))),
        }
    }

    /// List all canaries for a service.
    async fn list_canaries(
        &self,
        request: Request<ListCanariesRequest>,
    ) -> Result<Response<ListCanariesResponse>, Status> {
        let req = request.into_inner();
        let entries = self.orchestrator.list_for_service(&req.service);

        // Optional stage filter.
        let stage_filter: Option<i32> = req.stage_filter;
        let canaries = entries
            .iter()
            .filter(|e| {
                stage_filter
                    .map(|sf| stage_to_proto(e.current_stage) == sf)
                    .unwrap_or(true)
            })
            .map(|e| entry_to_status(e))
            .collect();

        Ok(Response::new(ListCanariesResponse { canaries }))
    }
}
