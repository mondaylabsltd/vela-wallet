/**
 * Regression test for the "language reverts to follow-system" bug.
 *
 * Root cause was that setLanguagePreference() wrote to AsyncStorage *after*
 * `await i18n.changeLanguage()`. If that await ever rejected/hung, the write
 * was skipped — the in-session UI switched but the choice was never persisted,
 * so on the next launch loadLanguage() found nothing and fell back to 'auto'.
 *
 * These tests pin the fix: the preference is persisted independently of
 * changeLanguage, and loadLanguage() restores it.
 */

// In-memory AsyncStorage (factory may only reference `mock`-prefixed outer vars).
const mockStore = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn((k: string) => Promise.resolve(mockStore.has(k) ? mockStore.get(k)! : null)),
    setItem: jest.fn((k: string, v: string) => { mockStore.set(k, v); return Promise.resolve(); }),
    removeItem: jest.fn((k: string) => { mockStore.delete(k); return Promise.resolve(); }),
  },
}));
// Device locale resolves to English (so 'auto' → 'en').
jest.mock('expo-localization', () => ({
  getLocales: () => [{ languageCode: 'en', languageTag: 'en-US', regionCode: 'US' }],
}));
// Avoid pulling in React for the i18next plugin.
jest.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: () => {} },
}));
// Keep the bundled-resources import light (the real one pulls 195 JSON files).
jest.mock('@/i18n/resources', () => ({
  resources: {
    en: { translation: {} },
    zh: { translation: {} },
    ja: { translation: {} },
    ko: { translation: {} },
  },
  en: {},
}));

import i18n, { setLanguagePreference, loadLanguage, getLanguagePreference } from '@/i18n';

const KEY = 'vela.language';

beforeEach(() => {
  mockStore.clear();
  jest.restoreAllMocks();
});

describe('language preference persistence', () => {
  test('persists the choice even when changeLanguage REJECTS (the original bug)', async () => {
    jest.spyOn(i18n, 'changeLanguage').mockRejectedValue(new Error('changeLanguage failed'));

    // Mirrors the UI handler: it does not await/throw to the caller.
    await setLanguagePreference('ko').catch(() => {});

    // The write must have happened regardless of changeLanguage failing.
    expect(mockStore.get(KEY)).toBe('ko');
  });

  test('persists the choice when changeLanguage hangs (never resolves)', async () => {
    jest.spyOn(i18n, 'changeLanguage').mockReturnValue(new Promise(() => {}) as never);

    void setLanguagePreference('ja'); // do not await — changeLanguage never settles
    await Promise.resolve();           // let the synchronous write + microtask flush

    expect(mockStore.get(KEY)).toBe('ja');
  });

  test('stores the raw preference (not the resolved language)', async () => {
    jest.spyOn(i18n, 'changeLanguage').mockResolvedValue((() => '') as never);

    await setLanguagePreference('auto');
    expect(mockStore.get(KEY)).toBe('auto'); // must be 'auto', not the resolved 'en'
  });

  test('loadLanguage() restores the persisted choice on startup', async () => {
    jest.spyOn(i18n, 'changeLanguage').mockResolvedValue((() => '') as never);
    mockStore.set(KEY, 'ko');

    await loadLanguage();
    expect(getLanguagePreference()).toBe('ko');
  });

  test('loadLanguage() still restores the preference even if changeLanguage rejects', async () => {
    jest.spyOn(i18n, 'changeLanguage').mockRejectedValue(new Error('boom'));
    mockStore.set(KEY, 'ja');

    await loadLanguage().catch(() => {});
    expect(getLanguagePreference()).toBe('ja');
  });

  test('ignores an unsupported stored value (keeps the current preference)', async () => {
    jest.spyOn(i18n, 'changeLanguage').mockResolvedValue((() => '') as never);
    await setLanguagePreference('auto'); // known baseline (_preference = 'auto')
    mockStore.set(KEY, 'klingon');       // corrupt the persisted value

    await loadLanguage();
    expect(getLanguagePreference()).toBe('auto'); // invalid value ignored
  });
});
