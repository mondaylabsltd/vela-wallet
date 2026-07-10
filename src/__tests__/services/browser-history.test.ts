// Recently-opened dApp history: dedupe-by-origin, newest-first, cap, delete/clear.
const mockStorage = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  getItem: jest.fn(async (key: string) => mockStorage.get(key) ?? null),
  setItem: jest.fn(async (key: string, val: string) => { mockStorage.set(key, val); }),
  removeItem: jest.fn(async (key: string) => { mockStorage.delete(key); }),
}));

import {
  getBrowserHistory,
  recordBrowserVisit,
  deleteBrowserHistory,
  clearBrowserHistory,
} from '@/services/browser-history';

beforeEach(() => mockStorage.clear());

describe('browser-history', () => {
  test('records a visit and reads it back', async () => {
    await recordBrowserVisit({ url: 'https://app.uniswap.org/swap', title: 'Uniswap', favicon: 'https://app.uniswap.org/f.ico' }, 1000);
    const h = await getBrowserHistory();
    expect(h).toHaveLength(1);
    expect(h[0]).toMatchObject({
      origin: 'https://app.uniswap.org',
      url: 'https://app.uniswap.org/swap',
      host: 'app.uniswap.org',
      title: 'Uniswap',
      favicon: 'https://app.uniswap.org/f.ico',
      lastVisited: 1000,
    });
  });

  test('dedupes by ORIGIN — a revisit updates in place (url/time), keeps prior title/favicon if absent', async () => {
    await recordBrowserVisit({ url: 'https://x.io/a', title: 'X', favicon: 'https://x.io/f.png' }, 1000);
    await recordBrowserVisit({ url: 'https://x.io/b' }, 2000); // different path, no title/favicon
    const h = await getBrowserHistory();
    expect(h).toHaveLength(1);
    expect(h[0]).toMatchObject({ url: 'https://x.io/b', title: 'X', favicon: 'https://x.io/f.png', lastVisited: 2000 });
  });

  test('newest-first ordering', async () => {
    await recordBrowserVisit({ url: 'https://a.io' }, 1000);
    await recordBrowserVisit({ url: 'https://b.io' }, 3000);
    await recordBrowserVisit({ url: 'https://c.io' }, 2000);
    const h = await getBrowserHistory();
    expect(h.map((e) => e.host)).toEqual(['b.io', 'c.io', 'a.io']);
  });

  test('ignores a url with no real web origin', async () => {
    await recordBrowserVisit({ url: 'not a url' }, 1000);
    await recordBrowserVisit({ url: '' }, 1000);
    expect(await getBrowserHistory()).toHaveLength(0);
  });

  test('delete removes a single origin; clear empties everything', async () => {
    await recordBrowserVisit({ url: 'https://a.io' }, 1000);
    await recordBrowserVisit({ url: 'https://b.io' }, 2000);
    await deleteBrowserHistory('https://a.io');
    expect((await getBrowserHistory()).map((e) => e.host)).toEqual(['b.io']);
    await clearBrowserHistory();
    expect(await getBrowserHistory()).toHaveLength(0);
  });

  test('caps at 40 entries (oldest dropped)', async () => {
    for (let i = 0; i < 45; i++) {
      await recordBrowserVisit({ url: `https://d${i}.io` }, 1000 + i);
    }
    const h = await getBrowserHistory();
    expect(h).toHaveLength(40);
    // newest kept, oldest 5 dropped
    expect(h[0].host).toBe('d44.io');
    expect(h.some((e) => e.host === 'd0.io')).toBe(false);
  });
});
