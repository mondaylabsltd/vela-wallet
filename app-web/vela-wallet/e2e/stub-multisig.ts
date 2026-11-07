/**
 * A hermetic world for the multisig e2e suites: the genesis service and its
 * proposal mailbox, and the chains a multisig lives on — answered from this
 * Node process, so the real page, its real core and the parallel space's
 * passkeys run against them with nothing leaving the machine (the suites'
 * default-deny net, `stub-chain.ts`).
 *
 * It is the core's own simulated world (`rust/crates/vela-core/tests/
 * multisig_world`), as far as these suites need it: the service records a
 * genesis and serves it back; the mailbox opens a proposal on its first
 * approval and merges the next ones; every chain holds Safe's v1.4.1 code at
 * its canonical addresses, and a multisig is not deployed until a member
 * executes one of its transactions from an outside wallet
 * (`executeAsOutsideOwner`: deployed with its genesis's members, its number
 * moved on, its `ExecutionSuccess` on chain). What a chain does not hold
 * answers `0x`, as a node does for an address with no code; a simulation it
 * cannot run is refused, and the review says it could not check.
 *
 * Every context routed here shares the one world: two "devices" read the same
 * mailbox, as two members' phones read the one service.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { BrowserContext, Page, Route } from '@playwright/test';
import { secp256k1 } from '@noble/curves/secp256k1';
import { multisigCore } from './core-facts';
import { MULTICALL_SEL, answerAggregate3, isAggregate3 } from './stub-multicall';

/** The wallet's default multisig service (`DEFAULT_SERVICE_ENDPOINTS.genesisServiceURL`). */
export const SERVICE = 'https://safe-genesis.getvela.app';
/** Where this world's chains answer: `https://rpc.multisig.test/<chainId>`. */
export const RPC_HOST = 'https://rpc.multisig.test';

/** The parallel space's fixture accounts (Vela accounts, their passkeys in the space). */
export const FIXTURES = {
	one: '0xD400866e00B055B20752a826CD5C89b811de130b',
	two: '0x031d7D57c99CAF891e1C250554691Fd12D84772b',
	three: '0x58cd0ce6A27099220543b31710d7860d75Ba1d3d'
} as const;

/**
 * An outside owner: a plain key (anvil's public dev account #1, a key the
 * whole world knows — it signs nothing of value anywhere).
 */
export const OUTSIDE = {
	address: '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
	key: '59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d'
} as const;

const ZERO = '0x0000000000000000000000000000000000000000';
const SINGLETON_L2 = '0x29fcB43b46531BcA003ddC8FCB67FFE91900C762';
const HANDLER = '0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99';
/** Safe's `fallback_manager.handler.address` slot. */
const FALLBACK_HANDLER_SLOT = '0x6c9a6c4a39284e37ed1cf53d337577d14212a4870fb976a4366c693b939918d5';
/** `keccak256("ExecutionSuccess(bytes32,uint256)")`. */
const EXECUTION_SUCCESS = '0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e';
const SAFE_CODE = join(
	import.meta.dirname,
	'..',
	'..',
	'..',
	'rust',
	'crates',
	'vela-core',
	'tests',
	'vectors',
	'safe-code'
);
/** Safe's own v1.4.1 code at its own addresses: what every chain here holds. */
const CANONICAL: Record<string, string> = Object.fromEntries(
	[
		['0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67', 'safe-proxy-factory-1.4.1'],
		['0x41675C099F32341bf84BFc5382aF534df5C7461a', 'safe-1.4.1'],
		['0x29fcB43b46531BcA003ddC8FCB67FFE91900C762', 'safe-l2-1.4.1'],
		['0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99', 'compatibility-fallback-handler-1.4.1'],
		['0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54', 'safe-to-l2-setup-1.4.1']
	].map(([address, name]) => [
		address.toLowerCase(),
		readFileSync(join(SAFE_CODE, `${name}.hex`), 'utf8').trim()
	])
);

/** A multisig's genesis, as the core builds and the service keeps it. */
export interface Genesis {
	factory: string;
	singleton: string;
	saltNonce: string;
	initializer: string;
	name?: string;
}

export interface Multisig {
	address: string;
	genesis: Genesis;
	owners: string[];
	threshold: number;
}

