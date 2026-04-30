// SPDX-License-Identifier: AGPL-3.0-or-later
import { useTranslation } from 'react-i18next';
import type { KycStatus } from '../api/types';

const statusToVariant: Record<KycStatus, string> = {
  pending: 'badge--warning',
  approved: 'badge--success',
  rejected: 'badge--error',
  suspended: 'badge--error',
  expired: 'badge--muted',
};

export function KycBadge({ status }: { status: KycStatus }) {
  const { t } = useTranslation();
  return (
    <span className={`badge ${statusToVariant[status]}`}>
      {t(`terroir.kyc.status.${status}`)}
    </span>
  );
}
