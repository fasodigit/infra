// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { listAudit } from '../api/client';
import { TableSkeleton } from '../components/LoadingSkeleton';
import type { AuditQuery } from '../api/types';

const PAGE_SIZE = 50;

export function Audit() {
  const { t } = useTranslation();
  const [page, setPage] = useState(1);
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [actor, setActor] = useState('');
  const [action, setAction] = useState('');

  const query: AuditQuery = {
    page,
    page_size: PAGE_SIZE,
    from: from ? new Date(from).toISOString() : undefined,
    to: to ? new Date(to).toISOString() : undefined,
    actor_id: actor || undefined,
    action: action || undefined,
  };

  const { data, isLoading, error } = useQuery({
    queryKey: ['audit', query],
    queryFn: () => listAudit(query),
  });

  const totalPages = data ? Math.max(1, Math.ceil(data.total / PAGE_SIZE)) : 1;

  return (
    <div>
      <h1 style={{ marginTop: 0 }}>{t('terroir.audit.title')}</h1>

      <div className="card" style={{ marginBottom: 16 }}>
        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
          <label>
            <span style={{ display: 'block', fontSize: 11 }}>
              {t('terroir.audit.filter_from')}
            </span>
            <input
              type="datetime-local"
              value={from}
              onChange={(e) => {
                setFrom(e.target.value);
                setPage(1);
              }}
            />
          </label>
          <label>
            <span style={{ display: 'block', fontSize: 11 }}>
              {t('terroir.audit.filter_to')}
            </span>
            <input
              type="datetime-local"
              value={to}
              onChange={(e) => {
                setTo(e.target.value);
                setPage(1);
              }}
            />
          </label>
          <label>
            <span style={{ display: 'block', fontSize: 11 }}>
              {t('terroir.audit.filter_actor')}
            </span>
            <input
              type="text"
              placeholder="UUID acteur"
              value={actor}
              onChange={(e) => {
                setActor(e.target.value);
                setPage(1);
              }}
            />
          </label>
          <label>
            <span style={{ display: 'block', fontSize: 11 }}>
              {t('terroir.audit.filter_action')}
            </span>
            <input
              type="text"
              placeholder="ex: producer.kyc.approved"
              value={action}
              onChange={(e) => {
                setAction(e.target.value);
                setPage(1);
              }}
            />
          </label>
        </div>
      </div>

      <div className="card">
        {isLoading ? (
          <TableSkeleton rows={10} cols={5} />
        ) : error ? (
          <div className="banner banner--error">
            {error instanceof Error ? error.message : 'unknown'}
          </div>
        ) : !data || data.items.length === 0 ? (
          <p>{t('terroir.audit.empty')}</p>
        ) : (
          <>
            <ul className="timeline">
              {data.items.map((ev) => (
                <li key={ev.id}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
                    <div>
                      <div className="mono" style={{ fontSize: 12, color: 'var(--terroir-text-muted)' }}>
                        {new Date(ev.timestamp).toLocaleString('fr-FR')}
                      </div>
                      <div>
                        <strong>{ev.action}</strong> →{' '}
                        <span className="badge badge--info">{ev.resource_type}</span>{' '}
                        <span className="mono" style={{ fontSize: 11 }}>
                          {ev.resource_id.slice(0, 8)}…
                        </span>
                      </div>
                      <div style={{ fontSize: 12, color: 'var(--terroir-text-muted)' }}>
                        {t('terroir.audit.actor')}: {ev.actor_email}
                      </div>
                    </div>
                    {ev.trace_id && (
                      <a
                        href={`http://localhost:16686/trace/${ev.trace_id}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="mono"
                        style={{ fontSize: 11 }}
                        title={t('terroir.audit.trace')}
                      >
                        🔍 {ev.trace_id.slice(0, 8)}…
                      </a>
                    )}
                  </div>
                </li>
              ))}
            </ul>
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
