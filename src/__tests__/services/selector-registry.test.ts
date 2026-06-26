/**
 * Tests for the 4-byte selector registry — merge across sources, dedup, cache,
 * and graceful failure. Network is mocked.
 */
import { lookupSelector, clearSelectorCache } from '@/services/selector-registry';

const SEL = '0x38ed1739';
const SWAP = 'swapExactTokensForTokens(uint256,uint256,address[],address,uint256)';
const ALT = 'someOtherCollision(uint256)';

function mockFetch(handler: (url: string) => any) {
  (global as any).fetch = jest.fn(async (url: string) => {
    const body = handler(url);
    return {
      ok: body !== null,
      json: async () => body,
    };
  });
}

const openchainBody = (names: string[]) => ({
  ok: true,
  result: { function: { [SEL]: names.map((name) => ({ name })) } },
});
const fourbyteBody = (sigs: string[]) => ({
  results: sigs.map((text_signature, i) => ({ id: i + 1, text_signature })),
});

describe('selector-registry', () => {
  beforeEach(() => { clearSelectorCache(); });
  afterEach(() => { jest.restoreAllMocks(); });

  test('merges openchain-compatible + 4byte results, openchain first, deduped', async () => {
    mockFetch((url) => {
      if (url.includes('sourcify') || url.includes('openchain')) return openchainBody([SWAP]);
      if (url.includes('4byte.directory')) return fourbyteBody([SWAP, ALT]); // SWAP dup
      return null;
    });
    const r = await lookupSelector(SEL);
    expect(r[0]).toBe(SWAP);      // openchain-sourced first
    expect(r).toContain(ALT);     // 4byte unique appended
    expect(r.filter((s) => s === SWAP)).toHaveLength(1); // deduped
  });

  test('works when only 4byte.directory responds', async () => {
    mockFetch((url) => (url.includes('4byte.directory') ? fourbyteBody([SWAP]) : null));
    expect(await lookupSelector(SEL)).toEqual([SWAP]);
  });

  test('4byte.directory sorted by id ascending (canonical first)', async () => {
    mockFetch((url) => {
      if (url.includes('4byte.directory')) return { results: [
        { id: 50, text_signature: ALT },
        { id: 2, text_signature: SWAP },
      ] };
      return null;
    });
    const r = await lookupSelector(SEL);
    expect(r[0]).toBe(SWAP);
  });

  test('returns [] and caches when nothing is found', async () => {
    const fetchMock = jest.fn(async () => ({ ok: true, json: async () => ({}) }));
    (global as any).fetch = fetchMock;
    expect(await lookupSelector(SEL)).toEqual([]);
    const callsAfterFirst = fetchMock.mock.calls.length;
    expect(await lookupSelector(SEL)).toEqual([]); // served from cache
    expect(fetchMock.mock.calls.length).toBe(callsAfterFirst); // no new network
  });

  test('rejects malformed selectors', async () => {
    mockFetch(() => null);
    expect(await lookupSelector('0x12')).toEqual([]);
  });

  test('survives network errors', async () => {
    (global as any).fetch = jest.fn(async () => { throw new Error('network down'); });
    expect(await lookupSelector(SEL)).toEqual([]);
  });
});
