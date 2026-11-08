/**
 * The app-wide host of the proposals machine (the real wasm, a simulated
 * shell):
 *
 * - what waits on a multisig NOT shown is the machine's (`presence`): before
 *   it reads anything, the waiting list an earlier page kept — re-read here
 *   against the members kept with it (Rust:
 *   `the_others_start_from_the_kept_list_then_are_read`);
 * - the multisig shown is the list it follows (`follow`), never one of the
 *   others its round reads;
 * - a notice it sends reaches where it goes: a multisig's record to the store,
 *   executed rows to the activity.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { secp256k1 } from '@noble/curves/secp256k1';
import { cancelTx, safeTxHash } from '../core/kernel';
import type { MultisigShellDeps } from './handler';

const SERVICE = 'http://service.test';
const SHOWN = '0x00000000000000000000000000000000000000a1';
const OTHER = '0x00000000000000000000000000000000000000b2';
const ME = '0x00000000000000000000000000000000000000aa';
/**
 * §37 (review 1-13): the other member — Hardhat's second dev account — whose
 * approval makes a kept proposal a member's. One no member approved is left
 * out of every count, as the mailbox takes a post from anyone.
 */
const PEER_KEY = '59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d';
const PEER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';
const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
const bytes = (h: string) =>
	Uint8Array.from((h.replace(/^0x/, '').match(/../g) ?? []).map((x) => parseInt(x, 16)));
/** PEER's approval of `hash`: its ECDSA signature, `r ‖ s ‖ v`. */
function peerApproval(hash: string) {
	const sig = secp256k1.sign(bytes(hash), bytes(PEER_KEY));
	return {
		owner: PEER,
		signature: `0x${hex(sig.toCompactRawBytes())}${(27 + sig.recovery).toString(16)}`,
		createdMs: 1
	};
}

const page = new EventTarget();
const doc = Object.assign(new EventTarget(), { visibilityState: 'visible' });
vi.stubGlobal('window', page);
vi.stubGlobal('document', doc);

const seams = vi.hoisted(() => ({
	used: [] as [string, number][],
	findings: [] as [string, number][]
}));
vi.mock('../core/store.svelte', () => ({
	multisig: {
		accounts: [],
		useChain: (address: string, chainId: number) => seams.used.push([address, chainId]),
		setFindingsOn: (address: string, chainId: number) => seams.findings.push([address, chainId]),
		setRecordedName: () => {},
		nameMembers: () => {},
		// The one host hands it the accounts machine and its records.
		attach: () => {},
		send: () => {}
	}
}));
vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({ contactsBook: { view: null } }));
vi.mock('$lib/session/core/session.svelte', () => ({ session: { view: { accounts: [] } } }));

const { proposalsHost } = await import('./proposals-host.svelte');
const { machines } = await import('./machines.svelte');

function memoryStore(seed: Record<string, string> = {}) {
	const map = new Map(Object.entries(seed));
	return {
		map,
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k)
	};
}

function deps(store: ReturnType<typeof memoryStore>, http: string[]): MultisigShellDeps {
	return {
		fetch: vi.fn(async (url: string | URL | Request) => {
			http.push(String(url));
			return new Response('{}', { status: 503 });
		}) as unknown as typeof fetch,
		rpc: async () => ({ error: { code: -32000, message: 'no node here' } }),
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
			open: async () => ({ room: 'r1', invite: null, pairing: null }),
			send: async () => {},
			receive: async () => ({ closed: 'closed' }),
			close: async () => {}
		},
		wallet: async () => null,
		notice: () => {}
	};
}

const account = (address: string) => ({
	address,
	genesis: { factory: '0x01', singleton: '0x02', saltNonce: '0x1', initializer: '0x' },
	owners: [ME, PEER],
	threshold: 2,
	recorded: true,
	chainIds: [100],
	addedMs: 1
});

/** A voiding transaction at nonce 0, waiting on `safe` as an earlier page kept it. */
async function keptList(safe: string) {
	const tx = await cancelTx(safe, '0x0');
	const hash = await safeTxHash(100, safe, tx);
	return {
		service: SERVICE,
		at: 500,
		chains: [
			{
				chainId: 100,
				members: { deployed: true, owners: [ME, PEER], threshold: 2, nonce: '0' },
				proposals: [
					{
						chainId: 100,
						safe,
						safeTxHash: hash,
						safeTx: tx,
						proposer: ME,
						// A member's, so it is listed and counted (§37).
						approvals: [peerApproval(hash)],
						createdMs: 1,
						updatedMs: 1
					}
				]
			}
		]
	};
}

const settled = async () => {
	for (let i = 0; i < 50; i += 1) await new Promise((r) => setTimeout(r, 0));
};

afterEach(() => {
	proposalsHost.resetForTests();
	seams.used = [];
	seams.findings = [];
});

describe('the proposals machine’s host', () => {
	it('says what waits on every multisig — the kept list first — and reads the others apart from the shown one', async () => {
		const store = memoryStore({
			'vela.multisigWaiting': JSON.stringify({
				[OTHER.toLowerCase()]: await keptList(OTHER),
				[SHOWN.toLowerCase()]: await keptList(SHOWN)
			})
		});
		const http: string[] = [];
		await proposalsHost.boot({
			deps: deps(store, http),
			load: async () => {},
			facts: () => ({
				serviceUrl: SERVICE,
				accounts: [{ address: ME, name: 'Me', keys: [], route: 'vela' }],
				networks: [100]
			}),
			multisigs: () => ({ accounts: [account(SHOWN), account(OTHER)] as never })
		});
		await settled();
		const presence = proposalsHost.view!.presence;
		expect(presence[OTHER.toLowerCase()]).toMatchObject({ needs_you: 1, ready: 0 });
		expect(presence[SHOWN.toLowerCase()]).toMatchObject({ needs_you: 1, ready: 0 });
		expect(proposalsHost.view!.others_waiting).toBe(true);
		// The shown one is followed: reading the others asks the service of the other one alone.
		await proposalsHost.follow(account(SHOWN) as never);
		await settled();
		expect(proposalsHost.view!.followed).toBe(SHOWN);
		http.length = 0;
		await proposalsHost.othersTick();
		await settled();
		expect(http.some((u) => u.toLowerCase().includes(OTHER.toLowerCase()))).toBe(true);
		expect(http.some((u) => u.toLowerCase().includes(SHOWN.toLowerCase()))).toBe(false);
	});

	it('the one host routes a multisig’s record to the store and executed rows to the activity', async () => {
		const rows: unknown[] = [];
		const recorded = vi.fn();
		await proposalsHost.boot({
			deps: deps(memoryStore(), []),
			load: async () => {},
			facts: () => ({ serviceUrl: SERVICE, accounts: [], networks: [100] }),
			multisigs: () => ({ accounts: [] }),
			saveRows: async (r) => void rows.push(...r)
		});
		proposalsHost.onrecorded = recorded;
		machines.route({
			kind: 'account',
			address: OTHER,
			update: { kind: 'use_chain', chain_id: 8453 }
		});
		machines.route({
			kind: 'account',
			address: OTHER,
			update: { kind: 'findings_on', chain_id: 8453, findings: [] }
		});
		await proposalsHost.route({ kind: 'activity_rows', rows: [{ id: 'row-1' }] });
		expect(seams.used).toEqual([[OTHER, 8453]]);
		expect(seams.findings).toEqual([[OTHER, 8453]]);
		expect(rows).toEqual([{ id: 'row-1' }]);
		expect(recorded).toHaveBeenCalledOnce();
	});
});
