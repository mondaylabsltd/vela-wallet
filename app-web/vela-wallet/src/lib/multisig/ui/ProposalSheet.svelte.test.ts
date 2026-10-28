/* eslint-disable @typescript-eslint/no-explicit-any -- stand-ins for the core's readings */
/**
 * One multisig transaction's sheet, in a browser.
 *
 * - dapp-1: a dApp's transaction is reviewed before anything is signed —
 *   "Propose and approve" is the only door to the passkey, beside Decline.
 * - ms-5: a member who would execute can approve without executing (and
 *   without paying the fee).
 * - ms-6: the proposer, approved already, can still withdraw it.
 * - ms-3: an execution the relay refused is said at once, with a retry.
 *
 * What it reads is stood in for (the core's suites drive it); what is real
 * is the sheet.
 */
import { tick } from 'svelte';
import type { SigningMessages } from '$lib/signing/messages';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const ME = '0x00000000000000000000000000000000000000aa';
const OTHER = '0x00000000000000000000000000000000000000bb';
const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = `0x${'ab'.repeat(32)}`;
const TO = '0x000000000000000000000000000000000000dEaD';

const fake = vi.hoisted(() => ({
	chain: null as any,
	draft: null as any,
	calls: [] as string[],
	execution: null as any,
	refused: false,
	/** Owners the passkey index knows: Vela accounts elsewhere (ms-11). */
	vela: [] as string[],
	/** What the chain says executed a done proposal (ms-9, ms-10). */
	executed: undefined as string | null | undefined
}));

vi.mock('../core/proposals.svelte', () => ({
	proposals: {
		loading: false,
		chain: () => fake.chain,
		refresh: async () => {}
	},
	approve: async () => void fake.calls.push('approve'),
	approveExternally: async () => void fake.calls.push('external'),
	execute: async () => {
		fake.calls.push('execute');
		return fake.execution;
	},
	proposeCancel: async () => void fake.calls.push('reject'),
	draftCalls: async () => fake.draft,
	MailboxUnreadableError: class extends Error {}
}));
vi.mock('../core/execute', () => ({
	ExecutorCannotPayError: class extends Error {},
	relayRefused: async () => fake.refused
}));
vi.mock('../core/owners', () => ({
	localOwners: (owners: string[]) =>
		owners
			.filter((o) => o.toLowerCase() === ME)
			.map((address) => ({ address, name: 'Mine', account: { address } }))
}));
vi.mock('../core/names.svelte', () => ({
	names: { label: (a: string) => a.slice(0, 6), isVela: (a: string) => fake.vela.includes(a) }
}));
vi.mock('../core/records', () => ({ findExecution: async () => fake.executed }));
vi.mock('../core/display', () => ({
	actionRecipient: (a: any) => a?.to ?? null,
	actionTitle: async () => 'Contract call',
	chainName: () => 'Gnosis',
	explorerTxUrl: () => null,
	formatUnits: (v: bigint) => String(v),
	localName: () => null,
	memberChanges: (_m: unknown, action: any) =>
		action?.kind === 'batch'
			? action.calls.flatMap((c: any, i: number) =>
					c.action.kind === 'changeThreshold' ? [`changes #${i + 1}`] : []
				)
			: [],
	shortenAddress: (a: string) => a.slice(0, 8),
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 })
}));
vi.mock('../core/kernel', () => ({
	sameAddress: (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase(),
	transfersOf: () => []
}));
vi.mock('$lib/wallet/identicon', () => ({ identiconSvgForClient: () => '<svg></svg>' }));
vi.mock('$lib/services/networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/networks')>()),
	nativeSymbol: () => 'xDAI'
}));

const { default: ProposalSheet } = await import('./ProposalSheet.svelte');

/** Every word is its own key, so a test names what it looks for. */
function words(): MultisigMessages {
	const out: any = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.proposal.siteAsks = '{{host}} asks for this transaction.';
	out.proposal.failed = 'Failed: {{reason}}';
	return out;
}

const members = { owners: [ME, OTHER], threshold: 2, nonce: 0n, deployed: true };
const local = { address: ME, name: 'Mine', account: { address: ME } };

function item(overrides: Record<string, unknown> = {}) {
	return {
		chainId: 100,
		safeTxHash: HASH,
		tx: {},
		review: { action: { kind: 'call', to: TO, value: '0', data: '0x095ea7b3' }, refusal: null },
		nonce: 0n,
		proposer: ME,
		approvals: [],
		approved: 0,
		threshold: 2,
		status: 'pending',
		canApprove: [local],
		canExecute: [],
		cancelledBy: null,
		createdMs: 1,
		...overrides
	};
}

const account = { address: SAFE, genesis: {}, owners: [ME, OTHER], threshold: 2 } as any;

/** The signing sheet's words, each its own name (a proposal's call reads as that sheet does). */
const names = { get: (_: object, key: string | symbol) => String(key) };
const signing = new Proxy({} as SigningMessages, {
	get: (_, key) => (key === 'certain' ? new Proxy({}, names) : String(key))
});

