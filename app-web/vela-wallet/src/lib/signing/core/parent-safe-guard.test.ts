/**
 * §31 — a site never makes this account act for a Safe it owns, on the web.
 *
 * The rule is the core's (`vela_core::app::parent_safe_guard`); what the web
 * owes it is pinned here, through the shipped wasm and the real executor:
 *
 * - the vector (`tests/vectors/rules/parent-safe-guard.json`) replays through
 *   the web's own door to the rule (`parentSafeCheck`) case for case;
 * - the executor answers the core's `read_safe_owner` with the node's answer
 *   as it came — `returned` (its result verbatim), `rpc_error`, or
 *   `unreachable` (a throw, the pool giving up, no answer within one read's
 *   budget, a reply with no result) — and decides nothing from it;
 * - the device's multisig list (`multisigs_changed`) makes the machine refuse
 *   a request for one of them at once: the sheet opens refused, naming the
 *   Safe, in the catalog's words; nothing is signed; closing it answers the
 *   site -32603 `parent_safe_blocked`;
 * - a Safe not on the list is read first (an `eth_call` of `isOwner`): read
 *   `true` it is refused `owner`, unreadable it is refused `unchecked`, and
 *   read `false` it is no refusal.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';

const submit = vi.hoisted(() => ({ calls: 0 }));
vi.mock('$lib/services/dapp-submit', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/services/dapp-submit')>();
	return {
		...actual,
		handleDAppRequest: async () => {
			submit.calls += 1;
			throw new Error('nothing may be signed here');
		}
	};
});

import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import type { SignView } from '$lib/core/generated/SignView';
import type { SignShellResult } from '$lib/core/generated/SignShellResult';
import { signErrorWords } from '$lib/core/kernels';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { PoolFailedError } from '$lib/services/rpc-pool';
import { fill } from '$lib/wallet/messages';
import { buildSigningModel, selfCallBlockedText, signingCloseEvent } from '../live';
import { INITIAL_CLEAR_VIEW, INITIAL_GUARD_VIEW } from './sheet.svelte';
import { IDLE_FEE_VIEW } from '$lib/flows/core/fee-quote.svelte';
import { createSignExecutor, readSafeOwner, type SafeOwnerCall } from './sign-executor';
import { createSignRequestSession } from './sign-session';
import type { SignEffect, SignShellPorts } from './sign-types';

interface VectorCase {
	name: string;
	input: {
		method: string;
		params: unknown[];
		signer: string;
		chain_id: number;
		known: string[];
		answers: unknown[];
		at_submit: boolean;
	};
	expect: unknown;
}

const VECTOR = JSON.parse(
	readFileSync('../../rust/crates/vela-core/tests/vectors/rules/parent-safe-guard.json', 'utf8')
) as {
	signer: string;
	parent: string;
	unknown: string;
	chainId: number;
	isOwnerSelector: string;
	blockedKey: string;
	uncheckedKey: string;
	cases: VectorCase[];
};

/**
 * r18-23: the Safe as the sentence names it — EIP-55, shortened (the
 * vector's parent `0x5afe…5afe`, checksummed): never the lowercase the
 * request carried.
 */
const PARENT_SHORT = '0x5afe5a…fe5AfE';
const UNKNOWN_SHORT = '0x777777…777777';

const CASE_A = VECTOR.cases.find(
	(c) => c.name === 'a: execTransaction(swapOwner) on a multisig on the list, v=1'
)!;
/** ABI `true` / `false`, as a node answers `isOwner`. */
const TRUE = '0x' + '0'.repeat(63) + '1';
const FALSE = '0x' + '0'.repeat(64);

describe('the vector, through the web’s door to the rule (parentSafeCheck)', () => {
	it('every case gives the core’s verdict', () => {
		expect(VECTOR.cases.length).toBeGreaterThanOrEqual(39);
		for (const c of VECTOR.cases) {
			const got = JSON.parse(wasm.parentSafeCheck(JSON.stringify(c.input))) as unknown;
			expect(got, c.name).toEqual(c.expect);
		}
	});
});

