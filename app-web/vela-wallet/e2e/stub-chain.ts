/**
 * The hermetic chain harness (spec 025 D11 / FR-108).
 *
 * Every read-path e2e runs against THIS, never a live chain: a default-deny
 * interception net over all off-origin traffic, with per-suite JSON-RPC and
 * HTTP handlers layered on top. 026's parallel-space port extends the same
 * seams with bundler stubs.
 */
import type { Page, Route } from '@playwright/test';

export type RpcHandler = (method: string, params: unknown[], url: string) => unknown | undefined;

/**
 * Deny-by-default for everything that is not the app itself. Register BEFORE
 * any page.goto: silence about a missed stub must be a failed request, never
 * a live call escaping CI.
 */
export async function denyOffOrigin(page: Page): Promise<void> {
	await page.route(/^https?:\/\/(?!localhost|127\.0\.0\.1)/, (route) => route.abort('failed'));
}

/** Answer JSON-RPC POSTs on matching URLs; non-matching methods 500. */
export async function stubJsonRpc(
	page: Page,
	urlPattern: RegExp,
	handle: RpcHandler
): Promise<void> {
	await page.route(urlPattern, async (route: Route) => {
		const request = route.request();
		if (request.method() !== 'POST') return route.abort('failed');
		let body: { method?: string; params?: unknown[]; id?: number };
		try {
			body = JSON.parse(request.postData() ?? '{}');
		} catch {
			return route.fulfill({ status: 400, body: 'bad json' });
		}
		const result = handle(body.method ?? '', body.params ?? [], request.url());
		if (result === undefined) return route.fulfill({ status: 500, body: 'unhandled method' });
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({ jsonrpc: '2.0', id: body.id ?? 1, result })
		});
	});
}

/** A fixed non-JSON failure (auth wall, rate limit…) on matching URLs. */
export async function stubHttpFailure(
	page: Page,
	urlPattern: RegExp,
	status: number
): Promise<void> {
	await page.route(urlPattern, (route) => route.fulfill({ status, body: 'no' }));
}

/** Read a key from the app's IndexedDB KV (the executors' storage). */
export async function readKv(page: Page, key: string): Promise<string | null> {
	return page.evaluate(
		(k) =>
			new Promise<string | null>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readonly');
					const get = tx.objectStore('kv').get(k);
					get.onsuccess = () => resolve(typeof get.result === 'string' ? get.result : null);
					get.onerror = () => reject(get.error);
				};
			}),
		key
	);
}

/** Drive one pool-routed read through the gated dev console. */
export async function poolCall(
	page: Page,
	method: string,
	params: unknown[],
	chainId: number
): Promise<unknown> {
	// The console arrives by DYNAMIC import after mount (so no core glue rides
	// the first-paint chunk) — a caller right after a reload must wait for it,
	// or a slow worker races the import and reads `undefined`.
	await page.waitForFunction(
		() =>
			typeof (window as unknown as { vela?: { poolCall?: unknown } }).vela?.poolCall === 'function',
		undefined,
		{ timeout: 15_000 }
	);
	return page.evaluate(
		async ({ method, params, chainId }) => {
			const vela = (
				window as unknown as {
					vela: { poolCall(m: string, p: unknown[], c: number): Promise<unknown> };
				}
			).vela;
			return vela.poolCall(method, params, chainId);
		},
		{ method, params, chainId }
	);
}

/**
 * ABI-encode `aggregate3 -> Result[]` — the inverse of the app's
 * `decAggregate3`. A dynamic array of dynamic `(bool success, bytes
 * returnData)` tuples: head offset, length, per-element offsets relative to
 * the offsets table, then each element as [success][bytes offset=0x40][bytes
 * length][bytes data, right-padded].
 */