/** A Safe transaction as the mailbox takes it (quantities hex). */
export interface WireSafeTx {
	to: string;
	value: string;
	data: string;
	operation: number;
	safeTxGas: string;
	baseGas: string;
	gasPrice: string;
	gasToken: string;
	refundReceiver: string;
	nonce: string;
}

/** A plain transfer of `wei` from the multisig, at `nonce`. */
export function transferTx(to: string, wei: bigint, nonce: number): WireSafeTx {
	return {
		to,
		value: `0x${wei.toString(16)}`,
		data: '0x',
		operation: 0,
		safeTxGas: '0x0',
		baseGas: '0x0',
		gasPrice: '0x0',
		gasToken: ZERO,
		refundReceiver: ZERO,
		nonce: `0x${nonce.toString(16)}`
	};
}

/** A multisig of `owners` needing `threshold`, named `name` — the core's genesis for it. */
export async function multisigOf(
	owners: string[],
	threshold: number,
	name: string,
	saltNonce = '0x1'
): Promise<Multisig> {
	const made = await multisigCore<{
		address: string;
		genesis: Genesis;
		owners: string[];
		threshold: number;
	}>('multisigCall', 'genesis', { owners, threshold, name, saltNonce });
	return {
		address: made.address,
		genesis: made.genesis,
		owners: made.owners,
		threshold: Number(made.threshold)
	};
}

/** The stored row the web keeps for a multisig added here (`vela.multisigAccounts`). */
export function storedRow(ms: Multisig, chainIds = [100]): Record<string, unknown> {
	return {
		address: ms.address,
		genesis: ms.genesis,
		owners: ms.owners,
		threshold: ms.threshold,
		recorded: true,
		shared: true,
		chainIds,
		addedMs: Date.now() - 60_000
	};
}

interface Proposal {
	chainId: number;
	safe: string;
	safeTxHash: string;
	safeTx: WireSafeTx;
	proposer: string;
	approvals: Record<string, unknown>[];
	createdMs: number;
	updatedMs: number;
}

const word = (n: number | bigint) => BigInt(n).toString(16).padStart(64, '0');

export class MultisigWorld {
	/** `GET /api/genesis/{safe}` answers, by lowercase address. */
	readonly records = new Map<string, Record<string, unknown>>();
	readonly mailbox: Proposal[] = [];
	/** Every `POST /api/proposals` body, in order. */
	readonly submissions: Record<string, unknown>[] = [];
	/** Every service request, `METHOD path`. */
	readonly asked: string[] = [];
	/** Native balances by `chainId:address` (lowercase); unlisted: none. */
	readonly balances = new Map<string, bigint>();
	/** Deployed multisigs by `chainId:address` (lowercase); unlisted: not deployed. */
	readonly safes = new Map<string, { owners: string[]; threshold: number; nonce: number }>();
	/** Executions on chain: each one's `ExecutionSuccess` in its transaction. */
	readonly executions: { chainId: number; safe: string; hash: string; tx: string }[] = [];
	/** Every JSON-RPC method asked, `chainId method`. */
	readonly rpcAsked: string[] = [];
	/** The chain refuses a search for `ExecutionSuccess`: a history that cannot be read. */
	refuseHistory = false;
	/**
	 * r16-3: the chains' outage — `refused`: every request is refused at
	 * connect; `hung`: no request is ever answered; `null`: they answer.
	 */
	outage: 'refused' | 'hung' | null = null;
	/** r17-1: how many requests the outage refused at connect. */
	refused = 0;
	/**
	 * r16-3: until this time (ms), a network back from an outage still turns
	 * its safety checks away (`eth_call`, `eth_simulateV1` refused at
	 * connect) while it answers everything else — its multisig's own code
	 * first. What Arc did for 1–5 s after R16's outages lifted.
	 */
	checksTurnedAwayUntil = 0;
	/** The node simulates (`eth_simulateV1` runs and succeeds); else it says "not here". */
	simulates = false;

	/** `address`'s native balance on `chainId` here (none unless set). */
	balance(chainId: number, address: string): bigint {
		return this.balances.get(`${chainId}:${address.toLowerCase()}`) ?? 0n;
	}

