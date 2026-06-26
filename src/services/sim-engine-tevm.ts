/**
 * Tevm simulation engine — the optional, provider-independent fallback.
 *
 * STATUS: present but DISABLED by default. It runs only after an explicit
 * `enableTevmFallback(true)`, and even then only if `tevm` actually loads in the
 * host runtime. This is deliberate:
 *
 *   - `tevm` ships a full JS EVM (ethereumjs). It is NOT a dependency of this
 *     app, and bundling it under Metro/Hermes is unverified. So this module
 *     never lets the bundler see an `import('tevm')` to resolve — not even a
 *     computed specifier, which Metro's dependency collector still rejects at
 *     build time (it breaks `expo export`). Instead it loads through a
 *     `new Function('m','return import(m)')` indirection — the same escape the
 *     QR scanner uses for its WASM module — so the bundle builds with `tevm`
 *     absent, and the import only resolves at runtime when one is installed.
 *   - Turning it on is a deliberate follow-up: `npm i tevm`, verify it bundles,
 *     then `enableTevmFallback(true)` at app start.
 *
 * When it does run it forks the chain from the user's own RPC (same pool as
 * everything else), replays the inner call(s), and reuses the SAME pure
 * `deriveAssetDeltas` core as the RPC engine — so the asset-diff semantics are
 * identical across engines. It returns `null` whenever it can't answer, so the
 * orchestrator simply moves on.
 */
import { getChainRpcUrl } from '@/services/rpc-pool';
import {
  deriveAssetDeltas, parseRevertReason,
  type EngineResult, type SimCall, type SimLog,
} from '@/services/sim-assets';

// The import lives inside a `new Function` body so the bundler never parses an
// `import('tevm')` to resolve — a computed specifier alone is NOT enough, Metro
// rejects `import(variable)` during dependency collection. This only resolves at
// runtime, when `tevm` is actually installed. Typed as `any`: there are no
// `tevm` type declarations installed, and that's fine.
const TEVM_SPECIFIER = 'tevm';
const dynamicImport = new Function('m', 'return import(m)') as (m: string) => Promise<any>;

let enabled = false;

/** Opt into the Tevm fallback (off by default). See the file header before enabling. */
export function enableTevmFallback(on: boolean): void {
  enabled = on;
}

/** Whether the Tevm fallback is currently armed. */
export function isTevmFallbackEnabled(): boolean {
  return enabled;
}

async function loadTevm(): Promise<any | null> {
  try {
    const mod: any = await dynamicImport(TEVM_SPECIFIER);
    return mod ?? null;
  } catch {
    return null; // not installed / can't load in this runtime
  }
}

/**
 * Simulate `calls` on a local fork and net the transfers for `from`. Returns
 * `null` unless the fallback is enabled, `tevm` loads, and a fork URL resolves.
 */
export async function tevmSimulate(
  from: string,
  calls: SimCall[],
  chainId: number,
): Promise<EngineResult | null> {
  if (!enabled || !calls.length || !calls[0]?.to) return null;

  const tevm = await loadTevm();
  if (!tevm?.createMemoryClient || !tevm?.http) return null;

  const forkUrl = await getChainRpcUrl(chainId).catch(() => null);
  if (!forkUrl) return null;

  try {
    const client = tevm.createMemoryClient({ fork: { transport: tevm.http(forkUrl) } });

    let ok = true;
    let revertReason: string | undefined;
    const logs: SimLog[] = [];

    for (const c of calls) {
      // addToBlockchain: persist each call's state so sequential calls (e.g.
      // approve → transferFrom in a batch) see prior effects, matching the RPC
      // engine's single-block semantics.
      const r: any = await client.tevmCall({
        from,
        to: c.to,
        data: c.data && c.data !== '0x' ? c.data : undefined,
        value: c.value && c.value !== '0x' ? BigInt(c.value) : undefined,
        skipBalance: true,
        addToBlockchain: true,
        throwOnFail: false,
      });

      if (Array.isArray(r?.errors) && r.errors.length > 0) {
        ok = false;
        if (!revertReason) {
          revertReason = parseRevertReason({ message: r.errors[0]?.message, data: r.errors[0]?.data });
        }
      }
      for (const lg of r?.logs ?? []) {
        logs.push({ address: lg?.address ?? '', topics: lg?.topics ?? [], data: lg?.data ?? '0x' });
      }
    }

    const deltas = ok ? deriveAssetDeltas(logs, from) : [];
    return { ok, revertReason, deltas };
  } catch {
    return null;
  }
}
