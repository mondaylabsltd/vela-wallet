/**
 * Both sides of a live session, each the core's own machine run by this
 * shell — Vela's session (`MultisigOutsideSessionCore`) and the connect page
 * (`MultisigOutsidePageCore`) — over an in-memory PeerConnect: Vela asks,
 * the page shows the request, the owner taps, the injected wallet signs, and
 * Vela verifies the answer and ends the ask with the approval. What the
 * onchain suite does in two browsers, here in one, with no relay. §12: the
 * page's chain holds the multisig (the owner its member), so what Vela asks
 * is reviewed before the wallet is asked — never handed over unread.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { signEthereum } from '$lib/testing/secp256k1';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { loadCore } from '$lib/core/client';
import { safeTxHash, type SafeTx } from '../core/kernel';
import type { OutsidePageEvent } from '../generated/OutsidePageEvent';
import type { OutsidePageView } from '../generated/OutsidePageView';
import type { OutsideSessionEvent } from '../generated/OutsideSessionEvent';
import type { OutsideSessionView } from '../generated/OutsideSessionView';
import type { PeerTransport } from './handler';
import { MultisigMachine } from './machine.svelte';
import { pageDeps } from './outside-page.svelte';
import { membersRpc, type MembersWorld } from './members.testing';

/** Hardhat's second dev account. */
const KEY = '59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d';
const OWNER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const TX: SafeTx = {
	to: '0x000000000000000000000000000000000000dEaD',
	value: '0x2386f26fc10000',
	data: '0x',
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: '0x0000000000000000000000000000000000000000',
	refundReceiver: '0x0000000000000000000000000000000000000000',
	nonce: '0x3'
};

/** One room between two sides: what one sends, the other receives. */
function wire() {
	const inbox: Record<string, unknown[]> = { vela: [], page: [] };
	const waiting: Record<string, ((m: { message: unknown }) => void) | null> = {
		vela: null,
		page: null
	};
	const tells: Record<string, (event: { type: string; room: string; state: string }) => void> = {};
	function side(me: 'vela' | 'page'): PeerTransport {
		const other = me === 'vela' ? 'page' : 'vela';
		return {
			open: async ({ invite }) => {
				const room = `${me}-room`;
				// Both in the room: each side is told it is connected.
				if (me === 'page') {
					setTimeout(() => {
						tells.vela?.({ type: 'peer_state', room: 'vela-room', state: 'connected' });
						tells.page?.({ type: 'peer_state', room: 'page-room', state: 'connected' });
					});
				}
				return {
					room,
					invite: invite === null ? 'wss://relay#invite' : null,
					pairing: `rec-${me}`
				};
			},
			send: async (_room, message) => {
				const copy = JSON.parse(JSON.stringify(message));
				const w = waiting[other];
				if (w) {
					waiting[other] = null;
					w({ message: copy });
				} else inbox[other].push(copy);
			},
			receive: (_room, signal) => {
				const next = inbox[me].shift();
				if (next !== undefined) return Promise.resolve({ message: next });
				return new Promise((resolve) => {
					waiting[me] = resolve;
					signal.addEventListener('abort', () => resolve({ closed: 'left' }), { once: true });
				});
			},
			close: async () => {}
		};
	}
	return { side, tells };
}

/** The owner's wallet: signs a SafeTx's typed data as the owner (its EIP-712 digest). */
function wallet(digest: string) {
	return {
		request: async ({ method }: { method: string }) => {
			switch (method) {
				case 'eth_accounts':
				case 'eth_requestAccounts':
					return [OWNER];
				case 'eth_chainId':
					return '0x64';
				case 'eth_signTypedData_v4':
					return signEthereum(digest, KEY);
				default:
					throw { code: 4200, message: `unsupported ${method}` };
			}
		}
	};
}

/**
 * A node as a public one answers the review's state checks: the trace
 * refused ("method not found"), the multisig's probe reverted — answers, not
 * silence (r16-3: a check that comes back silent is asked again before the
 * review may land "unchecked").
 */
