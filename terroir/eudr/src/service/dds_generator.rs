// SPDX-License-Identifier: AGPL-3.0-or-later
//! DDS generator (cf. ADR-004).
//!
//! Builds:
//!   - `payload_json`   : DDS payload conforming to the EU TRACES NT v1.4
//!     schema as a stable internal projection (see ADR-004 —
//!     adapter-versioned strategy).
//!   - `payload_sha256` : SHA-256 hex of the canonical payload.
//!   - `evidence_pdf`   : PDF bytes (printpdf) with the mandatory Hansen +
//!     JRC TMF citations (LICENSES-GEO §2.2 §3.2).

use anyhow::{Context, Result};
use printpdf::*;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::BufWriter;
use uuid::Uuid;

use crate::dto::GenerateDdsRequest;
use crate::repository::EudrValidationRow;

/// Mandatory Hansen citation per LICENSES-GEO.md §2.2.
pub const CITATION_HANSEN: &str = "Hansen, M.C. et al. 2013. \"High-Resolution Global Maps of 21st-Century Forest Cover Change.\" Science 342: 850-853. https://glad.umd.edu/dataset/global-forest-change (CC BY 4.0).";

/// Mandatory JRC citation per LICENSES-GEO.md §3.2.
pub const CITATION_JRC: &str = "Vancutsem, C. et al. 2021. \"Long-term (1990-2019) monitoring of forest cover changes in the humid tropics.\" Science Advances 7, eabe1603. https://forobs.jrc.ec.europa.eu/TMF (CC BY 4.0).";

#[derive(Debug, Serialize)]
pub struct GeneratedDds {
    pub payload: serde_json::Value,
    pub payload_sha256: String,
    pub evidence_pdf: Vec<u8>,
}

/// Build the DDS payload + evidence PDF.
pub fn generate(
    dds_id: Uuid,
    validation: &EudrValidationRow,
    req: &GenerateDdsRequest,
    operator_eori: &str,
) -> Result<GeneratedDds> {
    let payload = json!({
        "ddsId": dds_id,
        "schemaVersion": "1.4",
        "operator": {
            "eori": operator_eori,
            "country": req.country_iso2,
        },
        "product": {
            "hsCode": req.hs_code,
            "quantity": req.quantity,
            "unit": req.unit,
        },
        "plot": {
            "parcelId": validation.parcel_id,
            "polygonHash": validation.polygon_hash,
            "harvestPeriod": req.harvest_period,
            "deforestationCheck": {
                "status": validation.status,
                "overlapHa": validation.deforestation_overlap_ha,
                "datasetVersion": validation.dataset_version,
                "evidenceUrl": validation.evidence_url,
            },
        },
        "datasetAttribution": {
            "hansen": CITATION_HANSEN,
            "jrc": CITATION_JRC,
        },
        "createdAt": chrono::Utc::now().to_rfc3339(),
    });

    let canonical = serde_json::to_vec(&payload).context("serialize DDS payload for SHA-256")?;
    let mut h = Sha256::new();
    h.update(&canonical);
    let payload_sha256 = hex::encode(h.finalize());

    let evidence_pdf = render_evidence_pdf(dds_id, validation, req, operator_eori)
        .context("render DDS evidence PDF")?;

    Ok(GeneratedDds {
        payload,
        payload_sha256,
        evidence_pdf,
    })
}

fn render_evidence_pdf(
    dds_id: Uuid,
    validation: &EudrValidationRow,
    req: &GenerateDdsRequest,
    operator_eori: &str,
) -> Result<Vec<u8>> {
    let (doc, page1, layer1) = PdfDocument::new("TERROIR DDS", Mm(210.0), Mm(297.0), "evidence");
    let layer = doc.get_page(page1).get_layer(layer1);
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .context("load Helvetica font")?;
    let bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .context("load Helvetica-Bold font")?;

    layer.use_text(
        "TERROIR — Due Diligence Statement (Draft)",
        14.0,
        Mm(20.0),
        Mm(280.0),
        &bold,
    );
    layer.use_text(
        format!("DDS ID: {dds_id}"),
        10.0,
        Mm(20.0),
        Mm(270.0),
        &font,
    );
    layer.use_text(
        format!("Operator EORI: {operator_eori}"),
        10.0,
        Mm(20.0),
        Mm(263.0),
        &font,
    );
    layer.use_text(
        format!("HS Code: {} ({} {})", req.hs_code, req.quantity, req.unit),
        10.0,
        Mm(20.0),
        Mm(256.0),
        &font,
    );
    layer.use_text(
        format!("Country: {}", req.country_iso2),
        10.0,
        Mm(20.0),
        Mm(249.0),
        &font,
    );
    layer.use_text(
        format!("Harvest period: {}", req.harvest_period),
        10.0,
        Mm(20.0),
        Mm(242.0),
        &font,
    );
    layer.use_text("Deforestation check", 12.0, Mm(20.0), Mm(225.0), &bold);
    layer.use_text(
        format!("Status: {}", validation.status),
        10.0,
        Mm(20.0),
        Mm(218.0),
        &font,
    );
    layer.use_text(
        format!("Overlap (ha): {:.4}", validation.deforestation_overlap_ha),
        10.0,
        Mm(20.0),
        Mm(211.0),
        &font,
    );
    layer.use_text(
        format!("Dataset version: {}", validation.dataset_version),
        10.0,
        Mm(20.0),
        Mm(204.0),
        &font,
    );
    layer.use_text(
        format!("Polygon SHA-256: {}", validation.polygon_hash),
        7.0,
        Mm(20.0),
        Mm(197.0),
        &font,
    );

    layer.use_text(
        "Dataset attribution (CC BY 4.0)",
        12.0,
        Mm(20.0),
        Mm(60.0),
        &bold,
    );
    layer.use_text(CITATION_HANSEN, 7.0, Mm(20.0), Mm(50.0), &font);
    layer.use_text(CITATION_JRC, 7.0, Mm(20.0), Mm(35.0), &font);
    layer.use_text(
        "Generated by terroir-eudr (FASO DIGITALISATION) — AGPL-3.0-or-later.",
        7.0,
        Mm(20.0),
        Mm(20.0),
        &font,
    );

    let mut out = BufWriter::new(Vec::new());
    doc.save(&mut out).context("save PDF")?;
    out.into_inner().context("flush PDF buffer")
}