	/** The service holds `ms`'s genesis, recorded on Gnosis. */
	record(ms: Multisig): void {
		this.records.set(ms.address.toLowerCase(), {
			found: true,
			pending: false,
			genesis: ms.genesis,
			recordedOn: [100],
			readFrom: 100,
			verified: true,
			nameBySafe: true,
			renamedAt: 0
		});
	}

	/** The safeTxHash of `tx` from `safe` on `chainId`, as the core computes it. */
	hashOf(chainId: number, safe: string, tx: WireSafeTx): Promise<string> {
		return multisigCore<string>('multisigCall', 'safeTxHash', { chainId, safe, tx });
	}

	/**
	 * The outside owner proposes `tx` (its typed-data approval over the
	 * safeTxHash, `v` 27/28 — what MetaMask returns), as if from its own
	 * wallet through the connect page. Returns the safeTxHash.
	 */
	async proposeAsOutsideOwner(chainId: number, safe: string, tx: WireSafeTx): Promise<string> {
		const hash = await this.hashOf(chainId, safe, tx);
		const sig = secp256k1.sign(hash.slice(2), OUTSIDE.key);
		const signature = `0x${sig.toCompactHex()}${(27 + (sig.recovery ?? 0)).toString(16)}`;
		this.take({ chainId, safe, safeTx: tx, approval: { owner: OUTSIDE.address, signature } }, hash);
		return hash;
	}

	/** The outside owner approves the proposal `hash` (already in the mailbox). */
	async approveAsOutsideOwner(chainId: number, hash: string): Promise<void> {
		const p = this.mailbox.find(
			(x) => x.chainId === chainId && x.safeTxHash.toLowerCase() === hash.toLowerCase()
		);
		if (!p) throw new Error(`no proposal ${hash}`);
		await this.proposeAsOutsideOwner(chainId, p.safe, p.safeTx);
	}

	/**
	 * The outside owner executes `hash` itself, from its own wallet: the
	 * multisig is deployed with its genesis's members, its number moves on,
	 * and the chain holds the transaction's `ExecutionSuccess` — what a
	 * member's wallet finds there. Returns the transaction's hash.
	 */
	executeAsOutsideOwner(chainId: number, ms: Multisig, hash: string): string {
		const tx = `0x${'e7'.repeat(31)}${(this.executions.length + 1).toString(16).padStart(2, '0')}`;
		const key = `${chainId}:${ms.address.toLowerCase()}`;
		const safe = this.safes.get(key) ?? { owners: ms.owners, threshold: ms.threshold, nonce: 0 };
		this.safes.set(key, { ...safe, nonce: safe.nonce + 1 });
		this.executions.push({ chainId, safe: ms.address, hash: hash.toLowerCase(), tx });
		return tx;
	}

	/** Route `context`'s service and chain traffic here (after its default-deny net). */
	async install(context: BrowserContext | Page): Promise<void> {
		// Default-deny, answered as a refusal every engine reads the same: an
		// aborted request is a CORS failure WebKit reports as the page's own
		// error, which would hide one that is.
		await context.route(/^https?:\/\/(?!localhost|127\.0\.0\.1)/, (route) =>
			route.fulfill({
				status: 503,
				headers: { 'access-control-allow-origin': '*' },
				body: 'not in this world'
			})
		);
		await context.route(`${SERVICE}/**`, (route) => this.service(route));
		await context.route(`${RPC_HOST}/**`, (route) => this.rpc(route));
	}

	private take(
		req: { chainId: number; safe: string; safeTx: WireSafeTx; approval: Record<string, unknown> },
		hash: string
	): { status: string; proposal: Proposal } {
		const now = Date.now();
		const approval = { ...req.approval, createdMs: now };
		const owner = String(req.approval.owner).toLowerCase();
		const at = this.mailbox.find(
			(p) => p.chainId === req.chainId && p.safeTxHash.toLowerCase() === hash.toLowerCase()
		);
		if (at) {
			at.updatedMs = now;
			if (at.approvals.some((a) => String(a.owner).toLowerCase() === owner))
				return { status: 'unchanged', proposal: at };
			at.approvals.push(approval);
			return { status: 'approved', proposal: at };
		}
		const proposal: Proposal = {
			chainId: req.chainId,
			safe: req.safe,
			safeTxHash: hash,
			safeTx: req.safeTx,
			proposer: String(req.approval.owner),
			approvals: [approval],
			createdMs: now,
			updatedMs: now
		};
		this.mailbox.push(proposal);
		return { status: 'created', proposal };
	}

