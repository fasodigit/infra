import { Page, expect } from '@playwright/test';

/**
 * Check if the frontend is reachable on the baseURL.
 * Returns true if the login page loads; false otherwise.
 */
export async function isFrontendAvailable(page: Page, baseURL: string): Promise<boolean> {
  try {
    const res = await page.goto(`${baseURL}/auth/login`, {
      waitUntil: 'domcontentloaded',
      timeout: 8000,
    });
    return res !== null && res.status() < 500;
  } catch {
    return false;
  }
}

/**
 * Register a new user through the 4-step registration form.
 */
export async function registerUser(
  page: Page,
  opts: {
    name: string;
    email: string;
    password: string;
    phone?: string;
    role: 'eleveur' | 'client' | 'producteur_aliment';
    location?: string;
    capacity?: number;
    clientType?: string;
    groupement?: string;
  },
): Promise<void> {
  await page.goto('/auth/register');
  await page.waitForSelector('.register-card', { timeout: 10000 });

  // Step 1: Account info
  await page.locator('input[formControlName="nom"]').fill(opts.name);
  await page.locator('input[formControlName="email"]').fill(opts.email);
  if (opts.phone) {
    await page.locator('input[formControlName="phone"]').fill(opts.phone);
  }
  await page.locator('input[formControlName="password"]').fill(opts.password);
  await page.locator('input[formControlName="confirmPassword"]').fill(opts.password);
  // Click "Suivant" (next)
  await page.locator('button[matStepperNext]').first().click();

  // Step 2: Role selection
  await page.waitForTimeout(300);
  const roleOption = page.locator(`.role-option`).filter({
    has: page.locator(`mat-radio-button[value="${opts.role}"]`),
  });
  await roleOption.click();
  await page.locator('button[matStepperNext]').nth(1).click();

  // Step 3: Details (optional step)
  await page.waitForTimeout(300);
  if (opts.location) {
    await page.locator('input[formControlName="localisation"]').fill(opts.location);
  }
  if (opts.role === 'eleveur' && opts.capacity) {
    await page.locator('input[formControlName="capacite"]').fill(String(opts.capacity));
  }
  if (opts.role === 'client' && opts.clientType) {
    await page.locator('mat-select[formControlName="clientType"]').click();
    await page.locator(`mat-option[value="${opts.clientType}"]`).click();
  }
  await page.locator('button[matStepperNext]').nth(2).click();

  // Step 4: Groupement (optional)
  await page.waitForTimeout(300);
  if (opts.groupement) {
    await page.locator('input[formControlName="groupementNom"]').fill(opts.groupement);
  }
  // Click "Terminer" (finish)
  await page.locator('button').filter({ hasText: /finish|terminer/i }).click();
}

/**
 * Login as a user.
 */
export async function loginAs(
  page: Page,
  email: string,
  password: string,
): Promise<void> {
  await page.goto('/auth/login');
  await page.waitForSelector('.login-card', { timeout: 10000 });
  await page.locator('input[formControlName="email"]').fill(email);
  await page.locator('input[formControlName="password"]').fill(password);
  await page.locator('button[type="submit"]').click();
  // Wait for navigation away from login
  await page.waitForURL(/\/(dashboard|marketplace|orders)/, { timeout: 10000 });
}

/**
 * Logout the current user via the user menu.
 */
export async function logout(page: Page): Promise<void> {
  // Open user menu
  await page.locator('button[mat-icon-button]').filter({ has: page.locator('mat-icon:text("account_circle")') }).click();
  // Click logout
  await page.locator('button[mat-menu-item]').filter({ hasText: /logout|deconnex/i }).click();
  // Verify we land on login
  await page.waitForURL(/\/auth\/login/, { timeout: 10000 });
}

/**
 * Navigate to a route within the authenticated SPA.
 * Uses Angular's router via location change to avoid full page reload.
 */
export async function navigateTo(page: Page, route: string): Promise<void> {
  const currentUrl = page.url();

  // If we're already in the app (not on login page), use Angular router
  if (!currentUrl.includes('/auth/')) {
    // Use Angular's Location service to navigate without full reload
    await page.evaluate((r: string) => {
      // Try to access Angular's router via zone.js context
      const ngRef = (window as any).ng;
      if (ngRef) {
        try {
          const appRef = ngRef.getComponent(document.querySelector('app-root'));
          if (appRef) {
            // Navigate via Angular's injector
            const injector = ngRef.getOwningNgModule(appRef)?.injector
              || (document.querySelector('app-root') as any)?.__ngContext__?.[0];
            // Fallback: use location to trigger Angular routing
          }
        } catch {}
      }
      // Use popstate event which Angular's router listens to
      window.history.pushState({}, '', r);
      window.dispatchEvent(new PopStateEvent('popstate'));
    }, route);
    await page.waitForTimeout(1500);
    await page.waitForLoadState('domcontentloaded');
  } else {
    // If on auth page, do a full navigation
    await page.goto(route);
    await page.waitForLoadState('domcontentloaded');
  }
}

/**
 * Click a sidebar menu item by its label translation key pattern.
 */
export async function clickSidebarItem(page: Page, textPattern: RegExp): Promise<void> {
  const link = page.locator('mat-sidenav a[mat-list-item]').filter({ hasText: textPattern });
  await link.click();
  await page.waitForLoadState('domcontentloaded');
}
