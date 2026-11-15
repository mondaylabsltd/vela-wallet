/**
 * A read-only JSON-RPC client over a list of public endpoints.
 *
 * Each request goes to the first endpoint; if it fails — network error,
 * timeout, HTTP error, an error answer, or an answer that is not JSON-RPC —
 * it moves to the next, and the whole list is tried `1 + retryCount` times
 * before the last error is thrown. No endpoint is trusted for more than the
 * one answer it gives: the caller decodes and checks every result.
 *
 * With `batchSize`, requests made in the same tick go out as one JSON-RPC
 * batch per endpoint (so a page's hundred block-timestamp reads are one
 * round-trip, not a hundred). An item that fails inside a batch is retried
 * on the next endpoint on its own footing, exactly like an unbatched one.
 *
 * `readContracts` folds many contract reads into Multicall3 `aggregate3`
 * calls: a page of a hundred records is one `eth_call`, not a hundred. Each
 * inner answer is decoded and checked exactly as `readContract` would.
 */
import { decodeFunctionResult, encodeFunctionData, type Abi } from './abi';
import { fromQuantity, isHex, toQuantity, type Hex } from './hex';
import { loadEvm } from './wasm';

export class RpcError extends Error {
	constructor(
		message: string,
		readonly code?: number
	) {
		super(message);
		this.name = 'RpcError';
	}
}

export interface ClientOptions {
	timeoutMs?: number;
	/** Extra passes over the whole endpoint list. */
	retryCount?: number;
	/** Batch same-tick requests, at most this many per JSON-RPC batch. */
	batchSize?: number;
	/** Injected for tests. */
	fetch?: typeof fetch;
}

export interface ReadContractArgs {
	address: `0x${string}`;
	abi: Abi;
	functionName: string;
	args?: readonly unknown[];
}

/**
 * Multicall3, at the same address on every chain it is deployed to
 * (github.com/mds1/multicall3) — Gnosis and Ethereum among them. It only
 * relays calls; every answer is still decoded against the caller's own ABI.
 */
export const MULTICALL3_ADDRESS = '0xcA11bde05977b3631167028862bE2a173976CA11' as const;

const AGGREGATE3_ABI = [
	{
		type: 'function',
		name: 'aggregate3',
		stateMutability: 'payable',
		inputs: [
			{
				name: 'calls',
				type: 'tuple[]',
				components: [
					{ name: 'target', type: 'address' },
					{ name: 'allowFailure', type: 'bool' },
					{ name: 'callData', type: 'bytes' }
				]
			}
		],
		outputs: [
			{
				name: 'returnData',
				type: 'tuple[]',
				components: [
					{ name: 'success', type: 'bool' },
					{ name: 'returnData', type: 'bytes' }
				]
			}
		]
	}
] as const satisfies Abi;

/** Calls per `aggregate3`: keeps each `eth_call` well under public nodes'
 *  gas and response caps. A longer list is split, and the parts go out
 *  together (as one JSON-RPC batch when the client batches). */
const MULTICALL_CHUNK = 100;

export interface ChainClient {
	request<T = unknown>(args: { method: string; params?: readonly unknown[] }): Promise<T>;
	readContract<T = unknown>(args: ReadContractArgs): Promise<T>;
	/** Many reads through Multicall3, answers in the order asked. Rejects if
	 *  any one fails — the same all-or-nothing as `Promise.all` of
	 *  `readContract`, in a handful of requests instead of one per read. */
	readContracts(calls: readonly ReadContractArgs[]): Promise<unknown[]>;
	getBlock(args: { blockNumber: bigint }): Promise<{ number: bigint; timestamp: bigint }>;
}

interface Pending {
	method: string;
	params: readonly unknown[];
	resolve: (value: unknown) => void;
	reject: (error: unknown) => void;
}

interface RpcReply {
	id?: unknown;
	result?: unknown;
	error?: { code?: number; message?: string };
}

let nextId = 1;

