// SPDX-License-Identifier: AGPL-3.0-or-later
import { useParams, Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  useParcel,
  useEudrValidation,
  useDds,
  useSubmitDds,
} from '../hooks/useParcels';
import { ParcelMap } from '../components/Map';
import { EudrStatusBanner } from '../components/EudrStatusBadge';
import { LoadingSkeleton } from '../components/LoadingSkeleton';

export function ParcelDetail() {
  const { id } = useParams<{ id: string }>();
  const { t } = useTranslation();
  const { data: parcel, isLoading } = useParcel(id);
  const { data: eudr } = useEudrValidation(id);
  const { data: dds } = useDds(id);
  const submitDds = useSubmitDds();

  if (isLoading || !parcel) {
    return <LoadingSkeleton height={400} />;
  }

  return (
    <div>
      <Link
        to="/parcels"
        style={{ color: 'var(--terroir-savane)', display: 'inline-block', marginBottom: 8 }}
      >
        {t('terroir.parcel_detail.back')}
      </Link>
      <h1 style={{ marginTop: 0 }}>
        {t('terroir.parcel_detail.title')} — {parcel.crop_type} ({parcel.surface_ha.toFixed(2)} ha)
      </h1>

      <EudrStatusBanner status={parcel.eudr_status} />

      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 16 }}>
        <div className="card">
          <h2 style={{ marginTop: 0 }}>{t('terroir.parcel_detail.section_map')}</h2>
          <ParcelMap
            parcels={[parcel]}
            center={[parcel.centroid.lat, parcel.centroid.lon]}
            zoom={14}
            height={420}
          />
        </div>

        <div>
          <div className="card" style={{ marginBottom: 16 }}>
            <h2 style={{ marginTop: 0 }}>
              {t('terroir.parcel_detail.section_eudr')}
            </h2>
            {eudr ? (
              <dl style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '6px 12px', margin: 0 }}>
                <dt>{t('terroir.eudr.hansen_loss')}</dt>
                <dd>{eudr.hansen_loss_year ?? '—'}</dd>
                <dt>{t('terroir.eudr.jrc_eufo_hit')}</dt>
                <dd>{eudr.jrc_eufo_2020_hit ? '⚠ Oui' : 'Non'}</dd>
                {eudr.evidence_url && (
                  <>
                    <dt>{t('terroir.parcel_detail.evidence_link')}</dt>
                    <dd>
                      <a href={eudr.evidence_url} target="_blank" rel="noopener noreferrer">
                        ↗
                      </a>
                    </dd>
                  </>
                )}
                {eudr.rejection_reason && (
                  <>
                    <dt>{t('terroir.eudr.rejection_reason')}</dt>
                    <dd>{eudr.rejection_reason}</dd>
                  </>
                )}
                {eudr.reviewer_actor && (
                  <>
                    <dt>{t('terroir.eudr.reviewer')}</dt>
                    <dd className="mono" style={{ fontSize: 11 }}>{eudr.reviewer_actor}</dd>
                  </>
                )}
              </dl>
            ) : (
              <p style={{ color: 'var(--terroir-text-muted)' }}>—</p>
            )}
          </div>

          <div className="card">
            <h2 style={{ marginTop: 0 }}>
              {t('terroir.parcel_detail.section_dds')}
            </h2>
            {dds ? (
              <>
                <p style={{ fontSize: 12 }}>
                  Réf : <span className="mono">{dds.reference}</span>
                </p>
                <p style={{ fontSize: 12 }}>
                  Statut : <strong>{dds.status}</strong>
                </p>
                {dds.pdf_url && (
                  <iframe
                    title="DDS PDF preview"
                    src={dds.pdf_url}
                    style={{ width: '100%', height: 240, border: '1px solid var(--terroir-border)', borderRadius: 4 }}
                  />
                )}
                {dds.traces_nt_id ? (
                  <p style={{ fontSize: 12 }}>
                    TRACES NT : <span className="mono">{dds.traces_nt_id}</span>
                  </p>
                ) : (
                  <button
                    className="btn-primary"
                    onClick={() => submitDds.mutate(dds.id)}
                    disabled={submitDds.isPending || parcel.eudr_status !== 'validated'}
                    style={{ marginTop: 8 }}
                  >
                    {submitDds.isPending
                      ? t('terroir.parcel_detail.submitting')
                      : t('terroir.parcel_detail.submit_traces_nt')}
                  </button>
                )}
              </>
            ) : (
              <p style={{ color: 'var(--terroir-text-muted)' }}>
                {t('terroir.parcel_detail.no_dds')}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