function show(props: Record<string, unknown> = {}) {
	return render(ProposalSheet, {
		props: {
			signing,
			m: words(),
			account,
			chainId: 100,
			safeTxHash: HASH,
			locale: 'en',
			wide: true,
			closeLabel: 'Close',
			onclose: () => {},
			...props
		}
	});
}

const buttons = () =>
	[...document.body.querySelectorAll('button')].map((b) => b.textContent?.trim() ?? '');
const press = (label: string) =>
	[...document.body.querySelectorAll('button')]
		.find((b) => b.textContent?.trim() === label)
		?.click();
const settle = async () => {
	for (let i = 0; i < 5; i += 1) {
		await tick();
		await new Promise((r) => setTimeout(r, 0));
	}
};

beforeEach(() => {
	fake.calls = [];
	fake.refused = false;
	fake.vela = [];
	fake.executed = undefined;
	fake.chain = { chainId: 100, members, items: [item()], mailbox: true };
	fake.draft = { chain: { chainId: 100, members, items: [], mailbox: true }, item: item() };
});

describe("a dApp's transaction from a multisig (dapp-1)", () => {
	it('is reviewed first: what the site asks, and nothing signed until Propose and approve', async () => {
		const propose = vi.fn(async () => HASH);
		const decline = vi.fn();
		const draft = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x095ea7b3' }],
			origin: 'https://app.example',
			safeTxHash: null,
			propose,
			decline,
			close: () => {}
		};
		const screen = show({ draft, safeTxHash: '' });
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('app.example asks for this transaction.');
		expect(text).toContain('proposal.data');
		// Owner 2026-10-28: the proposer reads the call decoded, as every member
		// will — before "Propose and approve", before any passkey.
		expect(document.querySelector('[data-testid="multisig-call-review"]')).not.toBeNull();
		expect(buttons()).toContain('send.propose');
		expect(buttons()).toContain('external.decline');
		// No approval, invite or withdrawal of a transaction that does not exist yet.
		expect(buttons()).not.toContain('proposal.approve');
		expect(buttons()).not.toContain('proposal.reject');
		expect(propose).not.toHaveBeenCalled();

		press('send.propose');
		await settle();
		expect(propose).toHaveBeenCalledTimes(1);

		press('external.decline');
		expect(decline).toHaveBeenCalledTimes(1);
		await screen.unmount();
	});
});

describe("a dApp's eth_sendTransaction its proposer alone makes ready (round 3, G2)", () => {
	it('is proposed and executed in one go, and the site is handed the real transaction', async () => {
		const TX = `0x${'cd'.repeat(32)}`;
		const one = { ...members, threshold: 1 };
		const mine = [{ owner: ME, counts: true, approval: {}, createdMs: 1 }];
		fake.draft = {
			chain: { chainId: 100, members: one, items: [], mailbox: true },
			item: item({ threshold: 1, canExecute: [local] })
		};
		fake.chain = {
			chainId: 100,
			members: one,
			mailbox: true,
			items: [
				item({
					threshold: 1,
					approved: 1,
					approvals: mine,
					status: 'ready',
					canApprove: [],
					canExecute: [local]
				})
			]
		};
		fake.execution = { maybeSent: false, userOpHash: '0x01', waitForTxHash: async () => TX };
		const executed = vi.fn();
		const base = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x095ea7b3' }],
			origin: 'https://app.example',
			safeTxHash: null as string | null,
			decline: () => {},
			close: () => {},
			executed
		};
		let screen: ReturnType<typeof show> | null = null;
		const draft = {
			...base,
			// What the host does once it is in the mailbox: the sheet shows the proposal.
			propose: async () => {
				await screen?.rerender({ draft: { ...draft, safeTxHash: HASH }, safeTxHash: HASH });
				return HASH;
			}
		};
		screen = show({ draft, safeTxHash: '' });
		await settle();
		// One door, worded for what it does: no wait for approvals nobody else gives.
		expect(buttons()).toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('send.propose');
		press('proposal.approveAndExecute');
		await settle();
		expect(fake.calls).toEqual(['execute']);
		expect(executed).toHaveBeenCalledWith(TX);
		await screen.unmount();
	});

	it('waiting for others, says how many approvals it still needs, and that the site was told', async () => {
		fake.chain.items = [
			item({
				approved: 1,
				canApprove: [],
				approvals: [{ owner: ME, counts: true, approval: {}, createdMs: 1 }]
			})
		];
		const words2 = words();
		words2.proposal.stillNeeded = 'needed {{missing}}';
		const draft = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x' }],
			origin: 'https://app.example',
			safeTxHash: HASH,
			propose: async () => HASH,
			decline: () => {},
			close: () => {},
			executed: () => {}
		};
		const screen = show({ draft, m: words2 });
		await settle();
		expect(document.body.textContent).toContain('needed 1');
		expect(document.body.textContent).toContain('proposal.siteWaits');
		await screen.rerender({ draft: { ...draft, siteTold: true } });
		await settle();
		expect(document.body.textContent).toContain('proposal.siteTold');
		expect(document.body.textContent).not.toContain('proposal.siteWaits');
		expect(document.body.textContent).not.toContain('needed 1');
		await screen.unmount();
	});
});