	private async service(route: Route): Promise<void> {
		const request = route.request();
		const url = new URL(request.url());
		const path = url.pathname;
		this.asked.push(`${request.method()} ${path}${url.search}`);
		const json = (status: number, body: unknown) =>
			route.fulfill({
				status,
				contentType: 'application/json',
				headers: { 'access-control-allow-origin': '*' },
				body: JSON.stringify(body)
			});
		if (request.method() === 'OPTIONS')
			return route.fulfill({
				status: 204,
				headers: {
					'access-control-allow-origin': '*',
					'access-control-allow-methods': 'GET, POST, OPTIONS',
					'access-control-allow-headers': 'content-type'
				}
			});
		if (path === '/api/health')
			return json(200, {
				service: 'safe-genesis-registry',
				ok: true,
				homeChainId: 100,
				chains: [{ chainId: 100, registry: true }]
			});
		const genesis = /^\/api\/genesis\/(0x[0-9a-fA-F]{40})$/.exec(path);
		if (genesis) {
			const found = this.records.get(genesis[1].toLowerCase());
			return found ? json(200, found) : json(404, { found: false });
		}
		if (path === '/api/import' && request.method() === 'POST') {
			const body = request.postDataJSON() as { safe?: string; genesis?: Genesis };
			if (body.safe && body.genesis)
				this.records.set(body.safe.toLowerCase(), {
					found: true,
					pending: false,
					genesis: body.genesis,
					recordedOn: [100],
					readFrom: 100,
					verified: true
				});
			return json(200, { status: 'recorded', genesis: body.genesis });
		}
		if (path === '/api/proposals' && request.method() === 'POST') {
			const body = request.postDataJSON() as {
				chainId: number;
				safe: string;
				safeTx: WireSafeTx;
				approval: Record<string, unknown>;
			};
			this.submissions.push(body);
			const hash = await this.hashOf(body.chainId, body.safe, body.safeTx);
			const taken = this.take(body, hash);
			return json(taken.status === 'created' ? 201 : 200, taken);
		}
		const listed = /^\/api\/proposals\/(0x[0-9a-fA-F]{40})$/.exec(path);
		if (listed) {
			const safe = listed[1].toLowerCase();
			const chain = url.searchParams.get('chainId');
			const mine = this.mailbox.filter(
				(p) => p.safe.toLowerCase() === safe && (chain === null || p.chainId === Number(chain))
			);
			if (chain !== null)
				return json(200, {
					members: { deployed: false, owners: [], threshold: '1', nonce: '0' },
					proposals: mine
				});
			const counts = new Map<number, number>();
			for (const p of mine) counts.set(p.chainId, (counts.get(p.chainId) ?? 0) + 1);
			return json(200, {
				total: mine.length,
				chains: [...counts].map(([chainId, count]) => ({ chainId, count })),
				proposals: mine
			});
		}
		if (path.startsWith('/api/task/')) return json(200, { status: 'done' });
		return json(404, { error: 'no route' });
	}

	private async rpc(route: Route): Promise<void> {
		const request = route.request();
		if (request.method() !== 'POST') return route.abort('failed');
		// r16-3: down — refused at once, or never answered at all.
		if (this.outage === 'refused') {
			this.refused += 1;
			return route.abort('connectionrefused');
		}
		if (this.outage === 'hung') return new Promise<void>(() => {});
		const chainId = Number(new URL(request.url()).pathname.slice(1));
		const body = request.postDataJSON() as
			| { id?: number; method: string; params?: unknown[] }
			| { id?: number; method: string; params?: unknown[] }[];
		const calls = Array.isArray(body) ? body : [body];
		const check = (m: string) => m === 'eth_call' || m === 'eth_simulateV1';
		if (Date.now() < this.checksTurnedAwayUntil && calls.some((c) => check(c.method)))
			return route.abort('connectionrefused');
		const one = (call: { id?: number; method: string; params?: unknown[] }) => {
			const answer = this.answer(chainId, call.method, call.params ?? []);
			return 'error' in answer
				? { jsonrpc: '2.0', id: call.id ?? 1, error: answer.error }
				: { jsonrpc: '2.0', id: call.id ?? 1, result: answer.result };
		};
		return route.fulfill({
			contentType: 'application/json',
			headers: { 'access-control-allow-origin': '*' },
			body: JSON.stringify(Array.isArray(body) ? body.map(one) : one(body))
		});
	}

