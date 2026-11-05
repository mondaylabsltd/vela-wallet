/**
 * The multisig shell answers every operation in the vocabulary, never
 * rejects, and drives the real core machines (wasm) end to end.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';
import { MultisigAccountsCore } from '../../../../../../rust/pkg-web/vela_core.js';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { MultisigOperation } from '../generated/MultisigOperation';
import type { MultisigShellResult } from '../generated/MultisigShellResult';
import type { MultisigAccountsView } from '../generated/MultisigAccountsView';
import type { MultisigAccountsEvent } from '../generated/MultisigAccountsEvent';
import {
	multisigShell,
	operationFailure,
	passkeyRequest,
	type MultisigEffect,
	type MultisigShellDeps
} from './handler';

function memoryStore(seed: Record<string, string> = {}) {
	const map = new Map(Object.entries(seed));
	return {
		map,
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k)
	};
}

function deps(over: Partial<MultisigShellDeps> = {}): MultisigShellDeps {
	const store = memoryStore();
	return {
		fetch: vi.fn(async () => new Response('{}', { status: 200 })) as unknown as typeof fetch,
		rpc: async () => ({ result: '0x' }),
		storage: () => store,
		now: () => 1_000,
		random: (n) => new Uint8Array(n).fill(0xab),
		passkey: async () => ({ type: 'passkey_cancelled' }),
		askRisk: async () => false,
		sendCalls: async () => ({ type: 'calls_cancelled' }),
		quoteCalls: async () => ({ type: 'quote_failed', detail: 'none' }),
		simulate: async () => null,
		recipientFacts: async () => ({ isContract: null, firstTime: null }),
		userOpStatus: async () => null,
		peer: {
			open: async () => ({ room: 'r1', invite: 'inv', pairing: 'rec' }),
			send: async () => {},
			receive: async () => ({ closed: 'closed' }),
			close: async () => {}
		},
		wallet: async () => '0xsigned',
		notice: () => {},
		...over
	};
}

const run = (d: MultisigShellDeps, operation: MultisigOperation) =>
	multisigShell(d).execute({ id: 1, operation }, new AbortController().signal);

describe('the multisig shell', () => {
	it('answers HTTP as it came: the status, Retry-After and the body text', async () => {
		const fetch = vi.fn(
			async () => new Response('{"wait":1}', { status: 429, headers: { 'retry-after': '3' } })
		);
		const answer = await run(deps({ fetch: fetch as unknown as typeof globalThis.fetch }), {
			type: 'http',
			method: 'POST',
			url: 'http://service.test/api/proposals',
			body: '{"a":1}',
			timeout_ms: 1000
		});
		expect(answer).toEqual({
			type: 'http_answered',
			status: 429,
			retry_after: '3',
			body: '{"wait":1}'
		});
		const init = fetch.mock.calls[0] as unknown as [string, RequestInit];
		expect(init[1].method).toBe('POST');
		expect(init[1].headers).toEqual({ 'content-type': 'application/json' });
	});

	it('says an HTTP request nothing answered, and one past its timeout, as unreachable', async () => {
		const refused = await run(
			deps({
				fetch: (async () => {
					throw new TypeError('Failed to fetch');
				}) as unknown as typeof fetch
			}),
			{ type: 'http', method: 'GET', url: 'http://x', body: null, timeout_ms: 1000 }
		);
		expect(refused.type).toBe('http_unreachable');
		const hung = await run(
			deps({
				fetch: ((_url: string, init: RequestInit) =>
					new Promise((_, reject) => {
						init.signal?.addEventListener('abort', () =>
							reject(new DOMException('aborted', 'AbortError'))
						);
					})) as unknown as typeof fetch
			}),
			{ type: 'http', method: 'GET', url: 'http://x', body: null, timeout_ms: 20 }
		);
		expect(hung.type).toBe('http_unreachable');
	});

	it('answers JSON-RPC with the result, the node’s error, or unanswered (bounded)', async () => {
		const op = (bound: number | null): MultisigOperation => ({
			type: 'rpc',
			chain_id: 100,
			method: 'eth_call',
			params: [],
			bound_ms: bound
		});
		expect(await run(deps({ rpc: async () => ({ result: '0x01' }) }), op(null))).toEqual({
			type: 'rpc_result',
			result: '0x01'
		});
		expect(
			await run(deps({ rpc: async () => ({ error: { code: 3, message: 'reverted' } }) }), op(null))
		).toEqual({ type: 'rpc_error', code: 3n, message: 'reverted' });
		// r8-4: a revert's payload reaches the core (the state probe reads it).
		expect(
			await run(
				deps({ rpc: async () => ({ error: { code: 3, message: 'reverted', data: '0x0001' } }) }),
				op(null)
			)
		).toEqual({ type: 'rpc_error', code: 3n, message: 'reverted', data: '0x0001' });
		const thrown = await run(
			deps({
				rpc: async () => {
					throw new Error('pool gave up');
				}
			}),
			op(null)
		);
		expect(thrown.type).toBe('rpc_unanswered');
		const late = await run(deps({ rpc: () => new Promise(() => {}) }), op(10));
		expect(late.type).toBe('rpc_unanswered');
	});

	it('reads and writes the store; no storage is said, never thrown', async () => {
		const store = memoryStore({ a: '1' });
		const d = deps({ storage: () => store });
		expect(await run(d, { type: 'load_store', keys: ['a', 'b'] })).toEqual({
			type: 'store_loaded',
			entries: { a: '1', b: null }
		});
		await run(d, { type: 'write_store', key: 'b', value: '2' });
		await run(d, { type: 'write_store', key: 'a', value: null });
		expect([...store.map]).toEqual([['b', '2']]);
		expect((await run(deps({ storage: () => null }), { type: 'load_store', keys: [] })).type).toBe(
			'store_failed'
		);
	});

	it('gives the clock, random bytes as hex, and waits', async () => {
		const d = deps();
		expect(await run(d, { type: 'now' })).toEqual({ type: 'now', now_ms: 1_000 });
		expect(await run(d, { type: 'random', bytes: 2 })).toEqual({ type: 'random', hex: '0xabab' });
		expect(await run(d, { type: 'wait', ms: 1 })).toEqual({ type: 'waited' });
	});

	it('hands the product pipelines over and answers in their words', async () => {
		const notice = vi.fn();
		const d = deps({
			askRisk: async () => true,
			simulate: async () => '{"result":[]}',
			recipientFacts: async () => ({ isContract: false, firstTime: true }),
			userOpStatus: async () => ({ status: 'included', txHash: '0xt' }),
			notice
		});
		expect(await run(d, { type: 'ask_risk', chain_id: 1, safe: '0xs', findings: [] })).toEqual({
			type: 'risk_answered',
			signs: true
		});
		expect(await run(d, { type: 'simulate', chain_id: 1, from: '0xs', calls: [] })).toEqual({
			type: 'simulated',
			reply: '{"result":[]}'
		});
		expect(await run(d, { type: 'recipient_risk', chain_id: 1, address: '0xa' })).toEqual({
			type: 'recipient_facts',
			is_contract: false,
			first_time: true
		});
		expect(await run(d, { type: 'user_op_status', chain_id: 1, user_op_hash: '0xh' })).toEqual({
			type: 'user_op_status',
			status: 'included',
			tx_hash: '0xt'
		});
		const n = { kind: 'mailbox_took', safe: '0xs' } as const;
		expect(await run(d, { type: 'notice', notice: n })).toEqual({ type: 'ack' });
		expect(notice).toHaveBeenCalledWith(n);
	});

	it('runs PeerConnect rooms through the transport', async () => {
		const d = deps();
		expect(await run(d, { type: 'peer_open', relay: null, invite: null, pairing: null })).toEqual({
			type: 'peer_opened',
			room: 'r1',
			invite: 'inv',
			pairing: 'rec'
		});
		expect(await run(d, { type: 'peer_receive', room: 'r1' })).toEqual({
			type: 'peer_closed',
			reason: 'closed'
		});
	});

	it('says a wallet’s refusal with its EIP-1193 code', async () => {
		const d = deps({
			wallet: async () => {
				throw { code: 4001, message: 'User rejected' };
			}
		});
		expect(await run(d, { type: 'wallet_request', method: 'eth_sign', params: [] })).toEqual({
			type: 'wallet_refused',
			code: 4001n,
			message: 'User rejected'
		});
	});

	it('a node’s error code and a wallet’s 4001 reach the machine as the numbers they are', async () => {
		// The generated types spell these `i64` codes `bigint`; the wire is JSON:
		// what the core is handed back must be the number, never a string or a throw.
		const resolved: string[] = [];
		const operations: MultisigOperation[] = [
			{ type: 'rpc', chain_id: 100, method: 'eth_call', params: [], bound_ms: null },
			{ type: 'wallet_request', method: 'eth_sign', params: [] }
		];
		let next = 0;
		const core = {
			view: () => '{}',
			dispatch: () =>
				JSON.stringify({
					view: {},
					effects: operations.map((operation, i) => ({ id: i + 1, operation }))
				}),
			resolve_effect: (_id: bigint, result: string) => {
				resolved.push(result);
				next += 1;
				return JSON.stringify({ view: {}, effects: [] });
			},
			free: () => {}
		};
		const shell = multisigShell(
			deps({
				rpc: async () => ({ error: { code: -32000, message: 'execution reverted' } }),
				wallet: async () => {
					throw { code: 4001, message: 'User rejected' };
				}
			})
		);
		const loop = createJsonWasmShell<unknown, unknown, MultisigEffect, MultisigShellResult>(core, {
			onView: () => {},
			execute: shell.execute,
			toFailure: shell.toFailure,
			onError: (error) => {
				throw error;
			}
		});
		loop.dispatch({});
		await vi.waitFor(() => expect(next).toBe(2));
		// Both answered (in whichever order their pipelines came back).
		expect(resolved.map((r) => JSON.parse(r))).toEqual(
			expect.arrayContaining([
				{ type: 'rpc_error', code: -32000, message: 'execution reverted' },
				{ type: 'wallet_refused', code: 4001, message: 'User rejected' }
			])
		);
		loop.dispose();
	});

	it('answers every operation’s own failure when its pipeline threw', () => {
		const cases: [MultisigOperation, MultisigShellResult['type']][] = [
			[{ type: 'http', method: 'GET', url: 'x', body: null, timeout_ms: 1 }, 'http_unreachable'],
			[{ type: 'rpc', chain_id: 1, method: 'm', params: [], bound_ms: null }, 'rpc_unanswered'],
			[{ type: 'ask_risk', chain_id: 1, safe: 's', findings: [] }, 'risk_answered'],
			[
				{ type: 'send_calls', chain_id: 1, from: 'f', calls: [], fee: null, purpose: null },
				'calls_failed'
			],
			[{ type: 'quote_calls', chain_id: 1, from: 'f', calls: [] }, 'quote_failed'],
			[{ type: 'peer_receive', room: 'r' }, 'peer_closed'],
			[{ type: 'wallet_request', method: 'm', params: [] }, 'wallet_refused']
		];
		for (const [op, type] of cases) expect(operationFailure(op, new Error('x')).type).toBe(type);
		const send = operationFailure(
			{ type: 'send_calls', chain_id: 1, from: 'f', calls: [], fee: null, purpose: null },
			new Error('x')
		);
		expect(send).toMatchObject({ maybe_sent: false });
		expect(
			operationFailure({ type: 'ask_risk', chain_id: 1, safe: 's', findings: [] }, null)
		).toEqual({ type: 'risk_answered', signs: false });
	});

	it('names what a passkey is for, as the page and the extension are shown', () => {
		expect(
			passkeyRequest({
				kind: 'approve',
				multisig: '0xm',
				safe_tx_hash: '0xh',
				name: null,
				tx: {},
				anyway: false,
				owners: [],
				threshold: 0
			})
		).toEqual({ method: 'vela_multisigApprove', params: { safeTxHash: '0xh' } });
		// §16: the owner proof's params are the core's, handed over verbatim.
		const params = [
			{ multisig: '0xm', member: '0xo', genesis: { factory: '0xf' }, chainIds: [8453] }
		];
		expect(passkeyRequest({ kind: 'record', multisig: '0xm', params })).toEqual({
			method: 'vela_multisigCreate',
			params
		});
	});

	it('drives the core’s accounts machine: the stored multisigs read through the store', async () => {
		const stored = memoryStore({ 'vela.multisigAccounts': '[]' });
		const shell = multisigShell(deps({ storage: () => stored }));
		let view: MultisigAccountsView | null = null;
		const loop = createJsonWasmShell<
			MultisigAccountsView,
			MultisigAccountsEvent,
			MultisigEffect,
			MultisigShellResult
		>(new MultisigAccountsCore(), {
			onView: (v) => (view = v),
			execute: shell.execute,
			toFailure: shell.toFailure,
			onError: (e) => {
				throw e;
			}
		});
		loop.start({
			type: 'start',
			service_url: 'http://service.test',
			accounts: [],
			networks: [100]
		});
		await vi.waitFor(() => expect(view?.loaded).toBe(true));
		expect(view!.accounts).toEqual([]);
		loop.dispose();
	});
});
