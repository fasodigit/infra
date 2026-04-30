// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useProducers } from '../hooks/useProducers';
import { useCooperatives } from '../hooks/useParcels';
import { TableSkeleton } from '../components/LoadingSkeleton';
import { KycBadge } from '../components/KycBadge';
import type { KycStatus } from '../api/types';

const PAGE_SIZE = 25;

function maskNin(nin: string): string {
  // Tronqué à 4 derniers caractères pour confidentialité.
  if (nin.length <= 4) return nin;
  return `••••${nin.slice(-4)}`;
}

function maskPhone(phone: string): string {
  // Masque tout sauf 4 derniers chiffres.
  if (phone.length <= 4) return phone;
  return `${phone.slice(0, 4)}••••${phone.slice(-2)}`;
}

export function Producers() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState('');
  const [coopId, setCoopId] = useState<string>('');
  const [kycFilter, setKycFilter] = useState<KycStatus | ''>('');

  const { data: coops } = useCooperatives();
  const { data, isLoading, error } = useProducers({
    page,
    page_size: PAGE_SIZE,
    search: search || undefined,
    cooperative_id: coopId || undefined,
    kyc_status: (kycFilter as KycStatus) || undefined,
  });

  const totalPages = data ? Math.max(1, Math.ceil(data.total / PAGE_SIZE)) : 1;

  return (
    <div>
      <h1 style={{ marginTop: 0 }}>{t('terroir.producers.title')}</h1>

      <div className="card" style={{ marginBottom: 16 }}>
        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
          <input
            type="search"
            placeholder={t('terroir.producers.search_placeholder')}
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setPage(1);
            }}
            style={{ flex: 1, minWidth: 260 }}
          />
          <select
            value={coopId}
            onChange={(e) => {
              setCoopId(e.target.value);
              setPage(1);
            }}
          >
            <option value="">{t('terroir.producers.filter_cooperative')}</option>
            {coops?.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} ({c.region})
              </option>
            ))}
          </select>
          <select
            value={kycFilter}
            onChange={(e) => {
              setKycFilter(e.target.value as KycStatus | '');
              setPage(1);
            }}
          >
            <option value="">{t('terroir.producers.filter_kyc')}</option>
            <option value="pending">{t('terroir.kyc.status.pending')}</option>
            <option value="approved">{t('terroir.kyc.status.approved')}</option>
            <option value="rejected">{t('terroir.kyc.status.rejected')}</option>
            <option value="suspended">{t('terroir.kyc.status.suspended')}</option>
            <option value="expired">{t('terroir.kyc.status.expired')}</option>
          </select>
        </div>
      </div>

      <div className="card">
        {isLoading ? (
          <TableSkeleton rows={8} cols={7} />
        ) : error ? (
          <div className="banner banner--error">
            {error instanceof Error ? error.message : 'unknown'}
          </div>
        ) : !data || data.items.length === 0 ? (
          <p>{t('terroir.producers.empty')}</p>
        ) : (
          <>
            <table>
              <thead>
                <tr>
                  <th>{t('terroir.producers.table.name')}</th>
                  <th>{t('terroir.producers.table.nin')}</th>
                  <th>{t('terroir.producers.table.phone')}</th>
                  <th>{t('terroir.producers.table.department')}</th>
                  <th>{t('terroir.producers.table.mfa')}</th>
                  <th>{t('terroir.producers.table.kyc')}</th>
                  <th>{t('terroir.producers.table.updated_at')}</th>
                </tr>
              </thead>
              <tbody>
                {data.items.map((p) => (
                  <tr
                    key={p.id}
                    onClick={() => navigate(`/producers/${p.id}`)}
                    data-testid={`producer-row-${p.id}`}
                  >
                    <td>{p.full_name}</td>
                    <td className="mono">{maskNin(p.nin)}</td>
                    <td className="mono">{maskPhone(p.phone)}</td>
                    <td>{p.department}</td>
                    <td>
                      {p.mfa_enrolled
                        ? t('terroir.producers.mfa_yes')
                        : t('terroir.producers.mfa_no')}
                    </td>
                    <td>
                      <KycBadge status={p.kyc_status} />
                    </td>
                    <td className="mono" style={{ fontSize: 12 }}>
                      {new Date(p.updated_at).toLocaleString('fr-FR')}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginTop: 12,
              }}
            >
              <span style={{ fontSize: 12, color: 'var(--terroir-text-muted)' }}>
                {t('terroir.common.page_of', { page, total: totalPages })} —{' '}
                {data.total} total
              </span>
              <div style={{ display: 'flex', gap: 8 }}>
                <button
                  className="btn-ghost"
                  onClick={() => setPage((p) => Math.max(1, p - 1))}
                  disabled={page <= 1}
                >
                  {t('terroir.common.previous')}
                </button>
                <button
                  className="btn-ghost"
                  onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                  disabled={page >= totalPages}
                >
                  {t('terroir.common.next')}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