	private answer(
		chainId: number,
		method: string,
		params: unknown[]
	): { result: unknown } | { error: { code: number; message: string } } {
		this.rpcAsked.push(`${chainId} ${method}`);
		const deployed = (address: unknown) =>
			this.safes.get(`${chainId}:${String(address).toLowerCase()}`);
		switch (method) {
			case 'eth_chainId':
				return { result: `0x${chainId.toString(16)}` };
			case 'net_version':
				return { result: String(chainId) };
			case 'eth_blockNumber':
				return { result: '0x100' };
			case 'eth_gasPrice':
			case 'eth_maxPriorityFeePerGas':
				return { result: '0x3b9aca00' };
			case 'eth_getBlockByNumber':
				return {
					result: {
						number: '0x100',
						timestamp: `0x${Math.floor(Date.now() / 1000).toString(16)}`,
						baseFeePerGas: '0x3b9aca00',
						gasLimit: '0x1c9c380',
						hash: `0x${'ab'.repeat(32)}`
					}
				};
			case 'eth_getCode':
				if (deployed(params[0])) return { result: '0x6080' };
				return { result: CANONICAL[String(params[0]).toLowerCase()] ?? '0x' };
			case 'eth_getBalance':
				return { result: `0x${this.balance(chainId, String(params[0])).toString(16)}` };
			case 'eth_getTransactionCount':
				return { result: '0x0' };
			case 'eth_getStorageAt': {
				// A deployed multisig's proxy: Safe's L2 singleton and its handler, no guard.
				const slot = BigInt(String(params[1]));
				if (!deployed(params[0])) return { result: `0x${word(0)}` };
				if (slot === 0n) return { result: `0x${word(SINGLETON_L2)}` };
				if (slot === BigInt(FALLBACK_HANDLER_SLOT)) return { result: `0x${word(HANDLER)}` };
				return { result: `0x${word(0)}` };
			}
			case 'eth_getLogs': {
				const filter = params[0] as { address?: string; topics?: (string | null)[] };
				const hash = String(filter.topics?.[1] ?? '').toLowerCase();
				if (this.refuseHistory && filter.topics?.[0] === EXECUTION_SUCCESS)
					return { error: { code: -32000, message: 'history refused' } };
				return {
					result: this.executions
						.filter(
							(e) =>
								e.chainId === chainId &&
								e.hash === hash &&
								e.safe.toLowerCase() === String(filter.address ?? e.safe).toLowerCase()
						)
						.map((e) => ({
							address: e.safe,
							topics: [EXECUTION_SUCCESS, e.hash],
							data: `0x${word(0)}`,
							transactionHash: e.tx,
							blockNumber: '0x100',
							blockHash: `0x${'ab'.repeat(32)}`,
							logIndex: '0x0',
							transactionIndex: '0x0',
							removed: false
						}))
				};
			}
			case 'eth_getTransactionReceipt': {
				const e = this.executions.find((x) => x.tx === String(params[0]).toLowerCase());
				if (!e) return { result: null };
				return {
					result: {
						transactionHash: e.tx,
						transactionIndex: '0x0',
						blockHash: `0x${'ab'.repeat(32)}`,
						blockNumber: '0x100',
						from: OUTSIDE.address.toLowerCase(),
						to: e.safe,
						status: '0x1',
						gasUsed: '0x186a0',
						cumulativeGasUsed: '0x186a0',
						effectiveGasPrice: '0x3b9aca00',
						contractAddress: null,
						logsBloom: `0x${'0'.repeat(512)}`,
						type: '0x2',
						logs: [
							{
								address: e.safe,
								topics: [EXECUTION_SUCCESS, e.hash],
								data: `0x${word(0)}`,
								transactionHash: e.tx,
								blockNumber: '0x100',
								blockHash: `0x${'ab'.repeat(32)}`,
								logIndex: '0x0',
								transactionIndex: '0x0',
								removed: false
							}
						]
					}
				};
			}
			case 'eth_call':
				return { result: this.call(chainId, params[0] as { to?: string; data?: string }) };
			case 'eth_estimateGas':
				return { result: '0x30d40' };
			case 'eth_simulateV1':
				if (!this.simulates) return { error: { code: -32601, message: `${method} not here` } };
				return {
					result: [
						{
							number: '0x101',
							calls: [{ status: '0x1', returnData: '0x', gasUsed: '0x5208', logs: [] }]
						}
					]
				};
			default:
				return { error: { code: -32601, message: `${method} not here` } };
		}
	}