describe('read_safe_owner — the node’s answer, as it came', () => {
	const SAFE = VECTOR.unknown;
	const DATA = VECTOR.isOwnerSelector + VECTOR.signer.slice(2).padStart(64, '0');

	it('asks the pool for eth_call [{to, data}, latest] on the named chain', async () => {
		const asked: unknown[] = [];
		const call: SafeOwnerCall = async (method, params, chainId) => {
			asked.push({ method, params, chainId });
			return { result: TRUE };
		};
		expect(await readSafeOwner(8453, SAFE, DATA, call, 1000)).toEqual({
			type: 'returned',
			data: TRUE
		});
		expect(asked).toEqual([
			{ method: 'eth_call', params: [{ to: SAFE, data: DATA }, 'latest'], chainId: 8453 }
		]);
	});

	it('returned: the result string verbatim — even an empty one, or junk', async () => {
		for (const data of ['0x', FALSE, '0xnot-a-bool']) {
			expect(await readSafeOwner(1, SAFE, DATA, async () => ({ result: data }), 1000)).toEqual({
				type: 'returned',
				data
			});
		}
	});

	it('rpc_error: the node’s JSON-RPC error, its code an integer or null', async () => {
		expect(
			await readSafeOwner(
				1,
				SAFE,
				DATA,
				async () => ({ error: { code: 3, message: 'execution reverted' } }),
				1000
			)
		).toEqual({ type: 'rpc_error', code: 3, message: 'execution reverted' });
		expect(
			await readSafeOwner(1, SAFE, DATA, async () => ({ error: { code: 'x', message: 42 } }), 1000)
		).toEqual({ type: 'rpc_error', code: null, message: '' });
	});

	it('unreachable: a throw, the pool giving up, no result, or no answer in time', async () => {
		const thrown = await readSafeOwner(
			1,
			SAFE,
			DATA,
			async () => {
				throw new TypeError('Failed to fetch');
			},
			1000
		);
		expect(thrown).toEqual({ type: 'unreachable', detail: 'Failed to fetch' });
		const pool = await readSafeOwner(
			1,
			SAFE,
			DATA,
			async () => {
				throw new PoolFailedError('All RPC endpoints failed for chain 1', {
					maybeDelivered: false,
					rateLimited: true
				});
			},
			1000
		);
		expect(pool).toEqual({
			type: 'unreachable',
			detail: 'All RPC endpoints failed for chain 1'
		});
		expect(await readSafeOwner(1, SAFE, DATA, async () => ({ result: null }), 1000)).toEqual({
			type: 'unreachable',
			detail: 'the node answered no result'
		});
		// Never hangs: a node that never answers is "no answer" past the budget.
		const started = Date.now();
		const late = await readSafeOwner(1, SAFE, DATA, () => new Promise(() => {}), 30);
		expect(late).toEqual({ type: 'unreachable', detail: 'no answer in 30 ms' });
		expect(Date.now() - started).toBeLessThan(2000);
	});

	it('the executor echoes id, chain and Safe exactly, and a failure is unreachable', async () => {
		const executor = createSignExecutor(
			ports([]),
			async () => {},
			async () => ({ error: { code: -32000, message: 'header not found' } })
		);
		const effect = {
			operation: {
				type: 'read_safe_owner',
				id: 'rid-31',
				chain_id: 8453,
				safe: '0x7777777777777777777777777777777777777777',
				account: VECTOR.signer,
				data: DATA
			}
		} as unknown as SignEffect;
		expect(await executor.execute(effect)).toEqual({
			type: 'safe_owner_read',
			id: 'rid-31',
			chain_id: 8453,
			safe: '0x7777777777777777777777777777777777777777',
			answer: { type: 'rpc_error', code: -32000, message: 'header not found' }
		} satisfies SignShellResult);
		expect(executor.toFailure(effect, new Error('boom'))).toEqual({
			type: 'safe_owner_read',
			id: 'rid-31',
			chain_id: 8453,
			safe: '0x7777777777777777777777777777777777777777',
			answer: { type: 'unreachable', detail: 'boom' }
		});
	});
});

type Answer = {
	id: string;
	result?: unknown;
	error?: { code: number; message: string; kind?: string };
};

