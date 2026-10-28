// Ported from src/services/recipient-risk.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
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
 * are cached per address for the session. A history that could not be read
 * is not an empty one (round 3): it answers "unknown", said as that.
 */
import { poolRpcCall } from './rpc-pool';
import { readTransactionHistory } from './records';

export interface RecipientRisk {
	/** true = bytecode present (contract); false = EOA; null = unknown. */
	isContract: boolean | null;
	/**
	 * true = no prior *outgoing* transfer to this address in local history;
	 * null = the history could not be read — never "first time" on that.
	 */
	firstInteraction: boolean | null;
	/** The local history could not be read: the confirm says so, with a retry. */
	historyUnread: boolean;
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

/**
 * Have we ever sent (outgoing transfer/dapp tx) to this address before?
 * `null` when the history could not be read: a failed read is not "never"
 * (round 3 — it read as `false`, and the confirm tagged every address
 * "First time sending here" over a store it could not open).
 */
async function hasPriorInteraction(addr: string): Promise<boolean | null> {
	try {
		const lc = addr.toLowerCase();
		const txs = await readTransactionHistory();
		return txs.some(
			(t) =>
				t?.to?.toLowerCase() === lc &&
				(t.type === 'send' || t.type === 'dapp_tx' || t.type === undefined)
		);
	} catch {
		return null;
	}
}

/**
 * Resolve recipient-risk signals for a destination address. Never throws; the
 * `isContract` field is null when the chain couldn't be reached.
 */
export async function resolveRecipientRisk(
	chainId: number,
	address: string
): Promise<RecipientRisk> {
	if (!/^0x[0-9a-fA-F]{40}$/.test(address)) {
		return { isContract: null, firstInteraction: false, historyUnread: false };
	}
	const [isContract, prior] = await Promise.all([
		within(CODE_WAIT_MS, isContractAddress(chainId, address)),
		hasPriorInteraction(address)
	]);
	return {
		isContract,
		firstInteraction: prior === null ? null : !prior,
		historyUnread: prior === null
	};
}

/**
 * How long the contract read may hold the answer (send-18). The first-time
 * check is local and quick; a slow chain must not keep it from the confirm,
 * which waits for this answer a bounded while. Past it: unknown, as an
 * unreachable chain is — the read goes on and fills the cache.
 */
export const CODE_WAIT_MS = 3_000;

function within<T>(ms: number, read: Promise<T | null>): Promise<T | null> {
	return new Promise((resolve) => {
		const timer = setTimeout(() => resolve(null), ms);
		void read.then(
			(value) => {
				clearTimeout(timer);
				resolve(value);
			},
			() => {
				clearTimeout(timer);
				resolve(null);
			}
		);
	});
}