function nodeRefusingChecks(rpc: ReturnType<typeof membersRpc>): ReturnType<typeof membersRpc> {
	return async (chainId, method, params) => {
		if (method === 'debug_traceCall')
			return { error: { code: -32601, message: 'the method debug_traceCall does not exist' } };
		try {
			return await rpc(chainId, method, params);
		} catch (error) {
			const [call] = params as [{ to?: string }];
			if (method === 'eth_call' && call?.to?.toLowerCase() === SAFE.toLowerCase())
				return { error: { code: 3, message: 'execution reverted' } };
			throw error;
		}
	};
}

beforeAll(async () => {
	await loadCore();
});

describe('a live session, both sides the core', () => {
	it('Vela asks, the page shows it, the owner taps, Vela verifies and has the approval', async () => {
		localStorage.clear();
		const hash = safeTxHash(100, SAFE, TX);
		const { side, tells } = wire();
		const offline = {
			fetch: async () => new Response('{}', { status: 404 }),
			rpc: async () => {
				throw new Error('no chain here');
			}
		};
		const vela = new MultisigMachine<OutsideSessionView, OutsideSessionEvent>(
			new wasm.MultisigOutsideSessionCore(),
			{
				...pageDeps({ peer: side('vela'), wallet: () => null, askRisk: async () => false }),
				...offline
			},
			'outside session'
		);
		const owner = wallet(hash);
		// The page's chain: the multisig deployed there, the owner its member.
		const chain: MembersWorld = {
			safe: SAFE,
			chains: { 100: { deployed: true, owners: [OWNER], threshold: 1, nonce: 3 } },
			signers: {},
			asked: []
		};
		const page = new MultisigMachine<OutsidePageView, OutsidePageEvent>(
			new wasm.MultisigOutsidePageCore(),
			{
				...pageDeps({ peer: side('page'), wallet: () => owner, askRisk: async () => false }),
				...offline,
				rpc: nodeRefusingChecks(membersRpc(chain)),
				// r16-3: a node that refuses the simulation — an answer, so the
				// review's checks are final at once (a silent one is asked again).
				simulate: async () =>
					JSON.stringify({ error: { code: -32601, message: 'eth_simulateV1 does not exist' } })
			},
			'connect'
		);
		tells.vela = (e) => vela.dispatch(e as OutsideSessionEvent);
		tells.page = (e) => page.dispatch(e as OutsidePageEvent);

		vela.dispatch({
			type: 'start',
			request: { chainId: 100, safe: SAFE, tx: TX, safeTxHash: hash, owners: [OWNER], send: null },
			service_url: 'http://127.0.0.1:1',
			relay: null,
			origin: 'https://vela.test'
		});
		await vi.waitFor(() => expect(vela.view.invite).toBe('wss://relay#invite'));

		page.dispatch({ type: 'start', service_url: 'http://127.0.0.1:1', wallet_name: 'Test Wallet' });
		page.dispatch({ type: 'wallet', accounts: [OWNER], chain_id: 100 });
		page.dispatch({ type: 'join', invite: vela.view.invite!, members: [OWNER] });
		await vi.waitFor(() => expect(page.view.connectPrompt).not.toBeNull());
		page.dispatch({
			type: 'answer_connect',
			session: page.view.connectPrompt!.session,
			approve: true
		});
		await vi.waitFor(() => expect(page.view.request).not.toBeNull(), { timeout: 5_000 });
		expect(vela.view.phase).toBe('confirming');
		// §12: the request's review is read before the owner can hand it over.
		await vi.waitFor(() => expect(page.view.requestReview?.reviewState.kind).toBe('ready'), {
			timeout: 10_000
		});
		const asked = page.view.request!;
		page.dispatch({ type: 'answer_request', session: asked.session, id: asked.id, go: true });
		await vi.waitFor(() => expect(vela.view.outcome?.kind).toBe('approved'), { timeout: 5_000 });
		expect(page.view.activity[0].outcome).toBe('done');
		expect(
			(vela.view.outcome as { approval: { owner: string } }).approval.owner.toLowerCase()
		).toBe(OWNER);
		vela.dispose();
		page.dispose();
	});
});
