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
	refused: false
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
	draftCall: async () => fake.draft,
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
vi.mock('../core/names.svelte', () => ({ names: { label: (a: string) => a.slice(0, 6) } }));
vi.mock('../core/display', () => ({
	actionRecipient: (a: any) => a?.to ?? null,
	actionTitle: async () => 'Contract call',
	chainName: () => 'Gnosis',
	explorerTxUrl: () => null,
	formatUnits: (v: bigint) => String(v),
	localName: () => null,
	shortenAddress: (a: string) => a.slice(0, 8),
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 })
}));
vi.mock('../core/kernel', () => ({
	sameAddress: (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase()
}));
vi.mock('$lib/wallet/identicon', () => ({ identiconSvgForClient: () => '<svg></svg>' }));
vi.mock('$lib/services/networks', () => ({ nativeSymbol: () => 'xDAI' }));

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

function show(props: Record<string, unknown> = {}) {
	return render(ProposalSheet, {
		props: {
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
			call: { to: TO, value: '0x0', data: '0x095ea7b3' },
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
