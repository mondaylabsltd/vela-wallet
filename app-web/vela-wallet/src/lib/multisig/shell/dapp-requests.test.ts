/**
 * A dApp's request to a multisig, held by the proposals machine (the real
 * wasm, a simulated shell) and carried by `holdDappRequest`: what the
 * machine decides (Rust: tests/app_multisig_dapp.rs) reaches the site
 * exactly once, and the sheet follows it.
 *
 * - dapp-7: a message is refused — said first, answered (4200) once read;
 * - a request the core cannot read is answered at once, no sheet left up;
 * - a request it reads opens its review (the machine's held request) and
 *   every way out answers the site: a close before proposing is 4001, the
 *   sheet goes with it; leaving the surface owes the same, at once.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { MultisigShellDeps } from './handler';

const SERVICE = 'http://service.test';
const SAFE = '0x00000000000000000000000000000000000000a1';
const ME = '0x00000000000000000000000000000000000000aa';
const DEAD = '0x000000000000000000000000000000000000dEaD';

vi.stubGlobal('window', new EventTarget());
vi.stubGlobal('document', Object.assign(new EventTarget(), { visibilityState: 'visible' }));
vi.mock('../core/store.svelte', () => ({
	multisig: {
		accounts: [],
		useChain: () => {},
		setFindingsOn: () => {},
		send: () => {},
		attach: () => {}
	}
}));
vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({ contactsBook: { view: null } }));
vi.mock('$lib/session/core/session.svelte', () => ({ session: { view: { accounts: [] } } }));

const { proposalsHost } = await import('./proposals-host.svelte');
const { holdDappRequest } = await import('./dapp-requests.svelte');
const { multisigUi } = await import('../core/ui.svelte');

function deps(over: Partial<MultisigShellDeps> = {}): MultisigShellDeps {
	const map = new Map<string, string>();
	return {
		// The mailbox is out of reach: a draft cannot take a number.
		fetch: (async () => new Response('{}', { status: 503 })) as unknown as typeof fetch,
		rpc: async () => ({ error: { code: -32000, message: 'no node here' } }),
		storage: () => ({
			getItem: (k: string) => map.get(k) ?? null,
			setItem: (k: string, v: string) => void map.set(k, v),
			removeItem: (k: string) => void map.delete(k)
		}),
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
		notice: () => {},
		...over
	};
}

const account = {
	address: SAFE,
	genesis: { factory: '0x01', singleton: '0x02', saltNonce: '0x1', initializer: '0x' },
	owners: [ME],
	threshold: 1,
	recorded: true,
	chainIds: [100],
	addedMs: 1
} as never;

const settled = async () => {
	for (let i = 0; i < 50; i += 1) await new Promise((r) => setTimeout(r, 0));
};

async function boot(over: Partial<MultisigShellDeps> = {}, list: unknown[] = []) {
	await proposalsHost.boot({
		deps: deps(over),
		load: async () => {},
		facts: () => ({
			serviceUrl: SERVICE,
			accounts: [{ address: ME, name: 'Me', keys: [], route: 'vela' }],
			networks: [100]
		}),
		multisigs: () => ({ accounts: [account, ...list] as never })
	});
}

function handOff(method: string, chainId = 100) {
	const answers: unknown[] = [];
	const refused: unknown[] = [];
	return {
		answers,
		refused,
		request: {
			rid: 'r1',
			account,
			method,
			params:
				method === 'eth_sendTransaction'
					? [{ from: SAFE, to: DEAD, value: '0x1', data: '0x' }]
					: ['0xdead', SAFE],
			origin: 'https://app.example',
			chainId,
			limitMs: 300_000,
			claim: async () => true,
			refused: (error: unknown) => void refused.push(error),
			answer: (answer: unknown) => void answers.push(answer)
		}
	};
}

afterEach(() => {
	proposalsHost.resetForTests();
	multisigUi.close();
});

describe("a dApp's request to a multisig, held by the machine", () => {
	it('dapp-7: a message is said first, and answered its refusal once read', async () => {
		await boot();
		const h = handOff('personal_sign');
		const hold = await holdDappRequest(h.request);
		expect(h.refused).toEqual([
			{ code: 4200, message: 'A multisig account signs no messages yet' }
		]);
		expect(h.answers).toEqual([]);
		expect(multisigUi.sheet).toBeNull();
		hold.leave();
		expect(h.answers).toEqual([
			{ error: { code: 4200, message: 'A multisig account signs no messages yet' } }
		]);
		await settled();
		// Answered: the machine forgets it, and nothing is answered twice.
		expect(proposalsHost.view!.dapp).toEqual({});
		hold.leave();
		expect(h.answers).toHaveLength(1);
	});

	it('a transaction with no chain is answered at once; no sheet is left up', async () => {
		await boot();
		const h = handOff('eth_sendTransaction', 0);
		await holdDappRequest(h.request);
		expect(h.answers).toEqual([
			{ error: { code: -32602, message: 'A transaction needs a chain' } }
		]);
		expect(multisigUi.sheet).toBeNull();
	});

	it('its review opens on the held request; a close before proposing is 4001, the sheet goes', async () => {
		await boot();
		const h = handOff('eth_sendTransaction');
		const hold = await holdDappRequest(h.request);
		await settled();
		const sheet = multisigUi.sheet;
		expect(sheet?.kind).toBe('dapp');
		if (sheet?.kind !== 'dapp') return;
		expect(sheet.request.held).toMatchObject({ id: 'r1', stage: 'reviewing', safe: SAFE });
		// Nothing is signed or answered while it is reviewed.
		expect(h.answers).toEqual([]);
		hold.leave();
		expect(h.answers).toEqual([{ error: { code: 4001, message: 'User rejected the request' } }]);
		expect(multisigUi.sheet).toBeNull();
		expect(hold.closing()).toBeNull();
	});

	it('the surface going away owes the machine’s answer, at once', async () => {
		await boot();
		const h = handOff('eth_sendTransaction');
		const hold = await holdDappRequest(h.request);
		await settled();
		expect(hold.closing()).toEqual({
			error: { code: 4001, message: 'User rejected the request' }
		});
	});
});

/**
 * §31: a site connected to this multisig may never make it act for a Safe it
 * owns — another multisig on this device (refused at once), or one whose
 * `isOwner(this multisig)` reads true, or cannot be read (refused after the
 * machine's own read). Refused like a message (dapp-7: said first, answered
 * once read), with `data.reason: "parent_safe"` and the key it is said in.
 */
