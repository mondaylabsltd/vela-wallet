/**
 * Recipient-risk signals for the signing sheet.
 *
 * Two cheap, high-value checks that catch real losses:
 *   1. firstInteraction — you've never sent to this address before. Address
 *      poisoning seeds your history with look-alike addresses hoping you copy the
 *      wrong one; a "first time" badge makes a never-before-seen counterparty
 *      visible right when it matters.
 *   2. isContract — eth_getCode tells EOA (wallet) from contract. Sending tokens
 *      to a *contract* that isn't meant to receive them (especially the token
 *      contract itself) is a classic irreversible mistake.
 *
 * Both are best-effort: unknown/unreachable → null, never a false alarm. Results
 * are cached per address for the session.
 */
import { poolRpcCall } from '@/services/rpc-pool';
import { loadTransactions } from '@/services/storage';

export interface RecipientRisk {
  /** true = bytecode present (contract); false = EOA; null = unknown. */
  isContract: boolean | null;
  /** true = no prior *outgoing* transfer to this address in local history. */
  firstInteraction: boolean;
}

const codeCache = new Map<string, boolean>(); // `${chainId}:${addr}` → isContract

/** Reset caches (tests / account switch). */
export function clearRecipientRiskCache(): void {
  codeCache.clear();
}

async function isContractAddress(chainId: number, addr: string): Promise<boolean | null> {
  const key = `${chainId}:${addr.toLowerCase()}`;
  const cached = codeCache.get(key);
  if (cached !== undefined) return cached;
  try {
    const res = await poolRpcCall('eth_getCode', [addr, 'latest'], chainId);
    if (res?.error) return null;
    const code = res?.result;
    if (typeof code !== 'string') return null;
    // EIP-7702: a delegated EOA carries code `0xef0100 ++ implAddr` (exactly 23
    // bytes). It is a WALLET with smart-account features — a person's account, not
    // a contract — so it must NOT be badged "Contract". This is increasingly common
    // (smart EOAs; Vela's own accounts delegate), and vitalik.eth already does it.
    if (/^0xef0100[0-9a-fA-F]{40}$/i.test(code)) {
      codeCache.set(key, false);
      return false;
    }
    const isContract = code !== '0x' && code.length > 2;
    codeCache.set(key, isContract);
    return isContract;
  } catch {
    return null; // RPC unreachable — unknown, not a verdict
  }
}

/** Have we ever sent (outgoing transfer/dapp tx) to this address before? */
async function hasPriorInteraction(addr: string): Promise<boolean> {
  try {
    const lc = addr.toLowerCase();
    const txs = await loadTransactions();
    return txs.some(
      (t) => t.to?.toLowerCase() === lc && (t.type === 'send' || t.type === 'dapp_tx' || t.type === undefined),
    );
  } catch {
    return false;
  }
}

/**
 * Resolve recipient-risk signals for a destination address. Never throws; the
 * `isContract` field is null when the chain couldn't be reached.
 */
export async function resolveRecipientRisk(chainId: number, address: string): Promise<RecipientRisk> {
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) {
    return { isContract: null, firstInteraction: false };
  }
  const [isContract, prior] = await Promise.all([
    isContractAddress(chainId, address),
    hasPriorInteraction(address),
  ]);
  return { isContract, firstInteraction: !prior };
}
