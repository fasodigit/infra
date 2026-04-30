// SPDX-License-Identifier: AGPL-3.0-or-later
import { Routes, Route, Navigate } from 'react-router-dom';
import { Login } from './pages/Login';
import { Dashboard } from './pages/Dashboard';
import { Producers } from './pages/Producers';
import { ProducerDetail } from './pages/ProducerDetail';
import { Parcels } from './pages/Parcels';
import { ParcelDetail } from './pages/ParcelDetail';
import { Audit } from './pages/Audit';
import { Sidebar } from './components/Sidebar';
import { Topbar } from './components/Topbar';
import { RequireAuth } from './hooks/useAuth';
import { useTranslation } from 'react-i18next';

function Shell({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  return (
    <div className="app-shell">
      <aside className="app-shell__sidebar">
        <Sidebar />
      </aside>
      <header className="app-shell__topbar">
        <Topbar />
      </header>
      <main className="app-shell__main">
        {children}
        <footer className="app-shell__footer">
          {t('terroir.footer.attribution')}
        </footer>
      </main>
    </div>
  );
}

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route
        path="/"
        element={
          <RequireAuth>
            <Shell>
              <Navigate to="/dashboard" replace />
            </Shell>
          </RequireAuth>
        }
      />
      <Route
        path="/dashboard"
        element={
          <RequireAuth>
            <Shell>
              <Dashboard />
            </Shell>
          </RequireAuth>
        }
      />
      <Route
        path="/producers"
        element={
          <RequireAuth>
            <Shell>
              <Producers />
            </Shell>
          </RequireAuth>
        }
      />
      <Route
        path="/producers/:id"
        element={
          <RequireAuth>
            <Shell>
              <ProducerDetail />
            </Shell>
          </RequireAuth>
        }
      />
      <Route
        path="/parcels"
        element={
          <RequireAuth>
            <Shell>
              <Parcels />
            </Shell>
          </RequireAuth>
        }
      />
      <Route
        path="/parcels/:id"
        element={
          <RequireAuth>
            <Shell>
              <ParcelDetail />
            </Shell>
          </RequireAuth>
        }
      />
      <Route
        path="/audit"
        element={
          <RequireAuth>
            <Shell>
              <Audit />
            </Shell>
          </RequireAuth>
        }
      />
      <Route path="*" element={<Navigate to="/dashboard" replace />} />
    </Routes>
  );
}
