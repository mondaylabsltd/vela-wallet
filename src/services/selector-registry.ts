/**
 * 4-byte function-selector registry.
 *
 * When no ERC-7730 descriptor matches a transaction, we don't give up and blind
 * sign — we look the selector up in public signature databases, recover the
 * function signature, and decode the calldata generically. Only if EVERY source
 * fails (or nothing decodes) do we fall back to blind signing, with a clear
 * "couldn't verify" warning.
 *
 * Sources (queried in parallel, results merged for maximum coverage):
 *   1. Sourcify 4byte (api.4byte.sourcify.dev) — openchain-compatible API; the
 *      openchain.xyz database is migrating here.
 *   2. OpenChain (api.openchain.xyz) — legacy, still up during the migration.
 *   3. 4byte.directory — the original community database.
 * (1) and (2) share the openchain response shape (spam-filtered, most-likely
 * first); collisions are possible, so the caller tries each candidate and keeps
 * the one that decodes cleanly.
 */

const TIMEOUT_MS = 6_000;

// selector (0x + 8 hex, lowercased) → candidate signatures (most-likely first) | null
const cache = new Map<string, string[] | null>();

/** Clear the selector cache (tests). */
export function clearSelectorCache(): void {
  cache.clear();
}

async function getJson(url: string): Promise<any | null> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const res = await fetch(url, { headers: { Accept: 'application/json' }, signal: controller.signal });
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/** OpenChain-compatible hosts (Sourcify first — openchain is migrating to it). */
const OPENCHAIN_HOSTS = [
  'https://api.4byte.sourcify.dev',
  'https://api.openchain.xyz',
];

/** OpenChain-compatible lookup: spam-filtered, ordered most-likely-first. */
async function fromOpenChainHost(host: string, selector: string): Promise<string[]> {
  const data = await getJson(`${host}/signature-database/v1/lookup?function=${selector}&filter=true`);
  const entries = data?.result?.function?.[selector];
  if (!Array.isArray(entries)) return [];
  return entries.map((e: any) => e?.name).filter((s: any): s is string => typeof s === 'string' && s.includes('('));
}

/** 4byte.directory: sort by id ascending — the lowest id is the canonical one. */
async function from4byte(selector: string): Promise<string[]> {
  const data = await getJson(`https://www.4byte.directory/api/v1/signatures/?hex_signature=${selector}`);
  const results = data?.results;
  if (!Array.isArray(results)) return [];
  return results
    .slice()
    .sort((a: any, b: any) => (a?.id ?? 0) - (b?.id ?? 0))
    .map((r: any) => r?.text_signature)
    .filter((s: any): s is string => typeof s === 'string' && s.includes('('));
}

/**
 * Look up candidate function signatures for a 4-byte selector.
 * Returns most-likely-first, deduped. `[]` when nothing is found.
 */
export async function lookupSelector(selectorHex: string): Promise<string[]> {
  const selector = (selectorHex.startsWith('0x') ? selectorHex : '0x' + selectorHex).slice(0, 10).toLowerCase();
  if (selector.length !== 10) return [];
  const cached = cache.get(selector);
  if (cached !== undefined) return cached ?? [];

  // Query every source in parallel; openchain-compatible hosts are higher signal
  // (spam-filtered) so they lead the merge, then 4byte.directory fills gaps.
  const results = await Promise.all([
    ...OPENCHAIN_HOSTS.map((h) => fromOpenChainHost(h, selector).catch(() => [] as string[])),
    from4byte(selector).catch(() => [] as string[]),
  ]);

  const merged: string[] = [];
  const seen = new Set<string>();
  for (const sig of results.flat()) {
    if (!seen.has(sig)) { seen.add(sig); merged.push(sig); }
  }

  cache.set(selector, merged.length ? merged : null);
  return merged;
}