export function encodeAggregate3Result(results: { success: boolean; data: string }[]): string {
	const word = (n: number | bigint) => BigInt(n).toString(16).padStart(64, '0');
	const elements = results.map((r) => {
		const hex = r.data.startsWith('0x') ? r.data.slice(2) : r.data;
		const padded = hex.padEnd(Math.ceil(hex.length / 64) * 64, '0');
		return word(r.success ? 1 : 0) + word(0x40) + word(hex.length / 2) + padded;
	});
	let offset = results.length * 32;
	const offsets = elements.map((e) => {
		const here = offset;
		offset += e.length / 2;
		return word(here);
	});
	return '0x' + word(0x20) + word(results.length) + offsets.join('') + elements.join('');
}

/** How many `Call3`s an `aggregate3` calldata carries (the array length word). */
export function aggregate3CallCount(calldata: string): number {
	const hex = calldata.startsWith('0x') ? calldata.slice(2) : calldata;
	// selector (8) + head offset (64) + length (64)
	return parseInt(hex.slice(8 + 64, 8 + 128), 16);
}

/** A 32-byte word from a number/bigint, `0x`-less. */
export function abiWord(n: number | bigint): string {
	return BigInt(n).toString(16).padStart(64, '0');
}

/** Seed the settings editor's per-network RPC overrides (its stored shape). */
export async function seedNetworkOverrides(
	page: Page,
	entries: { chainId: number; rpcURL: string }[]
): Promise<void> {
	await page.evaluate((rows) => {
		return new Promise<void>((resolve, reject) => {
			const open = indexedDB.open('vela', 1);
			open.onupgradeneeded = () => open.result.createObjectStore('kv');
			open.onerror = () => reject(open.error);
			open.onsuccess = () => {
				const tx = open.result.transaction('kv', 'readwrite');
				tx.objectStore('kv').put(JSON.stringify(rows), 'vela.networkConfig');
				tx.oncomplete = () => resolve();
				tx.onerror = () => reject(tx.error);
			};
		});
	}, entries);
}

/**
 * The chain registry (ethereum-data): `/chains/eip155-{id}.json` answers the
 * given document or 404; the search index answers empty.
 */
export async function stubChainRegistry(
	page: Page,
	docs: Record<number, Record<string, unknown> | null>
): Promise<void> {
	await page.route(/\/chains\/eip155-(\d+)\.json$/, (route) => {
		const id = Number(/eip155-(\d+)\.json$/.exec(route.request().url())?.[1]);
		const doc = docs[id];
		if (!doc) return route.fulfill({ status: 404, body: 'no such chain' });
		return route.fulfill({ contentType: 'application/json', body: JSON.stringify(doc) });
	});
	await page.route(/\/index\/fuse-chains\.json$/, (route) =>
		route.fulfill({ contentType: 'application/json', body: JSON.stringify({ data: [] }) })
	);
}

// ---------------------------------------------------------------------------
// The relay (spec 026 T224)
// ---------------------------------------------------------------------------

/** What a stubbed relay answers, per JSON-RPC method. */
export type RelayHandler = (
	method: string,
	params: unknown[]
) => unknown | { error: { code: number; message: string } } | undefined;

export interface RelayRestAnswers {
	/** `/v1/account/{chainId}/{safe}` — the per-Safe gas deposit account. */
	account?: Record<string, unknown> | null;
	/** `/v1/treasury/{chainId}` — the relay's float. `null` ⇒ 404 (uncovered). */
	treasury?: Record<string, unknown> | null;
	/** `/v1/sponsor` — the silent sponsorship attempt. */
	sponsor?: Record<string, unknown> | null;
}

/**
 * Stub the relay: the ERC-4337 JSON-RPC methods the wallet speaks, plus the
 * three REST endpoints. Both halves are needed for a send — the quote and the
 * submit come over JSON-RPC, the gas account and the treasury over REST.
 *
 * `handler` answers the JSON-RPC methods; returning `undefined` falls through
 * to a null result, which is what a relay says for a receipt that has not
 * landed. Register this AFTER `stubJsonRpc` so the relay's origin wins.
 */
