/**
 * RPC simulation engine — `eth_simulateV1`.
 *
 * The primary engine. It executes the inner Safe→target call(s) in a simulated
 * block against live chain state via the user's own RPC pool (no third-party
 * "simulation" service), then derives the wallet's asset deltas from the logs.
 *
 *   - `validation: false`  → don't require the Safe to hold funds/nonce; we're
 *     asking "what would happen", and in the 4337 model the Safe pays no gas.
 *   - `traceTransfers: true` → native value moves come back as synthetic
 *     `Transfer` logs (sender = NATIVE_TRANSFER_SENTINEL), so one log parser
 *     covers both native and ERC-20.
 *
 * Returns `null` when the engine can't answer — the endpoint doesn't implement
 * `eth_simulateV1`, params were rejected, or every RPC was unreachable. The
 * orchestrator treats `null` as "try the next engine / degrade", never as a
 * result. A genuine top-level execution revert still comes back as
 * `{ ok: false }` (per-call `status`/`error`), not `null`.
 */
import { rpcCall } from '@/services/rpc-adapter';
import {
  deriveAssetDeltas, parseRevertReason,
  type EngineResult, type SimCall, type SimLog,
} from '@/services/sim-assets';

/** Normalise a call's value to a hex quantity `eth_simulateV1` accepts. */
function valueParam(value: string | undefined): string {
  return value && value !== '0x' ? value : '0x0';
}

/** A per-call result counts as success only when explicitly status 1. */
function callSucceeded(call: any): boolean {
  const s = call?.status;
  if (s === '0x1' || s === 1) return true;
  if (s === '0x0' || s === 0) return false;
  // Some nodes omit status; absence of an error is our best success signal.
  return call?.error == null;
}

/**
 * Simulate `calls` (executed sequentially in one block, sharing state) sent
 * from `from` (the Safe), and net the resulting transfers for `from`.
 */
export async function rpcSimulate(
  from: string,
  calls: SimCall[],
  chainId: number,
): Promise<EngineResult | null> {
  if (!calls.length || !calls[0]?.to) return null;

  const payload = {
    blockStateCalls: [
      {
        calls: calls.map((c) => ({
          from,
          to: c.to,
          ...(c.data && c.data !== '0x' ? { data: c.data } : {}),
          value: valueParam(c.value),
        })),
      },
    ],
    traceTransfers: true,
    validation: false,
    returnFullTransactions: false,
  };

  let res;
  try {
    res = await rpcCall('eth_simulateV1', [payload, 'latest'], chainId);
  } catch {
    return null; // every endpoint failed — unknown, let the caller degrade
  }

  // A top-level error means the method/params were rejected (unsupported node,
  // bad shape) — not an execution revert. Degrade rather than report a result.
  if (!res || res.error || !Array.isArray(res.result) || res.result.length === 0) {
    return null;
  }

  const callResults: any[] = res.result.flatMap((b: any) =>
    Array.isArray(b?.calls) ? b.calls : [],
  );
  if (callResults.length === 0) return null;

  let ok = true;
  let revertReason: string | undefined;
  const logs: SimLog[] = [];

  for (const call of callResults) {
    if (!callSucceeded(call)) {
      ok = false;
      if (!revertReason) revertReason = parseRevertReason(call?.error);
    }
    if (Array.isArray(call?.logs)) {
      for (const lg of call.logs) {
        logs.push({ address: lg?.address ?? '', topics: lg?.topics ?? [], data: lg?.data ?? '0x' });
      }
    }
  }

  // If it reverts, the EVM discards its effects and emits no transfer logs, so
  // there are no honest deltas to show — keep it empty and let `ok:false` speak.
  const deltas = ok ? deriveAssetDeltas(logs, from) : [];
  return { ok, revertReason, deltas };
}
