// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState, useEffect, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { initLoginFlow, submitLogin, whoami, type KratosFlow } from '../auth/kratos-client';

export function Login() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [flow, setFlow] = useState<KratosFlow | null>(null);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Si déjà authentifié, redirect dashboard.
    void whoami().then((s) => {
      if (s?.active) navigate('/dashboard', { replace: true });
    });
    // Init flow Kratos.
    void initLoginFlow()
      .then(setFlow)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [navigate]);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!flow) return;
    setSubmitting(true);
    setError(null);
    try {
      await submitLogin(flow, email, password);
      navigate('/dashboard', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : t('terroir.login.error_generic'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      style={{
        minHeight: '100vh',
        display: 'grid',
        placeItems: 'center',
        background: 'linear-gradient(135deg, var(--terroir-savane) 0%, var(--terroir-savane-dark) 100%)',
        padding: 24,
      }}
    >
      <div className="card" style={{ width: '100%', maxWidth: 420 }}>
        <h1 style={{ marginTop: 0, color: 'var(--terroir-savane)' }}>
          🌾 {t('terroir.brand')}
        </h1>
        <p style={{ color: 'var(--terroir-text-muted)', marginBottom: 24 }}>
          {t('terroir.login.subtitle')}
        </p>
        <form onSubmit={onSubmit} aria-label="login-form">
          <label style={{ display: 'block', marginBottom: 12 }}>
            <span style={{ display: 'block', fontWeight: 600, marginBottom: 4 }}>
              {t('terroir.login.email')}
            </span>
            <input
              type="email"
              name="identifier"
              required
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              style={{ width: '100%' }}
            />
          </label>
          <label style={{ display: 'block', marginBottom: 16 }}>
            <span style={{ display: 'block', fontWeight: 600, marginBottom: 4 }}>
              {t('terroir.login.password')}
            </span>
            <input
              type="password"
              name="password"
              required
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              style={{ width: '100%' }}
            />
          </label>
          {error && (
            <div className="banner banner--error" role="alert">
              {error}
            </div>
          )}
          <button
            type="submit"
            className="btn-primary"
            disabled={submitting || !flow}
            style={{ width: '100%' }}
          >
            {submitting ? t('terroir.login.submitting') : t('terroir.login.submit')}
          </button>
        </form>
        <div
          style={{
            marginTop: 24,
            fontSize: 11,
            color: 'var(--terroir-text-muted)',
            textAlign: 'center',
          }}
        >
          {t('terroir.footer.attribution')}
        </div>
      </div>
    </div>
  );
}