describe('§31: a request to act for a Safe this multisig owns', () => {
	const PARENT = '0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe';
	const UNKNOWN = '0x7777777777777777777777777777777777777777';
	const approveHash = '0xd4d9bdcd' + 'cd'.repeat(32);
	const TRUE = '0x' + '0'.repeat(63) + '1';
	const parentRow = { ...(account as object), address: PARENT, owners: [SAFE] };

	function toSafe(to: string) {
		const h = handOff('eth_sendTransaction');
		h.request.params = [{ from: SAFE, to, value: '0x0', data: approveHash }];
		return h;
	}

	it('another multisig on this device: refused at once, no sheet, answered -32603 once read', async () => {
		const reads: unknown[] = [];
		await boot(
			{
				rpc: async (chainId, method, params) => {
					reads.push({ chainId, method, params });
					return { error: { code: -32000, message: 'no node here' } };
				}
			},
			[parentRow]
		);
		const h = toSafe(PARENT);
		const hold = await holdDappRequest(h.request);
		await settled();
		expect(h.refused).toEqual([
			{
				code: -32603,
				message: expect.stringContaining('approveHash on ' + PARENT),
				data: {
					reason: 'parent_safe',
					why: 'member',
					safe: PARENT,
					function: 'approveHash',
					key: 'componentsUi.signing.parentSafeBlocked'
				}
			}
		]);
		expect(reads.filter((r) => (r as { method: string }).method === 'eth_call')).toEqual([]);
		expect(multisigUi.sheet).toBeNull();
		expect(h.answers).toEqual([]);
		hold.leave();
		expect(h.answers).toEqual([{ error: h.refused[0] }]);
	});

	it('a Safe not on the list whose isOwner reads true: refused `owner`, its sheet taken down', async () => {
		const calls: unknown[] = [];
		await boot({
			rpc: async (chainId, method, params) => {
				calls.push({ chainId, method, params });
				return method === 'eth_call'
					? { result: TRUE }
					: { error: { code: -32000, message: 'no node here' } };
			}
		});
		const h = toSafe(UNKNOWN);
		await holdDappRequest(h.request);
		await settled();
		expect(calls).toContainEqual({
			chainId: 100,
			method: 'eth_call',
			params: [{ to: UNKNOWN, data: '0x2f54bf6e' + SAFE.slice(2).padStart(64, '0') }, 'latest']
		});
		expect(h.refused).toMatchObject([
			{
				code: -32603,
				data: { reason: 'parent_safe', why: 'owner', safe: UNKNOWN, function: 'approveHash' }
			}
		]);
		expect(multisigUi.sheet).toBeNull();
	});

	it('a Safe whose ownership cannot be read: refused `unchecked`, in the unchecked words', async () => {
		await boot({
			rpc: async () => {
				throw new Error('no node answered');
			}
		});
		const h = toSafe(UNKNOWN);
		await holdDappRequest(h.request);
		await settled();
		expect(h.refused).toMatchObject([
			{
				code: -32603,
				data: {
					reason: 'parent_safe',
					why: 'unchecked',
					safe: UNKNOWN,
					key: 'componentsUi.signing.parentSafeUnchecked'
				}
			}
		]);
		expect(multisigUi.sheet).toBeNull();
	});
});
