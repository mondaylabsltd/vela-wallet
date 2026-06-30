/**
 * Tests for the one-click bug-report client: backend success, the no-token 503
 * fallback to the prefilled GitHub URL, network-failure fallback, and the
 * empty-input guard. User input must never be lost; nothing leaks secrets.
 */

jest.mock('react-native', () => ({ Platform: { OS: 'ios', Version: '18.0' } }));
jest.mock('@/constants/build-info', () => ({ APP_VERSION: '1.0.0', GIT_COMMIT: 'abc1234' }));
jest.mock('@/models/network', () => ({ chainName: (id: number) => `Chain${id}` }));
jest.mock('@/i18n', () => ({ LANGUAGE_NATIVE_NAMES: { en: 'English' } }));
jest.mock('@/services/rpc-pool', () => ({ getFailedRpcChains: () => new Set<number>() }));
jest.mock('@/services/feedback', () => ({
  buildBugReportURL: () => 'https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml',
}));

import { submitBugReport, buildReportPreview } from '@/services/bug-report';

describe('bug-report client', () => {
  const realFetch = global.fetch;
  afterEach(() => { global.fetch = realFetch; });

  test('empty description does not call the backend and offers the URL fallback', async () => {
    global.fetch = jest.fn() as any;
    const r = await submitBugReport({ what: '   ', language: 'en' as any });
    expect(r.ok).toBe(false);
    expect(r.reason).toBe('empty');
    expect(r.fallbackUrl).toContain('github.com');
    expect(global.fetch).not.toHaveBeenCalled();
  });

  test('returns the issue url + number on backend success', async () => {
    global.fetch = jest.fn(async () => ({
      ok: true,
      json: async () => ({ url: 'https://github.com/mondaylabsltd/vela-wallet/issues/42', number: 42, deduped: false }),
    })) as any;
    const r = await submitBugReport({ what: 'screen froze on send', language: 'en' as any });
    expect(r).toMatchObject({ ok: true, url: expect.stringContaining('/issues/42'), number: 42 });
  });

  test('503 not_configured falls back to the prefilled GitHub URL', async () => {
    global.fetch = jest.fn(async () => ({
      ok: false,
      status: 503,
      json: async () => ({ error: 'not_configured' }),
    })) as any;
    const r = await submitBugReport({ what: 'bug', language: 'en' as any });
    expect(r.ok).toBe(false);
    expect(r.reason).toBe('not_configured');
    expect(r.fallbackUrl).toContain('github.com');
  });

  test('a network failure falls back to the URL (input preserved by caller)', async () => {
    global.fetch = jest.fn(async () => { throw new Error('ECONNRESET'); }) as any;
    const r = await submitBugReport({ what: 'bug', language: 'en' as any });
    expect(r.ok).toBe(false);
    expect(r.fallbackUrl).toContain('github.com');
  });

  test('preview shows the env block and never throws', () => {
    const preview = buildReportPreview({ what: 'tapped send', steps: '1. open\n2. tap', language: 'en' as any });
    expect(preview).toContain('tapped send');
    expect(preview).toContain('App version: 1.0.0');
    expect(preview).toContain('Diagnostics');
  });
});
