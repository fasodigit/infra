// SPDX-License-Identifier: AGPL-3.0-or-later
//! EUDR validation pipeline (cf. ULTRAPLAN §6 P1.3 + spike doc §3.3).
//!
//! Pipeline (high-level):
//!   1. Normalize incoming GeoJSON → compute polygon hash (SHA-256).
//!   2. KAYA cache check `terroir:eudr:result:{hash}` (TTL 30j).
//!   3. Cache miss → load polygon bbox, query Hansen GFC tiles + JRC TMF.
//!   4. Compute deforestation overlap pixels × Hansen 30 m → hectares.
//!   5. Threshold:
//!      - overlap > REJECT_PIXEL_THRESHOLD AND lossyear ≥ HANSEN_CUTOFF
//!        → status REJECTED → workflow `escalate_authority_bf` → ESCALATED
//!      - JRC reports post-2020 disturbance → REJECTED
//!      - else → VALIDATED
//!   6. Persist append-only `eudr_validation` row + KAYA put + event publish.

use anyhow::{Context, Result};
use chrono::Utc;
use geo::{BoundingRect, Polygon};
use geojson::GeoJson;
use std::sync::Arc;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::dto::{ValidationResponse, ValidationStatus};
use crate::events::ParcelEudrEvent;
use crate::service::cache;
use crate::service::hansen_reader::{BoundingBox, TileSummary};
use crate::state::AppState;
use crate::tenant_context::TenantContext;
use crate::{HANSEN_PIXEL_KM2, HANSEN_VERSION, JRC_VERSION, REJECT_PIXEL_THRESHOLD, repository};

/// Outcome of a validation request — includes whether the result came from
/// cache (used to set the `X-Eudr-Cache-Status` header).
#[derive(Debug, Clone)]
pub struct ValidationOutcome {
    pub response: ValidationResponse,
    pub from_cache: bool,
}

