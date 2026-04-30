// SPDX-License-Identifier: AGPL-3.0-or-later
import { NavLink } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

const navItems = [
  { to: '/dashboard', i18nKey: 'terroir.nav.dashboard' },
  { to: '/producers', i18nKey: 'terroir.nav.producers' },
  { to: '/parcels', i18nKey: 'terroir.nav.parcels' },
  { to: '/audit', i18nKey: 'terroir.nav.audit' },
];

export function Sidebar() {
  const { t } = useTranslation();
  return (
    <nav>
      <div className="brand">
        <span aria-hidden="true">🌾</span>
        <div>
          <div>{t('terroir.brand')}</div>
          <div style={{ fontSize: 10, fontWeight: 400, color: 'rgba(255,255,255,0.6)' }}>
            {t('terroir.tagline')}
          </div>
        </div>
      </div>
      <ul style={{ listStyle: 'none', margin: 0, padding: '8px 0' }}>
        {navItems.map((item) => (
          <li key={item.to}>
            <NavLink
              to={item.to}
              className={({ isActive }) => (isActive ? 'nav-link active' : 'nav-link')}
            >
              {t(item.i18nKey)}
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  );
}
