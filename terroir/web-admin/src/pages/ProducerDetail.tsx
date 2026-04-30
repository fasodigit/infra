// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  useProducer,
  useProducerParcels,
  useApproveKyc,
  useSuspendProducer,
  useResetMfa,
} from '../hooks/useProducers';
import { KycBadge } from '../components/KycBadge';
import { EudrStatusBadge } from '../components/EudrStatusBadge';
import { LoadingSkeleton } from '../components/LoadingSkeleton';

export function ProducerDetail() {
  const { id } = useParams<{ id: string }>();
  const { t } = useTranslation();
  const { data: producer, isLoading, error } = useProducer(id);
  const { data: parcels } = useProducerParcels(id);
  const approveKyc = useApproveKyc();
  const suspend = useSuspendProducer();
  const resetMfa = useResetMfa();
  const [suspendOpen, setSuspendOpen] = useState(false);
  const [reason, setReason] = useState('');

  if (isLoading) {
    return <LoadingSkeleton height={300} />;
  }
  if (error || !producer || !id) {
    return (
      <div className="banner banner--error">
        {error instanceof Error ? error.message : t('terroir.common.error')}
      </div>
    );
  }

  return (
    <div>
      <Link to="/producers" className="nav-link" style={{ display: 'inline-block', padding: 0, color: 'var(--terroir-savane)' }}>
        {t('terroir.producer_detail.back')}
      </Link>
      <h1 style={{ marginTop: 8 }}>
        {producer.full_name} <KycBadge status={producer.kyc_status} />
      </h1>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <div className="card">
          <h2 style={{ marginTop: 0 }}>
            {t('terroir.producer_detail.section_profile')}
          </h2>
          <dl style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '8px 16px' }}>
            <dt>NIN</dt>
            <dd className="mono">{producer.nin}</dd>
            <dt>{t('terroir.producers.table.phone')}</dt>
            <dd className="mono">{producer.phone}</dd>
            <dt>{t('terroir.producers.table.department')}</dt>
            <dd>{producer.department} / {producer.province} / {producer.region}</dd>
            <dt>MFA</dt>
            <dd>{producer.mfa_enrolled ? '✓' : '✗'}</dd>
            <dt>{t('terroir.producers.table.updated_at')}</dt>
            <dd className="mono" style={{ fontSize: 12 }}>
              {new Date(producer.updated_at).toLocaleString('fr-FR')}
            </dd>
          </dl>
          <div style={{ display: 'flex', gap: 8, marginTop: 16, flexWrap: 'wrap' }}>
            <button
              className="btn-primary"
              onClick={() => approveKyc.mutate(id)}
              disabled={approveKyc.isPending || producer.kyc_status === 'approved'}
            >
              {t('terroir.producer_detail.actions.approve_kyc')}
            </button>
            <button
              className="btn-danger"
              onClick={() => setSuspendOpen(true)}
              disabled={suspend.isPending}
            >
              {t('terroir.producer_detail.actions.suspend')}
            </button>
            <button
              className="btn-ghost"
              onClick={() => resetMfa.mutate(id)}
              disabled={resetMfa.isPending}
            >
              {t('terroir.producer_detail.actions.reset_mfa')}
            </button>
          </div>
          {suspendOpen && (
            <div className="banner banner--warning" style={{ marginTop: 16 }}>
              <label style={{ display: 'block', marginBottom: 8, fontWeight: 600 }}>
                {t('terroir.producer_detail.confirm_suspend')}
              </label>
              <input
                type="text"
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                style={{ width: '100%', marginBottom: 8 }}
              />
              <div style={{ display: 'flex', gap: 8 }}>
                <button
                  className="btn-danger"
                  onClick={() => {
                    suspend.mutate({ id, reason });
                    setSuspendOpen(false);
                    setReason('');
                  }}
                  disabled={!reason.trim()}
                >
                  {t('terroir.common.confirm')}
                </button>
                <button className="btn-ghost" onClick={() => setSuspendOpen(false)}>
                  {t('terroir.common.cancel')}
                </button>
              </div>
            </div>
          )}
        </div>

        <div className="card">
          <h2 style={{ marginTop: 0 }}>{t('terroir.producer_detail.section_parcels')}</h2>
          {parcels && parcels.length > 0 ? (
            <table>
              <thead>
                <tr>
                  <th>Crop</th>
                  <th>Surface (ha)</th>
                  <th>EUDR</th>
                </tr>
              </thead>
              <tbody>
                {parcels.map((p) => (
                  <tr key={p.id} onClick={() => (window.location.href = `/parcels/${p.id}`)}>
                    <td>{p.crop_type}</td>
                    <td>{p.surface_ha.toFixed(2)}</td>
                    <td>
                      <EudrStatusBadge status={p.eudr_status} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p style={{ color: 'var(--terroir-text-muted)' }}>—</p>
          )}
        </div>
      </div>
    </div>
  );
}
