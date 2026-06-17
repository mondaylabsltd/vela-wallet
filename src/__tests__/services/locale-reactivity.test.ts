/**
 * Verifies the locale-prefs store actually drives formatNumber output and
 * notifies subscribers — the mechanism behind live number-format changes.
 */
const mockStore = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(async (k: string) => mockStore.get(k) ?? null),
    setItem: jest.fn(async (k: string, v: string) => { mockStore.set(k, v); }),
    removeItem: jest.fn(async (k: string) => { mockStore.delete(k); }),
  },
}));

import { saveLocalePrefs, getLocalePrefs, subscribeLocalePrefs } from '@/services/storage';
import { formatNumber } from '@/services/locale-format';
import { DEFAULT_LOCALE_PREFS } from '@/models/types';

test('saving a number format notifies subscribers and changes the snapshot identity', async () => {
  const before = getLocalePrefs();
  const listener = jest.fn();
  const unsub = subscribeLocalePrefs(listener);

  await saveLocalePrefs({ ...DEFAULT_LOCALE_PREFS, numberFormat: 'dot_comma' });

  expect(listener).toHaveBeenCalledTimes(1);
  expect(getLocalePrefs().numberFormat).toBe('dot_comma');
  expect(getLocalePrefs()).not.toBe(before); // new identity → useSyncExternalStore re-renders
  unsub();
});

test('formatNumber output follows the saved number format immediately', async () => {
  await saveLocalePrefs({ ...DEFAULT_LOCALE_PREFS, numberFormat: 'comma_dot' });
  expect(formatNumber(1234567.89, { minimumFractionDigits: 2 })).toBe('1,234,567.89');

  await saveLocalePrefs({ ...DEFAULT_LOCALE_PREFS, numberFormat: 'dot_comma' });
  expect(formatNumber(1234567.89, { minimumFractionDigits: 2 })).toBe('1.234.567,89');

  await saveLocalePrefs({ ...DEFAULT_LOCALE_PREFS, numberFormat: 'space_comma' });
  // space_comma groups with a non-breaking space — normalise it for the assert.
  expect(formatNumber(1234567.89, { minimumFractionDigits: 2 }).replace(/[  ]/g, ' ')).toBe('1 234 567,89');
});
