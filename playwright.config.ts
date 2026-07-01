import { defineConfig } from '@playwright/test';

// Point the suite at an already-running dev server with E2E_BASE_URL (e.g. a fresh
// `expo start --web --port 8092`). The parallel space needs a DEV build (`__DEV__`
// true) — the fixed-passkey override is a compile-time no-op in production. When
// E2E_BASE_URL is set we don't manage a server; otherwise we start one on 8081.
const BASE_URL = process.env.E2E_BASE_URL ?? 'http://localhost:8081';

export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  retries: 1,
  use: {
    baseURL: BASE_URL,
    headless: true,
    screenshot: 'only-on-failure',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
    },
  ],
  ...(process.env.E2E_BASE_URL
    ? {}
    : {
        webServer: {
          command: 'npx expo start --web --port 8081',
          port: 8081,
          timeout: 60_000,
          reuseExistingServer: true,
        },
      }),
});
