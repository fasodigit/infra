// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Mirror TypeScript des DTOs Rust exposés par terroir-core (:8830) via
// ARMAGEDDON :8080/api/terroir/core/*. Synchronisation manuelle au P1 ;
// schéma OpenAPI auto-généré prévu en P1.H (cycle-fix → cargo-utoipa).

export type Uuid = string;
export type IsoDateTime = string;

export interface Producer {
  id: Uuid;
  cooperative_id: Uuid;
  full_name: string;
  nin: string; // National Identification Number — masquer côté UI (truncate)
  phone: string; // E.164 — masquer côté UI
  department: string;
  province: string;
  region: string;
  kyc_status: KycStatus;
  mfa_enrolled: boolean;
  created_at: IsoDateTime;
  updated_at: IsoDateTime;
}

export type KycStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'suspended'
  | 'expired';

export interface ProducerListResponse {
  items: Producer[];
  page: number;
  page_size: number;
  total: number;
}

export interface ProducerListQuery {
  page?: number;
  page_size?: number;
  search?: string;
  cooperative_id?: Uuid;
  kyc_status?: KycStatus;
}

export interface Parcel {
  id: Uuid;
  producer_id: Uuid;
  cooperative_id: Uuid;
  crop_type: CropType;
  surface_ha: number;
  geojson: GeoJsonPolygon;
  centroid: { lat: number; lon: number };
  eudr_status: EudrStatus;
  last_validation_id?: Uuid;
  created_at: IsoDateTime;
  updated_at: IsoDateTime;
}

export type CropType =
  | 'cocoa'
  | 'coffee'
  | 'cotton'
  | 'cashew'
  | 'shea'
  | 'sesame'
  | 'other';

export type EudrStatus =
  | 'pending'
  | 'validated'
  | 'rejected'
  | 'escalated'
  | 'expired';

export interface GeoJsonPolygon {
  type: 'Polygon' | 'MultiPolygon';
  coordinates: number[][][] | number[][][][];
}

export interface ParcelListResponse {
  items: Parcel[];
  page: number;
  page_size: number;
  total: number;
}

export interface ParcelListQuery {
  page?: number;
  page_size?: number;
  cooperative_id?: Uuid;
  eudr_status?: EudrStatus;
  bbox?: [number, number, number, number]; // [minLon, minLat, maxLon, maxLat]
}

export interface EudrValidation {
  id: Uuid;
  parcel_id: Uuid;
  status: EudrStatus;
  decision_at: IsoDateTime;
  // Sources légales (cf. INFRA/terroir/docs/LICENSES-GEO.md)
  hansen_loss_year?: number; // Hansen Global Forest Change v1.11 (CC BY 4.0)
  jrc_eufo_2020_hit: boolean; // JRC EU forest map 2020 (CC BY 4.0)
  evidence_url?: string; // signed MinIO URL (objects/eudr/<id>.geojson.gz)
  rejection_reason?: string;
  reviewer_actor?: string;
}

export interface Dds {
  id: Uuid;
  parcel_id: Uuid;
  producer_id: Uuid;
  reference: string;
  status: DdsStatus;
  pdf_url?: string; // signed MinIO URL — preview iframe
  traces_nt_id?: string; // EU TRACES NT submission ID
  submitted_at?: IsoDateTime;
  created_at: IsoDateTime;
}

export type DdsStatus =
  | 'draft'
  | 'pending_submission'
  | 'submitted'
  | 'accepted'
  | 'rejected';

export interface AuditEvent {
  id: Uuid;
  timestamp: IsoDateTime;
  actor_id: Uuid;
  actor_email: string;
  action: string; // e.g. "producer.kyc.approved"
  resource_type: string; // "producer" | "parcel" | "dds" | …
  resource_id: Uuid;
  trace_id?: string; // W3C traceparent → Jaeger
  metadata?: Record<string, unknown>;
}

export interface AuditListResponse {
  items: AuditEvent[];
  page: number;
  page_size: number;
  total: number;
}

export interface AuditQuery {
  page?: number;
  page_size?: number;
  actor_id?: Uuid;
  action?: string;
  resource_type?: string;
  from?: IsoDateTime;
  to?: IsoDateTime;
}

export interface DashboardKpis {
  producers_total: number;
  parcels_validated: number;
  dds_submitted: number;
  eudr_alerts_rejected: number;
  series_7d: Array<{
    date: string; // YYYY-MM-DD
    producers: number;
    parcels: number;
    dds: number;
    rejected: number;
  }>;
}

export interface Cooperative {
  id: Uuid;
  union_id: Uuid;
  name: string;
  region: string;
  producers_count: number;
}
