// SPDX-License-Identifier: AGPL-3.0-or-later
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { fetchDashboardKpis } from '../api/client';
import { LoadingSkeleton } from '../components/LoadingSkeleton';
import type { DashboardKpis } from '../api/types';

function MiniSparkline({
  data,
  color,
  height = 60,
  width = 200,
}: {
  data: number[];
  color: string;
  height?: number;
  width?: number;
}) {
  if (data.length === 0) return null;
  const max = Math.max(...data, 1);
  const step = width / Math.max(data.length - 1, 1);
  const points = data
    .map((v, i) => `${i * step},${height - (v / max) * (height - 4) - 2}`)
    .join(' ');
  return (
    <svg width={width} height={height} role="img" aria-label="7d-trend">
      <polyline
        fill="none"
        stroke={color}
        strokeWidth={2}
        points={points}
      />
    </svg>
  );
}

function KpiCard({
  label,
  value,
  series,
  color,
}: {
  label: string;
  value: number;
  series: number[];
  color: string;
}) {
  return (
    <div className="card kpi-card" data-testid={`kpi-${label}`}>
      <div className="kpi-card__label">{label}</div>
      <div className="kpi-card__value">{value.toLocaleString('fr-FR')}</div>
      <MiniSparkline data={series} color={color} width={220} height={50} />
    </div>
  );
}

export function Dashboard() {
  const { t } = useTranslation();
  const { data, isLoading, error } = useQuery<DashboardKpis>({
    queryKey: ['dashboard-kpis'],
    queryFn: fetchDashboardKpis,
  });

  if (isLoading) {
    return (
      <div>
        <h1>{t('terroir.dashboard.title')}</h1>
        <div className="kpi-grid">
          {Array.from({ length: 4 }).map((_, i) => (
            <LoadingSkeleton key={i} height={140} />
          ))}
        </div>
      </div>
    );
  }
  if (error) {
    return (
      <div className="banner banner--error">
        {t('terroir.common.error')} : {error instanceof Error ? error.message : 'unknown'}
      </div>
    );
  }
  if (!data) return null;

  const series = data.series_7d;
  const sProducers = series.map((s) => s.producers);
  const sParcels = series.map((s) => s.parcels);
  const sDds = series.map((s) => s.dds);
  const sRejected = series.map((s) => s.rejected);

  return (
    <div>
      <h1 style={{ marginTop: 0 }}>{t('terroir.dashboard.title')}</h1>
      <div className="kpi-grid" style={{ marginBottom: 24 }}>
        <KpiCard
          label={t('terroir.dashboard.kpi.producers_total')}
          value={data.producers_total}
          series={sProducers}
          color="var(--terroir-savane)"
        />
        <KpiCard
          label={t('terroir.dashboard.kpi.parcels_validated')}
          value={data.parcels_validated}
          series={sParcels}
          color="var(--terroir-success)"
        />
        <KpiCard
          label={t('terroir.dashboard.kpi.dds_submitted')}
          value={data.dds_submitted}
          series={sDds}
          color="var(--terroir-info)"
        />
        <KpiCard
          label={t('terroir.dashboard.kpi.eudr_alerts_rejected')}
          value={data.eudr_alerts_rejected}
          series={sRejected}
          color="var(--terroir-rouge-bf)"
        />
      </div>

      <div className="card">
        <h2 style={{ marginTop: 0 }}>{t('terroir.dashboard.chart_7d_title')}</h2>
        <table>
          <thead>
            <tr>
              <th>Date</th>
              <th>{t('terroir.dashboard.kpi.producers_total')}</th>
              <th>{t('terroir.dashboard.kpi.parcels_validated')}</th>
              <th>{t('terroir.dashboard.kpi.dds_submitted')}</th>
              <th>{t('terroir.dashboard.kpi.eudr_alerts_rejected')}</th>
            </tr>
          </thead>
          <tbody>
            {series.map((s) => (
              <tr key={s.date}>
                <td className="mono">{s.date}</td>
                <td>{s.producers}</td>
                <td>{s.parcels}</td>
                <td>{s.dds}</td>
                <td>{s.rejected}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
