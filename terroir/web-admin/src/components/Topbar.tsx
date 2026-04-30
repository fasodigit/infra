// SPDX-License-Identifier: AGPL-3.0-or-later
import { useTranslation } from 'react-i18next';
import { useAuth } from '../hooks/useAuth';

export function Topbar() {
  const { t, i18n } = useTranslation();
  const { session, logout } = useAuth();

  const role = session?.identity.metadata_public?.role ?? 'manager';
  const roleLabel =
    role === 'admin'
      ? t('terroir.topbar.role_admin')
      : role === 'cooperative'
        ? t('terroir.topbar.role_cooperative')
        : t('terroir.topbar.role_manager');

  const toggleLang = () => {
    void i18n.changeLanguage(i18n.language === 'fr' ? 'en' : 'fr');
  };

  return (
    <>
      <div style={{ fontSize: 14, color: 'var(--terroir-text-muted)' }}>
        {session?.identity.traits.email ?? ''}
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <span className="badge badge--info">{roleLabel}</span>
        <button className="btn-ghost" onClick={toggleLang} aria-label="language toggle">
          {i18n.language.toUpperCase()}
        </button>
        <button className="btn-ghost" onClick={() => void logout()}>
          {t('terroir.nav.logout')}
        </button>
      </div>
    </>
  );
}