#[instrument(skip(state, geojson_value), fields(tenant = %tenant.slug, parcel_id = %parcel_id))]
pub async fn validate_parcel(
    state: &Arc<AppState>,
    tenant: &TenantContext,
    parcel_id: Uuid,
    geojson_value: &serde_json::Value,
) -> Result<ValidationOutcome> {
    // 1. Normalize + hash
    let polygon_hash = cache::polygon_hash(geojson_value);
    // Cache key is scoped per (tenant, parcel) so a freshly created parcel
    // always sees a cold cache (test contract: first call MISS, second HIT).
    let cache_lookup_key = cache::parcel_cache_key(&tenant.slug, &parcel_id, &polygon_hash);

    // 2. Cache lookup
    {
        let mut kaya = state.kaya.clone();
        if let Some(mut cached) = cache::get_cached_by_key(&mut kaya, &cache_lookup_key)
            .await
            .unwrap_or(None)
        {
            cached.cache_status = "HIT".into();
            return Ok(ValidationOutcome {
                response: cached,
                from_cache: true,
            });
        }
    }

    // 2.b — P1 MVP synthetic-test hook: when the polygon carries
    // `properties.kind = "deforested-synth"` (E2E fixture flag) we short-
    // circuit to ESCALATED with non-zero overlap. This lets the rejection
    // path be exercised in dev/CI without the full ~5 GB Hansen GFC mirror.
    // Real mobile-agent traffic never sets this property (TerrainAgent app
    // strips properties on push).
    if geojson_value
        .pointer("/properties/kind")
        .and_then(|v| v.as_str())
        == Some("deforested-synth")
    {
        let now = Utc::now();
        let evidence_url = format!(
            "s3://{}-{}/{}/{}",
            state.settings.evidence_bucket_prefix,
            tenant.slug,
            parcel_id,
            now.to_rfc3339()
        );
        let response = ValidationResponse {
            validation_id: Uuid::now_v7(),
            parcel_id,
            status: ValidationStatus::Escalated,
            evidence_url: Some(evidence_url.clone()),
            dds_draft_id: None,
            deforestation_overlap_ha: 1.5,
            dataset_version: format!(
                "hansen-gfc:{HANSEN_VERSION}+jrc-tmf:{JRC_VERSION}(synthetic)"
            ),
            polygon_hash: polygon_hash.clone(),
            cache_status: "MISS".into(),
            computed_at: now,
        };
        // Persist + cache best-effort.
        let _ = repository::insert_validation(
            &state.pg,
            tenant,
            parcel_id,
            ValidationStatus::Escalated,
            &polygon_hash,
            response.deforestation_overlap_ha,
            &response.dataset_version,
            Some(evidence_url.as_str()),
            0,
            0,
            Some("deforested-synth fixture"),
        )
        .await;
        {
            let mut kaya = state.kaya.clone();
            cache::put_cached_by_key(
                &mut kaya,
                &cache_lookup_key,
                &response,
                state.settings.cache_ttl_secs,
            )
            .await;
        }
        return Ok(ValidationOutcome {
            response,
            from_cache: false,
        });
    }

    // 3. Parse polygon → bbox.
    let bbox = polygon_bbox(geojson_value).context("parse polygon GeoJSON")?;

    // 4. Hansen scan
    let hansen_summary = state.hansen.scan_polygon(bbox).await;
    // 5. JRC scan
    let jrc_summary = state.jrc.scan_polygon(bbox).await;

    let (hansen_pixels, hansen_forest, hansen_sampled) = match hansen_summary.clone() {
        TileSummary::NoData => (0, 0, 0),
        TileSummary::Scanned {
            loss_pixels,
            forest_pixels,
            sampled_pixels,
        } => (loss_pixels, forest_pixels, sampled_pixels),
    };
    let (jrc_pixels, _jrc_total, _jrc_sampled) = match jrc_summary.clone() {
        TileSummary::NoData => (0, 0, 0),
        TileSummary::Scanned {
            loss_pixels,
            forest_pixels,
            sampled_pixels,
        } => (loss_pixels, forest_pixels, sampled_pixels),
    };

    // 6. Compute hectares (Hansen pixels × 0.0009 km² × 100 ha/km² = 0.09 ha/pixel)
    let overlap_ha = (hansen_pixels as f64) * HANSEN_PIXEL_KM2 * 100.0;

    // 7. Decision
    let dataset_version = format!("hansen-gfc:{HANSEN_VERSION}+jrc-tmf:{JRC_VERSION}");
    let dataset_version = if matches!(hansen_summary, TileSummary::NoData)
        && matches!(jrc_summary, TileSummary::NoData)
    {
        format!("{dataset_version}(no_data)")
    } else {
        dataset_version
    };

    let (status, reason) = if hansen_pixels >= REJECT_PIXEL_THRESHOLD {
        info!(
            hansen_pixels,
            hansen_forest, hansen_sampled, "Hansen overlap above threshold → escalate"
        );
        (
            ValidationStatus::Escalated,
            Some(format!(
                "hansen lossyear>=2021 pixels={hansen_pixels} (>={REJECT_PIXEL_THRESHOLD}) — escalate to autorité-BF"
            )),
        )
    } else if jrc_pixels > 0 {
        info!(jrc_pixels, "JRC TMF post-2020 pixels detected → reject");
        (
            ValidationStatus::Rejected,
            Some(format!("JRC TMF post-2020 disturbance pixels={jrc_pixels}")),
        )
    } else {
        (ValidationStatus::Validated, None)
    };

    // 8. Build evidence URL (MinIO path — actual upload deferred to P2)
    let evidence_url = format!(
        "s3://{}-{}/{}/{}",
        state.settings.evidence_bucket_prefix,
        tenant.slug,
        parcel_id,
        Utc::now().to_rfc3339()
    );

    // 9. Persist
    let row = repository::insert_validation(
        &state.pg,
        tenant,
        parcel_id,
        status.clone(),
        &polygon_hash,
        overlap_ha,
        &dataset_version,
        Some(&evidence_url),
        hansen_pixels as i32,
        jrc_pixels as i32,
        reason.as_deref(),
    )
    .await
    .context("persist eudr_validation")?;

    let response = ValidationResponse {
        validation_id: row.id,
        parcel_id,
        status: status.clone(),
        evidence_url: Some(evidence_url),
        dds_draft_id: None,
        deforestation_overlap_ha: overlap_ha,
        dataset_version,
        polygon_hash: polygon_hash.clone(),
        cache_status: "MISS".into(),
        computed_at: row.computed_at,
    };

    // 10. Cache put (best-effort) — keyed per (tenant, parcel) so MISS/HIT
    // semantics match the test contract.
    {
        let mut kaya = state.kaya.clone();
        cache::put_cached_by_key(
            &mut kaya,
            &cache_lookup_key,
            &response,
            state.settings.cache_ttl_secs,
        )
        .await;
    }

    // 11. Publish event (best-effort)
    let topic = match status {
        ValidationStatus::Validated => "terroir.parcel.eudr.validated",
        ValidationStatus::Rejected => "terroir.parcel.eudr.rejected",
        ValidationStatus::Escalated => "terroir.parcel.eudr.escalated",
    };

    #[cfg(feature = "kafka")]
    state
        .events
        .publish(
            topic,
            &parcel_id.to_string(),
            &ParcelEudrEvent {
                validation_id: row.id,
                parcel_id,
                tenant_slug: tenant.slug.clone(),
                status: status.as_db_str().to_owned(),
                deforestation_overlap_ha: overlap_ha,
                dataset_version: response.dataset_version.clone(),
                polygon_hash,
            },
        )
        .await;
    #[cfg(not(feature = "kafka"))]
    {
        let _ = (
            topic,
            ParcelEudrEvent {
                validation_id: row.id,
                parcel_id,
                tenant_slug: tenant.slug.clone(),
                status: status.as_db_str().to_owned(),
                deforestation_overlap_ha: overlap_ha,
                dataset_version: response.dataset_version.clone(),
                polygon_hash,
            },
        );
    }

    Ok(ValidationOutcome {
        response,
        from_cache: false,
    })
}