function ports(answers: Answer[]): SignShellPorts {
	return {
		transportFor: () => ({
			sendResponse: (id, result, error) => answers.push({ id, result, error })
		}),
		opSubmitted: () => {},
		opSigned: () => {},
		ceremony: () => {},
		askerLive: async () => true,
		approvedAtMs: () => null,
		requestOrigin: () => 'https://evil.example',
		assetSim: () => null,
		switchActiveAccount: async () => {},
		recordsWritten: () => {}
	};
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const APPROVE = {
	type: 'approve_tapped',
	opts: {
		max_fee_per_gas: null,
		bundler_cost_wei: null,
		gas_fee_token: null,
		quoted_fee: null,
		fee_collector: null,
		params_override_json: null,
		intent: null,
		unlimited_approved: false,
		unlimited_approved_legs: [] as number[]
	}
} as const;

/**
 * The real core and the real executor: this device's multisigs, then a
 * site's request `params` from the vector's signer on the vector's chain.
 */
async function arrive(
	params: unknown[],
	known: string[],
	ownerCall?: SafeOwnerCall,
	method = 'eth_sendTransaction'
) {
	const answers: Answer[] = [];
	const faults: unknown[] = [];
	const reads: unknown[] = [];
	let view: SignView | null = null;
	const session = createSignRequestSession({
		ports: ports(answers),
		onView: (next) => (view = next),
		onError: (error) => faults.push(error),
		ownerCall: async (method, p, chainId) => {
			reads.push({ method, params: p, chainId });
			if (!ownerCall) throw new Error('no read was expected');
			return ownerCall(method, p, chainId);
		}
	});
	session.start({ type: 'networks_changed', chain_ids: [1, VECTOR.chainId] });
	session.dispatch({
		type: 'accounts_changed',
		accounts: [{ address: VECTOR.signer, credential_id: 'cred-1' }],
		active_index: 0
	});
	session.dispatch({ type: 'multisigs_changed', addresses: known });
	session.dispatch({
		type: 'request_arrived',
		id: 'rid-31',
		method,
		params_json: JSON.stringify(params),
		origin: 'https://evil.example',
		transport_id: 'ext-31',
		dedicated_transport: true,
		per_request_chain: VECTOR.chainId,
		dapp: null,
		granted_address: VECTOR.signer,
		requested_address: null,
		request_ts_ms: null,
		now_ms: Date.now(),
		first_party: false
	});
	await settle();
	return {
		answers,
		faults,
		reads,
		view: () => view as SignView,
		/** The person tries to confirm anyway (a stale tap, a scripted click). */
		approve: async () => {
			session.dispatch(APPROVE);
			await settle();
		},
		/**
		 * The sheet's one way out: a refused sheet has no status, so its ✕ is
		 * `reject_tapped` (`signingCloseEvent`), as `SigningHost` sends it.
		 */
		close: async () => {
			session.dispatch({ type: signingCloseEvent(null) ?? 'reject_tapped' });
			await settle();
			session.dispose();
		}
	};
}

/** A vector case's own request, by name. */
function caseParams(name: string): unknown[] {
	const found = VECTOR.cases.find((c) => c.name === name);
	if (!found) throw new Error(`no vector case ${name}`);
	return found.input.params;
}

describe('a multisig on this device’s list (multisigs_changed)', () => {
	it('case (a) is refused at once — named, unsignable, answered -32603 parent_safe_blocked', async () => {
		submit.calls = 0;
		const run = await arrive(CASE_A.input.params, [VECTOR.parent]);
		const view = run.view();
		expect(view.surface).not.toBe('hidden');
		expect(view.blocked).toMatchObject({
			function: 'execTransaction',
			parent: VECTOR.parent,
			reason: 'member'
		});
		expect(view.error?.kind).toBe('parent_safe_blocked');
		expect(view.confirm_gate_open).toBe(false);
		expect(view.confirm_block).toBe('refused');
		// No read for a Safe already known, and nothing answered yet: the sheet
		// says why first.
		expect(run.reads).toEqual([]);
		expect(run.answers).toEqual([]);

		// The sheet's words: the catalog's, with the function and the Safe.
		for (const locale of ['en', 'zh'] as const) {
			const m = resolveSigningMessages(locale);
			expect(selfCallBlockedText(view.blocked!, m), locale).toBe(
				fill(m.parentSafeBlocked, {
					function: 'execTransaction',
					safe: PARENT_SHORT
				})
			);
			expect(m.parentSafeBlocked, locale).not.toContain('componentsUi.');
		}

		// Confirming anyway signs nothing.
		await run.approve();
		expect(submit.calls).toBe(0);

		await run.close();
		expect(submit.calls).toBe(0);
		expect(run.answers).toEqual([
			{
				id: 'rid-31',
				result: undefined,
				error: expect.objectContaining({
					code: -32603,
					kind: 'parent_safe_blocked',
					// The core's detail names the call and the Safe.
					message: signErrorWords('parent_safe_blocked', `execTransaction on ${VECTOR.parent}`)
				})
			}
		]);
		expect(run.faults).toEqual([]);
	});

	it('the same request with nothing on the list is read on chain first', async () => {
		const run = await arrive(CASE_A.input.params, [], async () => ({ result: TRUE }));
		await vi.waitFor(() => expect(run.view().blocked).not.toBeNull());
		expect(run.reads).toHaveLength(1);
		expect(run.view().blocked).toMatchObject({ parent: VECTOR.parent, reason: 'owner' });
		await run.close();
	});
});

describe('a Safe not on the list: read (isOwner) before it can be signed', () => {
	const isOwnerData = VECTOR.isOwnerSelector + VECTOR.signer.slice(2).padStart(64, '0');

	it('isOwner true: refused `owner`, after one eth_call through the executor', async () => {
		submit.calls = 0;
		const run = await arrive(
			caseParams('unknown Safe, isOwner true: refused'),
			[VECTOR.parent],
			async () => ({
				result: TRUE
			})
		);
		await vi.waitFor(() => expect(run.view().blocked).not.toBeNull());
		expect(run.reads).toEqual([
			{
				method: 'eth_call',
				params: [{ to: VECTOR.unknown, data: isOwnerData }, 'latest'],
				chainId: VECTOR.chainId
			}
		]);
		const view = run.view();
		expect(view.blocked).toMatchObject({ parent: VECTOR.unknown, reason: 'owner' });
		expect(view.confirm_gate_open).toBe(false);
		await run.approve();
		await run.close();
		expect(submit.calls).toBe(0);
		expect(run.answers).toEqual([
			{
				id: 'rid-31',
				result: undefined,
				error: expect.objectContaining({ code: -32603, kind: 'parent_safe_blocked' })
			}
		]);
	});

	it('unreachable: refused `unchecked`, in the unchecked words', async () => {
		submit.calls = 0;
		const run = await arrive(caseParams('unknown contract: read first'), [], async () => {
			throw new PoolFailedError(`All RPC endpoints failed for chain ${VECTOR.chainId}`, {
				maybeDelivered: false,
				rateLimited: false
			});
		});
		await vi.waitFor(() => expect(run.view().blocked).not.toBeNull());
		const view = run.view();
		expect(view.blocked).toMatchObject({ parent: VECTOR.unknown, reason: 'unchecked' });
		const m = resolveSigningMessages('en');
		expect(selfCallBlockedText(view.blocked!, m)).toBe(
			fill(m.parentSafeUnchecked, {
				function: 'approveHash',
				safe: UNKNOWN_SHORT
			})
		);
		await run.approve();
		await run.close();
		expect(submit.calls).toBe(0);
		expect(run.answers[0]?.error).toMatchObject({ code: -32603, kind: 'parent_safe_blocked' });
	});

	it('the confirm stays shut while the read is out', async () => {
		let answer: (value: { result: string }) => void = () => {};
		const run = await arrive(
			caseParams('unknown contract: read first'),
			[],
			() => new Promise((resolve) => (answer = resolve))
		);
		await vi.waitFor(() => expect(run.reads).toHaveLength(1));
		expect(run.view().blocked).toBeNull();
		expect(run.view().confirm_gate_open).toBe(false);
		answer({ result: FALSE });
		await vi.waitFor(() => expect(run.view().confirm_block).not.toBe('reading'));
		// Read false: not this account's Safe — no refusal.
		expect(run.view().blocked).toBeNull();
		await run.close();
	});

	/**
	 * r18-6 (Safari, the shared sheet): a SafeMessage to a Safe not on the
	 * list drew a shut red "Sign anyway" for the read's ~200 ms before the
	 * parent refusal — the label followed the clear-signing view alone
	 * (nobody can read a SafeMessage: `sign_anyway`), while the core's
	 * `confirm_block` said `reading`. While the read is out the sheet says
	 * "Reading the request…" over a shut, plain "Sign": no danger, never
	 * "anyway" — the core may yet refuse it with no way to sign.
	 */
	it('while the read is out the confirm is a shut "Sign" — never "anyway", never red', async () => {
		let answer: (value: { result: string }) => void = () => {};
		const run = await arrive(
			caseParams('unknown Safe, SafeMessage, isOwner true: refused'),
			[],
			() => new Promise((resolve) => (answer = resolve)),
			'eth_signTypedData_v4'
		);
		await vi.waitFor(() => expect(run.reads).toHaveLength(1));
		expect(run.view().confirm_block).toBe('reading');
		const m = resolveSigningMessages('en');
		const sheet = () =>
			buildSigningModel({
				sign: run.view(),
				// What the clear-signing machine says of a SafeMessage: nobody
				// can read it — its own confirm would be "Sign anyway".
				clear: { ...INITIAL_CLEAR_VIEW, resolved: true, sign_anyway: true },
				guard: INITIAL_GUARD_VIEW,
				fee: IDLE_FEE_VIEW,
				currency: { code: 'USD', rate: 1, committed: true, pending: null },
				m,
				identity: { name: 'Mine', address: VECTOR.signer, identiconSvg: '<svg/>' },
				identicon: () => '<svg/>'
			});
		const reading = sheet();
		expect(reading?.confirm.enabled).toBe(false);
		expect(reading?.confirm.action).toBe(m.signLabel);
		expect(reading?.confirm.action).toBe('Sign');
		expect(reading?.confirm.action).not.toMatch(/anyway/i);
		expect(reading?.confirm.danger).toBeUndefined();
		expect(reading?.confirm.note).toBe('Reading the request…');
		// The read says owner: refused in the parent's words, no confirm to press.
		answer({ result: TRUE });
		await vi.waitFor(() => expect(run.view().blocked).not.toBeNull());
		const refused = sheet();
		expect(refused?.dismissOnly).toBe(m.close);
		expect(run.view().confirm_block).toBe('refused');
		await run.close();
	});
});
