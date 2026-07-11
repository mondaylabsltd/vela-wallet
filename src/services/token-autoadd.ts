/**
 * Silent auto-add of tokens a CONFIRMED transaction actually delivered, so a swap
 * or bridge output just appears on the next balance sync with zero user action.
 *
 * SECURITY — the whole point of this module is WHERE the data comes from:
 *   ✅ AUTHENTICATED on-chain receipt logs (`eth_getUserOperationReceipt` →
 *      `receipt.logs`): the tx really landed and the token really emitted its own
 *      Transfer to the user. Trustworthy.
 *   ❌ NEVER a sign-time `eth_simulateV1` simulation. Those logs are attacker-
 *      controllable — a hostile dApp can synthesize a fake `Transfer(_, you, big)`
 *      and answer `symbol()` to spoof a token. Adding from them would let a malicious
 *      dApp seed a scam token into the wallet list, which then poisons future-sim
 *      trust (held → trusted) AND the transfer-monitor allowlist. See the asymmetric-
 *      trust model in tx-simulation.ts. Only ever call this with real receipt logs.
 */
import { deriveAssetDeltas, type SimLog } from '@/services/sim-assets';
import { loadCustomTokens, saveCustomToken } from '@/services/storage';
import { clearTokenCache, getCachedHeldTokens } from '@/services/wallet-api';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { knownTokenSymbol } from '@/services/tokens';
import { chainName } from '@/models/network';

/** An eth log as returned in a UserOp/tx receipt — same shape deriveAssetDeltas nets. */
export type ReceiptLog = SimLog;

/**
 * Add any ERC-20 the confirmed tx (whose authentic `logs` are passed) net-delivered
 * to `from` on `chainId`, that isn't already listed/held/known. Silent: the token
 * simply appears on the next balance sync (the caller-agnostic quiet UX the founder
 * wants). Returns how many were added. Never throws. Idempotent (dedupes by id).
 */
export async function autoAddReceivedTokens(
  from: string | undefined,
  chainId: number,
  logs: ReceiptLog[] | undefined,
): Promise<number> {
  if (!from || !Array.isArray(logs) || logs.length === 0) return 0;
  try {
    // Net the AUTHENTIC receipt logs; keep only ERC-20s the user NET-RECEIVED.
    const received = deriveAssetDeltas(logs, from)
      .filter((d) => d.kind === 'erc20' && !!d.token && d.delta > 0n)
      .map((d) => d.token!.toLowerCase());
    if (received.length === 0) return 0;

    // Skip anything already visible: manually-added customs, currently-held tokens,
    // and curated known tokens (stablecoins etc. already render). Avoids overwriting
    // a user's curated entry (saveCustomToken replaces by id) and list noise.
    const alreadyListed = new Set(
      (await loadCustomTokens())
        .filter((t) => t.chainId === chainId)
        .map((t) => t.contractAddress.toLowerCase()),
    );
    const held = new Set(getCachedHeldTokens(from, chainId).map((a) => a.toLowerCase()));
    const fresh = [...new Set(received)].filter(
      (addr) => !alreadyListed.has(addr) && !held.has(addr) && !knownTokenSymbol(addr),
    );
    if (fresh.length === 0) return 0;

    // Resolve on-chain symbol/decimals. resolveTokenMetadata gives no `name`, so
    // default name→symbol (the list shows the symbol; a real name isn't worth an
    // extra call). Skip a token whose symbol can't be resolved (don't seed a "?").
    const meta = await resolveTokenMetadata(chainId, fresh);
    const net = chainName(chainId);
    let added = 0;
    for (const addr of fresh) {
      const m = meta.get(addr);
      if (!m?.symbol) continue;
      await saveCustomToken({
        id: `${chainId}_${addr}`,
        chainId,
        contractAddress: addr,
        symbol: m.symbol,
        name: m.symbol,
        decimals: m.decimals,
        networkName: net,
      });
      added++;
    }
    // saveCustomToken doesn't touch the 5-min fetchTokens cache, so without this the
    // token wouldn't appear until the TTL lapsed. Invalidate so the next sync shows it.
    if (added > 0) clearTokenCache(from);
    return added;
  } catch {
    return 0;
  }
}