describe('a member who would execute it (ms-5)', () => {
	it('can approve without executing, beside Approve and execute', async () => {
		fake.chain.items = [item({ approved: 1, canExecute: [local] })];
		const screen = show();
		await settle();
		expect(buttons()).toContain('proposal.approveAndExecute');
		expect(buttons()).toContain('proposal.approve');
		press('proposal.approve');
		await settle();
		expect(fake.calls).toEqual(['approve']);
		await screen.unmount();
	});
});

describe('the proposer (ms-6)', () => {
	it('can withdraw its own proposal after approving it', async () => {
		fake.chain.items = [
			item({
				approved: 1,
				canApprove: [],
				approvals: [{ owner: ME, counts: true, approval: {}, createdMs: 1 }]
			})
		];
		const screen = show();
		await settle();
		expect(buttons()).toContain('proposal.reject');
		press('proposal.reject');
		await settle();
		// The confirmation, then the cancellation it proposes.
		[...document.body.querySelectorAll('button')]
			.filter((b) => b.textContent?.trim() === 'proposal.reject')
			.at(-1)
			?.click();
		await settle();
		expect(fake.calls).toEqual(['reject']);
		await screen.unmount();
	});
});

describe('an execution the relay refused (ms-3)', () => {
	it('is a failure with a retry at once, not "Executing…" for three minutes', async () => {
		fake.chain.items = [
			item({
				approved: 1,
				canApprove: [],
				canExecute: [local],
				approvals: [{ owner: OTHER, counts: true, approval: {}, createdMs: 1 }]
			})
		];
		fake.refused = true;
		fake.execution = {
			maybeSent: true,
			userOpHash: '0x01',
			waitForTxHash: () => new Promise(() => {})
		};
		const screen = show();
		await settle();
		press('proposal.approveAndExecute');
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('Failed: proposal.notSent');
		expect(text).not.toContain('proposal.executed');
		// The way to try again is still there.
		expect(buttons()).toContain('proposal.approveAndExecute');
		await screen.unmount();
	});
});

describe('a batch of calls (owner, 2026-10-28)', () => {
	it('reads every call, and says first, in red, a call that changes the multisig', async () => {
		const batch = {
			kind: 'batch',
			calls: [
				{
					to: TO,
					value: '0',
					data: '0x095ea7b3',
					action: { kind: 'call', to: TO, value: '0', data: '0x095ea7b3' }
				},
				{
					to: SAFE,
					value: '0',
					data: '0x694e80c3',
					action: { kind: 'changeThreshold', threshold: '1' }
				}
			]
		};
		fake.chain.items = [item({ review: { action: batch, refusal: null } })];
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="multisig-call-review"]')).not.toBeNull();
		expect(document.body.textContent).toContain('changes #2');
		// Every call's own bytes, in order.
		expect(document.body.textContent).toContain(`#2 ${SAFE} 0x694e80c3`);
		await screen.unmount();
	});
});

describe('a transaction whose number is used (ms-9, ms-10)', () => {
	const done = () => item({ status: 'done', canApprove: [], cancelledBy: `0x${'cd'.repeat(32)}` });

	it('executed anywhere: "Executed", never "1/2 approved", "Waiting" or a pending cancellation', async () => {
		fake.chain.items = [done()];
		fake.executed = `0x${'ee'.repeat(32)}`;
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.executed');
		expect(text).not.toContain('status.progress');
		expect(text).not.toContain('proposal.waiting');
		expect(text).not.toContain('proposal.cancelPending');
		expect(text).not.toContain('proposal.noExpiry');
		await screen.unmount();
	});

	it('voided by another transaction: said as cancelled', async () => {
		fake.chain.items = [done()];
		fake.executed = null;
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.voided');
		expect(text).toContain('proposal.voidedNote');
		await screen.unmount();
	});

	it('the chain cannot say: neither executed nor cancelled is claimed', async () => {
		fake.chain.items = [done()];
		fake.executed = undefined;
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.doneNote');
		expect(text).not.toContain('proposal.voided');
		await screen.unmount();
	});
});

describe('an owner whose wallet is elsewhere (ms-11, ms-23)', () => {
	it('is offered an outside wallet — a queued transaction too — but a Vela member is not', async () => {
		fake.chain.items = [item({ status: 'queued' })];
		let screen = show();
		await settle();
		expect(buttons()).toContain('external.signWithWallet');
		await screen.unmount();

		// The same member, known to the passkey index: it approves in Vela.
		fake.vela = [OTHER];
		screen = show();
		await settle();
		expect(buttons()).not.toContain('external.signWithWallet');
		await screen.unmount();
	});
});