export async function stubRelay(
	page: Page,
	urlPattern: RegExp,
	handler: RelayHandler,
	rest: RelayRestAnswers = {}
): Promise<void> {
	await page.route(/\/v1\/account\/(\d+)\/(0x[0-9a-fA-F]+)/, (route) => {
		if (rest.account === null) return route.fulfill({ status: 404, body: 'no account' });
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify(
				rest.account ?? {
					activeDepositAddress: '0x' + 'a1'.repeat(20),
					onchainBalance: '0x2386f26fc10000', // 0.01
					spendableBalance: '0x2386f26fc10000',
					status: 'ACTIVE'
				}
			)
		});
	});
	await page.route(/\/v1\/treasury\/(\d+)/, (route) => {
		if (rest.treasury === null) return route.fulfill({ status: 404, body: 'uncovered' });
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify(
				rest.treasury ?? {
					address: '0x' + 'b2'.repeat(20),
					asset: 'native',
					balance: '0x8ac7230489e80000', // 10
					floor: '0x2386f26fc10000',
					bootstrapNeeded: false
				}
			)
		});
	});
	await page.route(/\/v1\/sponsor/, (route) =>
		route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify(rest.sponsor ?? { sponsored: true })
		})
	);
	await page.route(urlPattern, async (route) => {
		const request = route.request();
		if (request.method() !== 'POST') return route.fallback();
		const body = request.postDataJSON() as { id?: number; method: string; params?: unknown[] };
		const answer = handler(body.method, body.params ?? []);
		const payload =
			answer !== null && typeof answer === 'object' && 'error' in (answer as object)
				? { jsonrpc: '2.0', id: body.id ?? 1, error: (answer as { error: unknown }).error }
				: { jsonrpc: '2.0', id: body.id ?? 1, result: answer ?? null };
		return route.fulfill({ contentType: 'application/json', body: JSON.stringify(payload) });
	});
}

/**
 * A relay that quotes, accepts and confirms — the happy path, as one object.
 * `receipt` decides what the receipt poll answers: 'landed' (confirmed),
 * 'pending' (accepted, no receipt yet) or 'failed' (reverted on-chain).
 */
export function happyRelay(
	userOpHash: string,
	txHash: string,
	receipt: () => 'landed' | 'pending' | 'failed' = () => 'landed'
): RelayHandler {
	return (method) => {
		switch (method) {
			case 'pimlico_getUserOperationGasPrice':
				return {
					slow: { maxFeePerGas: '0x3b9aca00', maxPriorityFeePerGas: '0x3b9aca00' },
					standard: { maxFeePerGas: '0x3b9aca00', maxPriorityFeePerGas: '0x3b9aca00' },
					fast: { maxFeePerGas: '0x77359400', maxPriorityFeePerGas: '0x77359400' }
				};
			case 'eth_estimateUserOperationGas':
				return {
					preVerificationGas: '0xc350',
					verificationGasLimit: '0x186a0',
					callGasLimit: '0x186a0'
				};
			case 'vela_getInBandGasQuote':
				// The relay's in-band rows: the fee is paid from the Safe's own
				// balance to this recipient. Every chain is in-band, so a send
				// cannot be priced without them.
				return [
					{
						recipient: '0x' + 'fe'.repeat(20),
						asset: 'native',
						feeToken: null,
						balance: '0x14d1120d7b160000',
						decimals: 18,
						symbol: 'ETH',
						usdBalance: '4500',
						usdPrice: '3000'
					}
				];
			case 'eth_sendUserOperation':
				return userOpHash;
			// The relay's own method and statuses (spec 082 RA7, G13): the web
			// used to ask `eth_getUserOperationStatus`, which the relay never
			// served, and `pending` is not a status it ever answers.
			case 'pimlico_getUserOperationStatus':
				return { status: receipt() === 'pending' ? 'submitted' : 'included' };
			case 'eth_getUserOperationReceipt': {
				const state = receipt();
				if (state === 'pending') return null;
				return {
					userOpHash,
					success: state === 'landed',
					receipt: {
						transactionHash: txHash,
						status: state === 'landed' ? '0x1' : '0x0',
						blockNumber: '0x11',
						blockHash: '0x' + 'cd'.repeat(32),
						gasUsed: '0x5208',
						logs: []
					}
				};
			}
			default:
				return undefined;
		}
	};
}
