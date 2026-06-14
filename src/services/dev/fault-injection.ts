/**
 * Dev-only network fault injection.
 *
 * Reproduces degraded conditions — RPC down, slow RPC, flaky RPC, missing
 * prices — from the browser console, so the degraded-state UX can be validated
 * without waiting to hit them in the wild.
 *
 * Wired in dev only (see `installFaultConsole`, called from app/_layout). Every
 * hook short-circuits on a single `active` boolean when no faults are set, so
 * production code paths are unaffected (and the console object is never installed
 * outside `__DEV__`).
 *
 * Usage (browser console):
 *   vela.failRpc(137)      // force every RPC call on Polygon to fail
 *   vela.failRpc('all')    // force every chain's RPC to fail
 *   vela.slowRpc(4000)     // add 4s latency to every RPC call
 *   vela.flakyRpc(0.5)     // randomly fail 50% of RPC calls
 *   vela.nullPrice(1)      // Ethereum tokens load but have no price (undercount)
 *   vela.clear()           // reset everything
 *   vela.status()          // print active faults
 *   vela.help()            // print this list
 */

interface Faults {
  rpcFailAll: boolean;
  rpcFailChains: Set<number>;
  rpcLatencyMs: number;
  rpcFailRate: number; // 0..1
  priceNullAll: boolean;
  priceNullChains: Set<number>;
}

const faults: Faults = {
  rpcFailAll: false,
  rpcFailChains: new Set(),
  rpcLatencyMs: 0,
  rpcFailRate: 0,
  priceNullAll: false,
  priceNullChains: new Set(),
};

/** Fast-path flag, recomputed on every mutation so hooks cost one boolean read. */
let active = false;

function recompute(): void {
  active =
    faults.rpcFailAll ||
    faults.rpcFailChains.size > 0 ||
    faults.rpcLatencyMs > 0 ||
    faults.rpcFailRate > 0 ||
    faults.priceNullAll ||
    faults.priceNullChains.size > 0;
}

// ---------------------------------------------------------------------------
// Hooks (called from hot paths — must stay cheap)
// ---------------------------------------------------------------------------

/** True if an RPC call on this chain should be forced to fail right now. */
export function rpcShouldFail(chainId: number): boolean {
  if (!active) return false;
  if (faults.rpcFailAll || faults.rpcFailChains.has(chainId)) return true;
  if (faults.rpcFailRate > 0 && Math.random() < faults.rpcFailRate) return true;
  return false;
}

/** Artificial latency (ms) to add before an RPC call. 0 = none. */
export function rpcLatencyMs(): number {
  return active ? faults.rpcLatencyMs : 0;
}

/** True if token prices on this chain should be nulled out (simulate undercount). */
export function priceShouldNull(chainId: number): boolean {
  if (!active) return false;
  return faults.priceNullAll || faults.priceNullChains.has(chainId);
}

// ---------------------------------------------------------------------------
// Console API
// ---------------------------------------------------------------------------

const CHAIN_NAMES: Record<number, string> = {
  1: 'Ethereum', 56: 'BNB', 137: 'Polygon', 42161: 'Arbitrum',
  10: 'Optimism', 8453: 'Base', 43114: 'Avalanche', 100: 'Gnosis',
};

function chainLabel(id: number): string {
  return CHAIN_NAMES[id] ? `${CHAIN_NAMES[id]} (${id})` : `chain ${id}`;
}

function describe(): string[] {
  const lines: string[] = [];
  if (faults.rpcFailAll) lines.push('• RPC: failing on ALL chains');
  else if (faults.rpcFailChains.size) lines.push(`• RPC: failing on ${[...faults.rpcFailChains].map(chainLabel).join(', ')}`);
  if (faults.rpcLatencyMs) lines.push(`• RPC latency: +${faults.rpcLatencyMs}ms per call`);
  if (faults.rpcFailRate) lines.push(`• RPC flaky: ${Math.round(faults.rpcFailRate * 100)}% of calls fail`);
  if (faults.priceNullAll) lines.push('• Prices: nulled on ALL chains (tokens show, total drops)');
  else if (faults.priceNullChains.size) lines.push(`• Prices: nulled on ${[...faults.priceNullChains].map(chainLabel).join(', ')}`);
  return lines;
}

const HELP = [
  'Vela fault injection — simulate degraded network conditions:',
  '',
  "  vela.failRpc(chainId | 'all')   force RPC calls to fail (also stops activity — same RPC layer)",
  '  vela.slowRpc(ms)                add latency to every RPC call',
  '  vela.flakyRpc(rate 0..1)        randomly fail that fraction of RPC calls',
  "  vela.nullPrice(chainId | 'all') tokens load but have no price → total undercounts",
  '  vela.clear()                    reset all faults',
  '  vela.status()                   show active faults',
  '  vela.help()                     show this help',
  '',
  '  Chain IDs: 1 Ethereum · 56 BNB · 137 Polygon · 42161 Arbitrum · 10 Optimism · 8453 Base · 43114 Avalanche · 100 Gnosis',
  '  Also available: velaSimulateReceipt(amount, token), velaVoiceTest(amount, token)',
].join('\n');

type ChainArg = number | 'all';

/**
 * Install the `vela` console namespace. Idempotent; safe to call once at startup.
 * Returns the api object (also for tests).
 */
export function installFaultConsole(): void {
  const g = globalThis as any;

  const toggleChain = (set: Set<number>, allKey: 'rpcFailAll' | 'priceNullAll', arg: ChainArg) => {
    if (arg === 'all') { faults[allKey] = true; }
    else { set.add(arg); }
    recompute();
  };

  const api = {
    failRpc(chain: ChainArg) {
      toggleChain(faults.rpcFailChains, 'rpcFailAll', chain);
      return api.status();
    },
    slowRpc(ms: number) {
      faults.rpcLatencyMs = Math.max(0, ms | 0);
      recompute();
      return api.status();
    },
    flakyRpc(rate: number) {
      faults.rpcFailRate = Math.min(1, Math.max(0, rate));
      recompute();
      return api.status();
    },
    nullPrice(chain: ChainArg) {
      toggleChain(faults.priceNullChains, 'priceNullAll', chain);
      return api.status();
    },
    clear() {
      faults.rpcFailAll = false;
      faults.rpcFailChains.clear();
      faults.rpcLatencyMs = 0;
      faults.rpcFailRate = 0;
      faults.priceNullAll = false;
      faults.priceNullChains.clear();
      recompute();
      console.log('[vela] faults cleared');
      return 'cleared';
    },
    status() {
      const lines = describe();
      console.log(lines.length ? ['[vela] active faults:', ...lines].join('\n') : '[vela] no active faults');
      return lines.length ? lines.join('; ') : 'none';
    },
    help() {
      console.log(HELP);
      return undefined;
    },
  };

  g.vela = Object.assign(g.vela ?? {}, api);
  console.log('[vela] fault injection ready — run vela.help()');
}