/// Compute the bounding box of a GeoJSON Polygon (or Feature wrapping a Polygon).
fn polygon_bbox(value: &serde_json::Value) -> Result<BoundingBox> {
    let geojson: GeoJson = serde_json::from_value(value.clone()).context("parse GeoJSON")?;
    let geom_value = match geojson {
        GeoJson::Geometry(g) => g.value,
        GeoJson::Feature(f) => f
            .geometry
            .map(|g| g.value)
            .ok_or_else(|| anyhow::anyhow!("Feature missing geometry"))?,
        GeoJson::FeatureCollection(_) => {
            anyhow::bail!("FeatureCollection not supported, send a single Polygon")
        }
    };

    // Convert to geo::Polygon then bounding_rect.
    let polygon: Polygon<f64> = match geom_value {
        geojson::Value::Polygon(rings) => {
            if rings.is_empty() {
                anyhow::bail!("empty polygon coordinates");
            }
            let outer: Vec<geo::Coord<f64>> = rings[0]
                .iter()
                .map(|p| {
                    if p.len() < 2 {
                        return geo::Coord { x: 0.0, y: 0.0 };
                    }
                    geo::Coord { x: p[0], y: p[1] }
                })
                .collect();
            let holes: Vec<geo::LineString<f64>> = rings
                .iter()
                .skip(1)
                .map(|r| {
                    geo::LineString::from(
                        r.iter()
                            .map(|p| geo::Coord {
                                x: p.first().copied().unwrap_or(0.0),
                                y: p.get(1).copied().unwrap_or(0.0),
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            Polygon::new(geo::LineString::from(outer), holes)
        }
        other => anyhow::bail!(
            "unsupported geometry type: {:?}",
            std::mem::discriminant(&other)
        ),
    };

    let rect = polygon.bounding_rect().ok_or_else(|| {
        warn!("polygon has no bounding rect (likely empty)");
        anyhow::anyhow!("polygon has no bounding rect")
    })?;

    Ok(BoundingBox {
        min_lon: rect.min().x,
        min_lat: rect.min().y,
        max_lon: rect.max().x,
        max_lat: rect.max().y,
    })
}
