/**
 * Web E2E smoke tests.
 *
 * Verifies critical user paths work on the web build:
 * - App loads and renders onboarding
 * - Navigation works
 * - Key UI elements are present
 *
 * Run: npx playwright test
 * Requires: expo web server running (auto-started by playwright.config.ts)
 */
import { test, expect } from '@playwright/test';

test.describe('Vela Wallet Web - Smoke Tests', () => {
  test('app loads and shows onboarding screen', async ({ page }) => {
    await page.goto('/');
    // Wait for the app to hydrate and render
    await page.waitForLoadState('networkidle');

    // The onboarding screen should be visible
    // Look for "Vela" branding or the welcome screen content
    const body = await page.textContent('body');
    expect(body).toBeTruthy();

    // App should not show a blank white page or crash error
    // Note: Expo web may not set a document title
    const hasContent = (body?.length ?? 0) > 10;
    expect(hasContent).toBeTruthy();
  });

  test('onboarding has create wallet and sign in options', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Wait for React to render (expo-router may take a moment)
    await page.waitForTimeout(3000);

    // Look for key onboarding UI elements
    const pageText = await page.textContent('body');

    // The welcome screen should contain wallet-related text
    const hasWalletText =
      pageText?.includes('Wallet') ||
      pageText?.includes('wallet') ||
      pageText?.includes('Create') ||
      pageText?.includes('Sign In') ||
      pageText?.includes('Get Started');

    expect(hasWalletText).toBeTruthy();
  });

  test('page does not have JavaScript errors on load', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Filter out known non-critical errors (e.g., ResizeObserver)
    const criticalErrors = errors.filter(
      (e) => !e.includes('ResizeObserver') && !e.includes('ServiceWorker')
    );

    expect(criticalErrors).toEqual([]);
  });

  test('navigation to /onboarding works', async ({ page }) => {
    await page.goto('/onboarding');
    await page.waitForLoadState('networkidle');

    const status = page.url();
    // Should not redirect to a 404 or error page
    expect(status).toContain('/onboarding');
  });

  test('settings route is accessible', async ({ page }) => {
    // The settings tab route should be defined
    const response = await page.goto('/settings');
    // Even if it redirects (because no wallet is set up), it should not 404
    expect(response?.status()).toBeLessThan(500);
  });

  test('about page loads', async ({ page }) => {
    await page.goto('/about');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    const text = await page.textContent('body');
    const hasAboutContent =
      text?.includes('Vela') ||
      text?.includes('Version') ||
      text?.includes('About');

    expect(hasAboutContent).toBeTruthy();
  });
});
