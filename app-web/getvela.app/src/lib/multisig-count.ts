/**
 * The multisig count on the homepage, over plain JSON-RPC (not `#lib/evm/index.ts`: the
 * landing page must not carry ABI and signing code for one number).
 *
 * Production chains only — a testnet's seeded records are not accounts. Per
 * chain, in one request: is the registry there (`eth_getCode`), and how many
 * records (`totalRecords()`). The same Safe may be recorded on Gnosis AND Ethereum, so
 * the counts are never added: the figure is the larger of them, a count of
 * distinct Safes that can only err low.
 */
import { GENESIS_REGISTRY_ADDRESS, MULTISIG_CHAINS, type MultisigChain } from './registry-chains';

/** `totalRecords()` */
const TOTAL_RECORDS = '0x125f8974';

type Fetch = typeof fetch;

export type ChainCount = { state: 'not-deployed' } | { state: 'deployed'; count: number };

export type MultisigCount =
	{ state: 'not-deployed' } | { state: 'deployed'; count: number; chains: string[] };

/** Only a hex string is an answer; some nodes put errors inside `result`. */
function hexResult(reply: unknown): string {
	const result = (reply as { result?: unknown } | null)?.result;
	if (typeof result !== 'string' || !/^0x[0-9a-fA-F]*$/.test(result)) throw new Error('no answer');
	return result;
}

/**
 * One chain's count, failing over across its RPCs; null when none answered.
 * Is the registry there (`eth_getCode`) and how many records
 * (`totalRecords()`) go out together, as one JSON-RPC batch: one request per
 * chain, not two after one another.
 */
export async function readChainCount(
	chain: Pick<MultisigChain, 'rpcs'>,
	fetchFn: Fetch = fetch
): Promise<ChainCount | null> {
	const body = JSON.stringify([
		{ jsonrpc: '2.0', id: 0, method: 'eth_getCode', params: [GENESIS_REGISTRY_ADDRESS, 'latest'] },
		{
			jsonrpc: '2.0',
			id: 1,
			method: 'eth_call',
			params: [{ to: GENESIS_REGISTRY_ADDRESS, data: TOTAL_RECORDS }, 'latest']
		}
	]);
	for (const url of chain.rpcs) {
		try {
			const res = await fetchFn(url, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body
			});
			const replies: unknown = await res.json();
			if (!Array.isArray(replies)) throw new Error('not a batch answer');
			const byId = (id: number) => replies.find((r) => r?.id === id);
			if (hexResult(byId(0)) === '0x') return { state: 'not-deployed' };
			const total = hexResult(byId(1));
			if (total.length < 66) throw new Error('short answer');
			return { state: 'deployed', count: parseInt(total.slice(0, 66), 16) };
		} catch {
			// next endpoint
		}
	}
	return null;
}

/**
 * The homepage figure. Every production chain must answer: one unreadable
 * chain makes the whole figure unknown (null) rather than quietly low — the
 * same rule as the wallet counter beside it.
 */
export async function readMultisigCount(
	fetchFn: Fetch = fetch,
	chains: readonly MultisigChain[] = MULTISIG_CHAINS
): Promise<MultisigCount | null> {
	const production = chains.filter((c) => !c.testnet);
	const counts = await Promise.all(production.map((c) => readChainCount(c, fetchFn)));
	if (counts.some((c) => c === null)) return null;
	let best = -1;
	const deployedOn: string[] = [];
	counts.forEach((c, i) => {
		if (c?.state !== 'deployed') return;
		deployedOn.push(production[i].label);
		best = Math.max(best, c.count);
	});
	return best < 0
		? { state: 'not-deployed' }
		: { state: 'deployed', count: best, chains: deployedOn };
}