	private call(chainId: number, call: { to?: string; data?: string }): string {
		const data = (call.data ?? '0x').toLowerCase();
		const safe = this.safes.get(`${chainId}:${String(call.to).toLowerCase()}`);
		if (safe) {
			const addresses = (list: string[]) =>
				`0x${word(32)}${word(list.length)}${list.map((a) => word(BigInt(a))).join('')}`;
			switch (data.slice(0, 10)) {
				case '0xa0e67e2b': // getOwners()
					return addresses(safe.owners);
				case '0xe75235b8': // getThreshold()
					return `0x${word(safe.threshold)}`;
				case '0xaffed0e0': // nonce()
					return `0x${word(safe.nonce)}`;
				case '0xcc2f8452': // getModulesPaginated(address,uint256): none
					return `0x${word(64)}${word(1)}${word(0)}`;
				case '0x7d832974': // approvedHashes(address,bytes32)
					return `0x${word(0)}`;
				default:
					return '0x';
			}
		}
		// Multicall3's aggregate3: a native balance is this world's; every
		// other inner call fails, as a call to an address with no code would.
		if (isAggregate3(data))
			return answerAggregate3(data, (inner) => {
				if (inner.selector !== MULTICALL_SEL.getEthBalance) return undefined;
				const who = `0x${inner.data.slice(8 + 24, 8 + 64)}`;
				return { success: true, data: word(this.balance(chainId, who)) };
			});
		return '0x';
	}
}

/**
 * Point the profile's chain 100 at this world and settle its shown networks
 * to Gnosis alone, before the wallet reads them.
 */
export async function useWorldNetworks(page: Page, chainIds = [100]): Promise<void> {
	await page.evaluate(
		({ rows, shown }) =>
			new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					const kv = tx.objectStore('kv');
					kv.put(JSON.stringify(rows), 'vela.networkConfig');
					kv.put(JSON.stringify({ version: 1, shown }), 'vela.shownNetworks');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			}),
		{
			rows: chainIds.map((chainId) => ({ chainId, rpcURL: `${RPC_HOST}/${chainId}` })),
			shown: chainIds
		}
	);
}

/**
 * Enter the parallel space, its chains pointed at the world, and land on the
 * wallet. `keep` names the fixture accounts this "device" holds.
 */
export async function enterSpace(page: Page, keep?: string[]): Promise<void> {
	await page.addInitScript(() => localStorage.setItem('vela.intro.seen', String(Date.now())));
	await page.goto('/en/parallel');
	await useWorldNetworks(page);
	// The list renders once the core is aboard: the buttons are wired by then.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet/);
	if (keep) {
		await page.evaluate((wanted) => {
			const rows = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
				address: string;
			}[];
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify(
					rows.filter((r) => wanted.some((w) => w.toLowerCase() === r.address.toLowerCase()))
				)
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
		}, keep);
		await page.reload();
	}
}

/** Keep `ms` on this device as added (shown), and reload onto it. */
export async function holdMultisig(page: Page, ms: Multisig): Promise<void> {
	await page.evaluate(
		({ row, address }) => {
			localStorage.setItem('vela.multisigAccounts', JSON.stringify([row]));
			localStorage.setItem('vela.activeMultisig', address);
		},
		{ row: storedRow(ms), address: ms.address }
	);
	await page.reload();
}
