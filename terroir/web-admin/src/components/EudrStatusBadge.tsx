// SPDX-License-Identifier: AGPL-3.0-or-later
import { useTranslation } from 'react-i18next';
import type { EudrStatus } from '../api/types';

const statusToVariant: Record<EudrStatus, string> = {
  pending: 'badge--info',
  validated: 'badge--success',
  rejected: 'badge--error',
  escalated: 'badge--warning',
  expired: 'badge--muted',
};

export function EudrStatusBadge({ status }: { status: EudrStatus }) {
  const { t } = useTranslation();
  return (
    <span className={`badge ${statusToVariant[status]}`}>
      {t(`terroir.eudr.status.${status}`)}
    </span>
  );
}

export function EudrStatusBanner({ status }: { status: EudrStatus }) {
  const { t } = useTranslation();
  const variant =
    status === 'validated'
      ? 'banner--success'
      : status === 'rejected' || status === 'expired'
        ? 'banner--error'
        : status === 'escalated'
          ? 'banner--warning'
          : '';
  return (
    <div className={`banner ${variant}`}>
      <strong>EUDR : {t(`terroir.eudr.status.${status}`)}</strong>
    </div>
  );
}
