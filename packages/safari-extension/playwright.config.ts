import { defineConfig } from '@playwright/test';

// Isolated from the app's root playwright.config.ts (which boots an Expo web
// server). This one only tests the extension provider in a headless browser
// against a mock native/background bridge — no app, no server dependency.
// Run from the repo root:  npx playwright test --config packages/safari-extension/playwright.config.ts
export default defineConfig({
  testDir: './test',
  testMatch: ['**/*.spec.ts'],
  timeout: 20_000,
  fullyParallel: true,
  use: { headless: true },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
