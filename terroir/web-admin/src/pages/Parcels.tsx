// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useParcels, useCooperatives } from '../hooks/useParcels';
import { ParcelMap } from '../components/Map';
import { LoadingSkeleton } from '../components/LoadingSkeleton';
import { EudrStatusBadge } from '../components/EudrStatusBadge';
import type { EudrStatus } from '../api/types';

export function Parcels() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [statusFilter, setStatusFilter] = useState<EudrStatus | ''>('');
  const [coopId, setCoopId] = useState<string>('');

  const { data: coops } = useCooperatives();
  const { data, isLoading, error } = useParcels({
    page_size: 200,
    eudr_status: (statusFilter as EudrStatus) || undefined,
    cooperative_id: coopId || undefined,
  });

  return (
    <div>
      <h1 style={{ marginTop: 0 }}>{t('terroir.parcels.title')}</h1>

      <div className="card" style={{ marginBottom: 16 }}>
        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value as EudrStatus | '')}
          >
            <option value="">{t('terroir.parcels.filter_status_all')}</option>
            <option value="validated">
              {t('terroir.parcels.filter_status_validated')}
            </option>
            <option value="rejected">
              {t('terroir.parcels.filter_status_rejected')}
            </option>
            <option value="escalated">
              {t('terroir.parcels.filter_status_escalated')}
            </option>
            <option value="pending">{t('terroir.parcels.filter_status_pending')}</option>
          </select>
          <select value={coopId} onChange={(e) => setCoopId(e.target.value)}>
            <option value="">{t('terroir.producers.filter_cooperative')}</option>
            {coops?.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {isLoading ? (
        <LoadingSkeleton height={600} />
      ) : error ? (
        <div className="banner banner--error">
          {error instanceof Error ? error.message : 'unknown'}
        </div>
      ) : !data || data.items.length === 0 ? (
        <div className="card">
          <p>{t('terroir.parcels.empty')}</p>
        </div>
      ) : (
        <>
          <ParcelMap
            parcels={data.items}
            onParcelClick={(parcelId) => navigate(`/parcels/${parcelId}`)}
          />
          <div style={{ marginTop: 12, fontSize: 12, color: 'var(--terroir-text-muted)' }}>
            {data.items.length} parcelle(s) affichée(s) — {data.total} total
          </div>
          <div className="card" style={{ marginTop: 16 }}>
            <table>
              <thead>
                <tr>
                  <th>Crop</th>
                  <th>Surface (ha)</th>
                  <th>EUDR</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {data.items.slice(0, 50).map((p) => (
                  <tr
                    key={p.id}
                    onClick={() => navigate(`/parcels/${p.id}`)}
                    data-testid={`parcel-row-${p.id}`}
                  >
                    <td>{p.crop_type}</td>
                    <td>{p.surface_ha.toFixed(2)}</td>
                    <td>
                      <EudrStatusBadge status={p.eudr_status} />
                    </td>
                    <td className="mono" style={{ fontSize: 11 }}>
                      {p.id.slice(0, 8)}…
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