export function createClient(rpcs: readonly string[], options: ClientOptions = {}): ChainClient {
	if (rpcs.length === 0) throw new Error('no RPC endpoints');
	const timeoutMs = options.timeoutMs ?? 10_000;
	const passes = 1 + (options.retryCount ?? 1);
	const doFetch = options.fetch ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
	const order = Array.from({ length: passes }, () => rpcs).flat();

	async function post(url: string, body: unknown): Promise<unknown> {
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), timeoutMs);
		try {
			const res = await doFetch(url, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(body),
				signal: controller.signal
			});
			if (!res.ok) throw new RpcError(`HTTP ${res.status} from ${url}`);
			return await res.json();
		} finally {
			clearTimeout(timer);
		}
	}

	function settle(reply: RpcReply | undefined): unknown {
		if (!reply || typeof reply !== 'object') throw new RpcError('not a JSON-RPC answer');
		if (reply.error) throw new RpcError(reply.error.message ?? 'RPC error', reply.error.code);
		if (!('result' in reply)) throw new RpcError('JSON-RPC answer without a result');
		return reply.result;
	}

	/** Send `items` to endpoint `attempt`; whatever fails moves on to the next. */
	async function run(items: Pending[], attempt: number): Promise<void> {
		const url = order[attempt];
		const last = attempt === order.length - 1;
		const failed: Pending[] = [];
		const fail = (p: Pending, e: unknown) => (last ? p.reject(e) : failed.push(p));
		try {
			if (options.batchSize) {
				const ids = items.map(() => nextId++);
				const replies = await post(
					url,
					items.map((p, i) => ({ jsonrpc: '2.0', id: ids[i], method: p.method, params: p.params }))
				);
				if (!Array.isArray(replies)) throw new RpcError('batch answer is not an array');
				const byId = new Map<unknown, RpcReply>(replies.map((r: RpcReply) => [r?.id, r]));
				items.forEach((p, i) => {
					try {
						p.resolve(settle(byId.get(ids[i])));
					} catch (e) {
						fail(p, e);
					}
				});
			} else {
				const [p] = items;
				p.resolve(
					settle(
						(await post(url, {
							jsonrpc: '2.0',
							id: nextId++,
							method: p.method,
							params: p.params
						})) as RpcReply
					)
				);
			}
		} catch (e) {
			for (const p of items) fail(p, e);
		}
		if (failed.length) await run(failed, attempt + 1);
	}

	let queue: Pending[] = [];
	function flush(): void {
		const all = queue;
		queue = [];
		const size = options.batchSize ?? 1;
		for (let i = 0; i < all.length; i += size) void run(all.slice(i, i + size), 0);
	}

	function request<T = unknown>({
		method,
		params = []
	}: {
		method: string;
		params?: readonly unknown[];
	}): Promise<T> {
		return new Promise<T>((resolve, reject) => {
			const p: Pending = { method, params, resolve: resolve as (v: unknown) => void, reject };
			if (!options.batchSize) {
				void run([p], 0);
				return;
			}
			queue.push(p);
			if (queue.length === 1) setTimeout(flush, 0);
		});
	}

	async function readContract<T = unknown>({
		address,
		abi,
		functionName,
		args = []
	}: ReadContractArgs): Promise<T> {
		await loadEvm();
		const data = encodeFunctionData(abi, functionName, args);
		const result = await request({ method: 'eth_call', params: [{ to: address, data }, 'latest'] });
		if (!isHex(result)) throw new RpcError('eth_call answer is not hex');
		return decodeFunctionResult(abi, functionName, result as Hex) as T;
	}

	async function aggregate(calls: readonly ReadContractArgs[]): Promise<unknown[]> {
		const data = encodeFunctionData(AGGREGATE3_ABI, 'aggregate3', [
			calls.map((c) => ({
				target: c.address,
				// Every call may fail on its own, so a revert names the read that
				// reverted instead of reverting the whole batch anonymously.
				allowFailure: true,
				callData: encodeFunctionData(c.abi, c.functionName, c.args ?? [])
			}))
		]);
		const result = await request({
			method: 'eth_call',
			params: [{ to: MULTICALL3_ADDRESS, data }, 'latest']
		});
		if (!isHex(result)) throw new RpcError('eth_call answer is not hex');
		const answers = decodeFunctionResult(AGGREGATE3_ABI, 'aggregate3', result as Hex) as {
			success: boolean;
			returnData: Hex;
		}[];
		if (answers.length !== calls.length) {
			throw new RpcError(`aggregate3 answered ${answers.length} of ${calls.length} calls`);
		}
		return answers.map((a, i) => {
			const { abi, functionName } = calls[i];
			if (!a.success) throw new RpcError(`${functionName} reverted`);
			return decodeFunctionResult(abi, functionName, a.returnData);
		});
	}

	async function readContracts(calls: readonly ReadContractArgs[]): Promise<unknown[]> {
		if (calls.length === 0) return [];
		await loadEvm();
		const parts: (readonly ReadContractArgs[])[] = [];
		for (let i = 0; i < calls.length; i += MULTICALL_CHUNK) {
			parts.push(calls.slice(i, i + MULTICALL_CHUNK));
		}
		return (await Promise.all(parts.map(aggregate))).flat();
	}

	async function getBlock({ blockNumber }: { blockNumber: bigint }) {
		const block = await request<{ number?: unknown; timestamp?: unknown } | null>({
			method: 'eth_getBlockByNumber',
			params: [toQuantity(blockNumber), false]
		});
		if (!block) throw new RpcError(`block ${blockNumber} not found`);
		return { number: fromQuantity(block.number), timestamp: fromQuantity(block.timestamp) };
	}

	return { request, readContract, readContracts, getBlock };
}
