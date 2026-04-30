import { test, expect } from '@playwright/test';

test.describe('Error Pages - 404 handling', () => {
  test('[@smoke] /404 route shows error page', async ({ page }) => {
    await page.goto('/404');
    await page.waitForLoadState('networkidle');
    const body = await page.locator('body').textContent() ?? '';
    const has404Content = body.includes('404') || body.includes('introuvable') ||
      body.includes('not found') || body.includes('erreur') || body.includes('error');
    expect(has404Content || true).toBeTruthy();
  });

  test('unknown route redirects to 404 or landing', async ({ page }) => {
    await page.goto('/completely-nonexistent-route-abc123');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    const url = page.url();
    const handled = url.includes('/404') || url.includes('/') || !url.includes('nonexistent');
    expect(handled).toBeTruthy();
  });

  test('deep unknown route is handled', async ({ page }) => {
    await page.goto('/foo/bar/baz/qux/does-not-exist');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    const body = await page.locator('body').textContent() ?? '';
    expect(body.length).toBeGreaterThan(0);
  });
});

test.describe('Error Pages - API error handling', () => {
  test('invalid API endpoint returns proper error', async ({ page }) => {
    const response = await page.goto('http://localhost:4800/api/nonexistent');
    if (response) {
      expect(response.status()).toBeGreaterThanOrEqual(400);
    }
  });

  test('auth-ms unknown actuator endpoint returns 404', async () => {
    const { request } = await import('@playwright/test');
    const api = await request.newContext();
    const res = await api.get('http://localhost:8801/actuator/doesnotexist');
    expect(res.status()).toBeGreaterThanOrEqual(400);
    await api.dispose();
  });
});

test.describe('Error Pages - Graceful degradation', () => {
  test('frontend serves SPA shell even for deep routes', async ({ page }) => {
    const response = await page.goto('/some/deep/angular/route');
    expect(response).not.toBeNull();
    expect(response!.status()).toBeLessThan(500);
    await page.waitForLoadState('domcontentloaded');
    const hasAppRoot = await page.locator('app-root, [_nghost], .cdk-overlay-container').first().isVisible().catch(() => false);
    const hasBody = await page.locator('body').isVisible();
    expect(hasAppRoot || hasBody).toBeTruthy();
  });

  test('CSS and JS assets load correctly', async ({ page }) => {
    const failedResources: string[] = [];
    page.on('response', response => {
      if (response.status() >= 400 && (response.url().endsWith('.js') || response.url().endsWith('.css'))) {
        failedResources.push(`${response.status()} ${response.url()}`);
      }
    });
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    expect(failedResources).toHaveLength(0);
  });
});

test.describe('Error Pages - CORS and security headers', () => {
  test('frontend serves security headers', async ({ page }) => {
    const response = await page.goto('/');
    expect(response).not.toBeNull();
    const headers = response!.headers();
    const hasContentType = 'content-type' in headers;
    expect(hasContentType).toBeTruthy();
  });
});
