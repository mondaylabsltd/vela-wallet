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
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';
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
	executed: undefined as string | null | undefined,
	/** What the chain says executed another hash (the cancellation's, ms-9). */
	executedBy: {} as Record<string, string | null | undefined>,
	/** The shown multisig's other networks, as `proposals` reads them. */
	chains: [] as any[],
	followed: null as string | null,
	/** Where an approval signed here stands, while the mailbox lacks it. */
	kept: null as any,
	sentAgain: 0,
	/** Each executor's look at the execution (r4-5, r4-7), by address. */
	previews: {} as Record<string, any>,
	/** The look each execute was handed. */
	executedWith: [] as any[],
	/** What execute throws before its passkey (r4-11). */
	executeThrows: null as any
}));

vi.mock('../core/proposals.svelte', () => ({
	proposals: {
		loading: false,
		chain: () => fake.chain,
		refresh: async () => {},
		get chains() {
			return fake.chains;
		},
		get followed() {
			return fake.followed;
		}
	},
	outbox: {
		stateOf: () => fake.kept,
		sendAgain: async () => {
			fake.sentAgain += 1;
			fake.kept = null;
			return [];
		}
	},
	approve: async () => void fake.calls.push('approve'),
	approveExternally: async () => void fake.calls.push('external'),
	execute: async (_a: unknown, _i: unknown, _o: unknown, _m: unknown, preview: unknown) => {
		fake.executedWith.push(preview);
		if (fake.executeThrows) throw fake.executeThrows;
		fake.calls.push('execute');
		return fake.execution;
	},
	executionInput: (_a: unknown, item: any, executor: any, members: unknown) => ({
		chainId: item.chainId,
		executor,
		members
	}),
	proposeCancel: async () => void fake.calls.push('reject'),
	draftCalls: async () => fake.draft,
	MailboxUnreadableError: class extends Error {},
	NoOutsideSignerError: class extends Error {}
}));
vi.mock('../core/execute', () => {
	class ExecutorCannotPayError extends Error {}
	class ExecutionWouldFailError extends Error {
		constructor(readonly why: unknown) {
			super('would fail');
		}
	}
	class ExecutionUnpricedError extends Error {}
	return {
		ExecutorCannotPayError,
		ExecutionWouldFailError,
		ExecutionUnpricedError,
		stoppedBeforeSigning: (e: unknown) =>
			e instanceof ExecutorCannotPayError ||
			e instanceof ExecutionWouldFailError ||
			e instanceof ExecutionUnpricedError,
		relayRefused: async () => fake.refused
	};
});
vi.mock('../core/preflight.svelte', () => ({
	PREVIEW_FRESH_MS: 15_000,
	preflight: {
		get: (input: any) => fake.previews[input.executor.address] ?? null,
		check: async (input: any) => fake.previews[input.executor.address] ?? { kind: 'unpriced' }
	}
}));
// How each member signs: this device's, a Vela account the index knows, else a plain wallet.
vi.mock('../core/signers.svelte', () => ({
	signers: {
		kindOf: (_chain: number, a: string) =>
			a.toLowerCase() === ME ? 'here' : fake.vela.includes(a) ? 'vela' : 'wallet'
	}
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
// A rename executed here reads the record's new name: not this sheet's test.
vi.mock('../core/accounts', () => ({ recheckRecord: async () => {} }));
vi.mock('../core/records', () => ({
	findExecution: async (_safe: string, _chain: number, hash: string) =>
		hash in fake.executedBy ? fake.executedBy[hash] : fake.executed
}));
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
vi.mock('../core/kernel', () => {
	const same = (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase();
	return {
		sameAddress: same,
		transfersOf: () => [],
		memberDrift: (was: { owners: string[] }, is: { owners: string[] }) => ({
			same:
				was.owners.length === is.owners.length &&
				was.owners.every((o) => is.owners.some((x) => same(x, o)))
		})
	};
});
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
		unsent: false,
		renameUndeployed: false,
		...overrides
	};
}

const account = { address: SAFE, genesis: {}, owners: [ME, OTHER], threshold: 2 } as any;

/** A look that says it runs, for `fee` wei, by an executor holding `balance`. */
function priced(over: Record<string, unknown> = {}) {
	return {
		kind: 'priced',
		calls: [],
		fee: { amount: 35n * 10n ** 15n, recipient: '0xfe' },
		balance: 1000n,
		canPay: true,
		setup: { multisig: false, members: [], executor: false, amount: null },
		...over
	};
}

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
	fake.executedBy = {};
	fake.chains = [];
	fake.followed = null;
	fake.kept = null;
	fake.sentAgain = 0;
	fake.previews = { [ME]: priced() };
	fake.executedWith = [];
	fake.executeThrows = null;
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

/**
 * A rename is called BY the Safe on the chain whose registry holds the
 * record — the index chain (Gnosis) — whatever networks the multisig is used
 * on: the review says where it runs, and that executing deploys the multisig
 * there first when it is not yet.
 */
describe('a rename on chain (registry v4)', () => {
	const rename = { review: { action: { kind: 'rename', name: 'Team' }, refusal: null } };

	it('says it runs on the registry’s chain — and never that executing deploys it', async () => {
		fake.chain = {
			chainId: 100,
			members: { ...members, deployed: false },
			items: [item({ ...rename, renameUndeployed: true, canExecute: [] })],
			mailbox: true
		};
		const screen = show();
		await settle();
		const where = document.querySelector('[data-testid="rename-where"]')?.textContent ?? '';
		expect(where).toContain('proposal.renameRuns');
		expect(document.querySelector('[data-testid="rename-undeployed"]')).not.toBeNull();
		await screen.unmount();
	});

	it('says only where it runs once it is deployed there', async () => {
		fake.chain.items = [item(rename)];
		const screen = show();
		await settle();
		const where = document.querySelector('[data-testid="rename-where"]')?.textContent ?? '';
		expect(where).toContain('proposal.renameRuns');
		expect(document.querySelector('[data-testid="rename-undeployed"]')).toBeNull();
		await screen.unmount();
	});

	it('another transaction says neither', async () => {
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="rename-where"]')).toBeNull();
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

	it('replaced by its executed cancellation: one wording, nothing that says otherwise', async () => {
		fake.chain.items = [done()];
		fake.executed = null;
		fake.executedBy = { [`0x${'cd'.repeat(32)}`]: `0x${'ee'.repeat(32)}` };
		const screen = show();
		await settle();
		const text = document.body.textContent ?? '';
		expect(text).toContain('proposal.voided');
		expect(document.querySelector('[data-testid="proposal-voided"]')?.textContent?.trim()).toBe(
			'proposal.voidedNote'
		);
		expect(text).not.toContain('proposal.voidedByOther');
		expect(text).not.toContain('proposal.executed');
		expect(text).not.toContain('proposal.doneNote');
		expect(text).not.toContain('proposal.cancelPending');
		expect(text).not.toContain('proposal.waiting');
		expect(text).not.toContain('proposal.noExpiry');
		expect(text).not.toContain('status.progress');
		await screen.unmount();
	});

	it('replaced by another transaction at its number: said as that, not as a cancellation', async () => {
		fake.chain.items = [done()];
		fake.executed = null;
		fake.executedBy = { [`0x${'cd'.repeat(32)}`]: null };
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="proposal-voided"]')?.textContent?.trim()).toBe(
			'proposal.voidedByOther'
		);
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

describe('a link’s transaction on a device with no member (ms-21)', () => {
	const THIRD = '0x00000000000000000000000000000000000000cc';
	const strangers = { owners: [OTHER, THIRD], threshold: 2, nonce: 0n, deployed: true };
	const linked = (refresh = vi.fn(async () => {})) => ({
		loading: false,
		chain: () => ({
			chainId: 100,
			members: strangers,
			items: [item({ canApprove: [], proposer: OTHER })],
			mailbox: true
		}),
		refresh
	});

	it('is read from its own source, says no member is here, and offers the two ways on', async () => {
		// The shown multisig's list has nothing: the sheet reads the link's source.
		fake.chain = null;
		const onwatch = vi.fn();
		const screen = show({
			source: linked(),
			watch: { added: false, onwatch },
			account: { ...account, owners: strangers.owners }
		});
		await settle();
		const preview = document.querySelector('[data-testid="proposal-preview"]');
		expect(preview?.textContent).toContain('proposal.previewNote');
		expect(document.body.textContent).not.toContain('proposal.notMember');
		expect(document.body.textContent).not.toContain('proposal.watching');
		expect(buttons()).toContain('proposal.watch');
		expect(buttons()).toContain('external.signWithWallet');
		// Nothing to approve or execute from here, and nothing was done.
		expect(buttons()).not.toContain('proposal.approve');
		press('proposal.watch');
		expect(onwatch).toHaveBeenCalledTimes(1);
		await screen.unmount();
	});

	it('kept here: said, and no second Watch', async () => {
		fake.chain = null;
		const screen = show({
			source: linked(),
			watch: { added: true, onwatch: () => {} },
			account: { ...account, owners: strangers.owners }
		});
		await settle();
		expect(document.body.textContent).toContain('proposal.watching');
		expect(buttons()).not.toContain('proposal.watch');
		await screen.unmount();
	});

	it('an outside approval re-reads the link’s source, not the shown multisig’s', async () => {
		fake.chain = null;
		const refresh = vi.fn(async () => {});
		const screen = show({
			source: linked(refresh),
			watch: { added: false, onwatch: () => {} },
			account: { ...account, owners: strangers.owners }
		});
		await settle();
		const external = [...document.querySelectorAll('[data-testid="proposal-preview"] button')].find(
			(b) => b.textContent?.trim() === 'external.signWithWallet'
		) as HTMLButtonElement;
		external.click();
		await settle();
		expect(fake.calls).toContain('external');
		expect(refresh).toHaveBeenCalled();
		await screen.unmount();
	});
});

describe('every artwork opens the identicon viewer (founder call, 2026-09-05)', () => {
	afterEach(() => identiconViewer.close());

	it('the recipient and each member, on its own address; a row control stays beside it', async () => {
		const screen = show();
		await settle();
		document.body.querySelector<HTMLButtonElement>('.to button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(TO);

		const rows = [...document.body.querySelectorAll('.approvals li')];
		expect(rows).toHaveLength(2);
		rows[1].querySelector<HTMLButtonElement>('button.identicon')?.click();
		expect(identiconViewer.current?.address).toBe(OTHER);
		// "Sign with another wallet" is the row's own control, beside the
		// artwork's button: no button holds another.
		expect(rows[1].textContent).toContain('external.signWithWallet');
		expect(document.body.querySelector('button button')).toBeNull();
		await screen.unmount();
	});
});

const STRANGER = '0x00000000000000000000000000000000000000ee';
const transfer = { kind: 'transfer', token: null, to: TO, amount: '1' };

/** Owners are each network's: what the sheet says about who counts, and where it starts. */
describe('members per network', () => {
	it('shows an approval from an account that is no member here as not counted', async () => {
		fake.chain = {
			chainId: 100,
			members,
			mailbox: true,
			items: [
				item({
					review: { action: transfer, refusal: null },
					approvals: [{ owner: STRANGER, counts: false, approval: {}, createdMs: 1 }]
				})
			]
		};
		show();
		await settle();
		const line = document.body.querySelector('[data-testid="not-counted"]');
		expect(line?.textContent).toContain('proposal.notCounted');
		// Not among the members' rows either: only this network's members are listed.
		expect(document.body.querySelectorAll('.approvals li')).toHaveLength(2);
	});

	it('says a new network starts with the original members when another network changed', async () => {
		const undeployed = { owners: [ME, OTHER], threshold: 2, nonce: 0n, deployed: false };
		fake.chain = {
			chainId: 100,
			members: undeployed,
			mailbox: true,
			items: [item({ review: { action: transfer, refusal: null } })]
		};
		fake.followed = SAFE;
		fake.chains = [
			{ chainId: 100, members: undeployed, items: [] },
			{ chainId: 10, members: { owners: [ME, STRANGER], threshold: 1, deployed: true }, items: [] }
		];
		show();
		await settle();
		const notice = document.body.querySelector('[data-testid="starts-original"]');
		expect(notice?.textContent).toContain('proposal.startsOriginal');
		expect(notice?.querySelectorAll('li')).toHaveLength(2);
	});

	it('says nothing of it when every network still has the original members', async () => {
		const undeployed = { owners: [ME, OTHER], threshold: 2, nonce: 0n, deployed: false };
		fake.chain = { chainId: 100, members: undeployed, mailbox: true, items: [item()] };
		fake.followed = SAFE;
		fake.chains = [
			{ chainId: 10, members: { owners: [OTHER, ME], threshold: 2, deployed: true }, items: [] }
		];
		show();
		await settle();
		expect(document.body.querySelector('[data-testid="starts-original"]')).toBeNull();
	});
});

describe('a rename (registry v4)', () => {
	it('never offers to deploy: where it is not deployed, it says so and cannot be executed', async () => {
		fake.chain = {
			chainId: 100,
			members: { ...members, deployed: false },
			mailbox: true,
			items: [
				item({
					review: { action: { kind: 'rename', name: 'Team treasury' }, refusal: null },
					renameUndeployed: true,
					canExecute: [],
					canApprove: []
				})
			]
		};
		show();
		await settle();
		expect(document.body.querySelector('[data-testid="rename-undeployed"]')?.textContent).toContain(
			'proposal.renameUndeployed'
		);
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons()).not.toContain('proposal.approveAndExecute');
	});
});

describe('what was signed here and the mailbox has not taken', () => {
	it('says when it is sent again, then offers Send again', async () => {
		fake.chain = {
			chainId: 100,
			members,
			mailbox: true,
			items: [item({ unsent: true })]
		};
		fake.kept = { why: 'busy', retryAt: Date.now() + 24_000 };
		const words0 = words();
		words0.proposal.outboxBusy = 'busy — again in {{seconds}} s';
		show({ m: words0 });
		await settle();
		const line = document.body.querySelector('[data-testid="proposal-unsent"]');
		expect(line?.textContent).toMatch(/busy — again in 2[34] s/);
		expect(buttons()).not.toContain('proposal.outboxSendAgain');
	});

	it('kept, not sent on its own any more: Send again sends what was signed', async () => {
		fake.chain = { chainId: 100, members, mailbox: true, items: [item({ unsent: true })] };
		fake.kept = { why: 'busy', retryAt: null };
		show();
		await settle();
		expect(document.body.textContent).toContain('proposal.outboxNotSent');
		press('proposal.outboxSendAgain');
		await settle();
		expect(fake.sentAgain).toBe(1);
		// Never a second signature: no approve, no passkey.
		expect(fake.calls).toEqual([]);
	});
});

describe('an execution looked at before its passkey (r4-5, r4-7)', () => {
	const ready = (over: Record<string, unknown> = {}) =>
		item({
			approved: 1,
			status: 'pending',
			canExecute: [local],
			approvals: [{ owner: OTHER, counts: true, approval: {}, createdMs: 1 }],
			canApprove: [local],
			...over
		});

	it('that would fail says why, and offers Approve — never Execute', async () => {
		fake.chain.items = [ready()];
		fake.previews = {
			[ME]: {
				kind: 'fails',
				why: { kind: 'short', symbol: 'USDC', decimals: 0, have: 28n, need: 50n }
			}
		};
		const m = words();
		m.proposal.wouldFail = 'Would fail: {{reason}}.';
		m.proposal.whyShort = 'holds {{have}}, needs {{need}}';
		const screen = show({ m });
		await settle();
		const said = document.querySelector('[data-testid="execute-would-fail"]');
		expect(said?.textContent?.trim()).toBe('Would fail: holds 28 USDC, needs 50 USDC.');
		expect(buttons()).not.toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons()).toContain('proposal.approve');
		expect(fake.calls).toEqual([]);
		await screen.unmount();
	});

	it('shows the fee before the tap, with what it sets up, and signs that very look', async () => {
		fake.chain.items = [ready()];
		const look = priced({
			setup: { multisig: true, members: [OTHER], executor: false, amount: 20n * 10n ** 15n }
		});
		fake.previews = { [ME]: look };
		fake.execution = { maybeSent: false, userOpHash: '0x01', waitForTxHash: async () => '0x02' };
		const m = words();
		m.proposal.feeLine = 'Fee {{fee}} by {{name}}';
		m.proposal.feeSetup = 'Setup on {{network}} {{amount}}:';
		m.proposal.setupMember = 'activating {{name}}';
		const screen = show({ m });
		await settle();
		const fee = document.querySelector('[data-testid="execute-fee"]')?.textContent ?? '';
		expect(fee).toContain('Fee 0.035 xDAI by 0x0000');
		expect(fee).toContain('Setup on Gnosis 0.02 xDAI:');
		expect(fee).toContain('proposal.setupMultisig');
		expect(fee).toContain('activating 0x0000');
		press('proposal.approveAndExecute');
		await settle();
		expect(fake.calls).toEqual(['execute']);
		expect(fake.executedWith).toEqual([look]);
		await screen.unmount();
	});

	it('while the look is out, says so and holds Execute', async () => {
		fake.chain.items = [ready()];
		fake.previews = {};
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="execute-checking"]')).not.toBeNull();
		const execute = [...document.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === 'proposal.approveAndExecute'
		) as HTMLButtonElement;
		expect(execute.disabled).toBe(true);
		await screen.unmount();
	});
});

describe('who executes (r4-10, r4D-1)', () => {
	const SECOND = { address: OTHER, name: 'Second', account: { address: OTHER } };

	it('lists each executor’s coin, marks who cannot pay, and picks one who can', async () => {
		fake.chain.items = [
			item({
				approved: 2,
				status: 'ready',
				canApprove: [],
				canExecute: [local, SECOND],
				approvals: [
					{ owner: ME, counts: true, approval: {}, createdMs: 1 },
					{ owner: OTHER, counts: true, approval: {}, createdMs: 1 }
				]
			})
		];
		fake.previews = {
			[ME]: priced({ balance: 10n, canPay: false }),
			[OTHER]: priced({ balance: 900n })
		};
		const screen = show();
		await settle();
		const balances = [...document.querySelectorAll('[data-testid="actor-balance"]')].map((b) =>
			b.textContent?.replace(/\s+/g, ' ').trim()
		);
		expect(balances).toEqual(['10 xDAI · proposal.pickerCantPay', '900 xDAI']);
		const on = document.querySelector('.actor label.on')?.textContent ?? '';
		expect(on, 'the one who can pay is picked').toContain('Second');
		expect(buttons()).toContain('proposal.execute');
		await screen.unmount();
	});

	it('picked one who cannot pay: said before the tap; Approve is the action', async () => {
		fake.chain.items = [item({ approved: 1, canExecute: [local], canApprove: [local] })];
		fake.previews = { [ME]: priced({ balance: 0n, canPay: false }) };
		const m = words();
		m.proposal.cannotPay = '{{name}} cannot pay {{fee}}, has {{balance}}';
		const screen = show({ m });
		await settle();
		expect(document.querySelector('[data-testid="execute-cant-pay"]')?.textContent?.trim()).toBe(
			`0x0000 cannot pay ${35n * 10n ** 15n} xDAI, has 0 xDAI`
		);
		expect(buttons()).not.toContain('proposal.approveAndExecute');
		expect(buttons()).toContain('proposal.approve');
		await screen.unmount();
	});
});

describe('"Approve and execute" whose execute part cannot run (r4-11)', () => {
	it('keeps the approval, and says only executing could not run', async () => {
		fake.chain.items = [item({ approved: 1, canExecute: [local], canApprove: [local] })];
		const { ExecutorCannotPayError } = await import('../core/execute');
		fake.executeThrows = new ExecutorCannotPayError(1n, 0n);
		const screen = show();
		await settle();
		press('proposal.approveAndExecute');
		await settle();
		expect(fake.calls).toEqual(['approve']);
		const said = document.querySelector('[data-testid="approved-not-executed"]');
		expect(said?.textContent).toContain('proposal.approvedNotExecuted');
		expect(document.body.textContent).not.toContain('Failed:');
		await screen.unmount();
	});
});

describe('how each missing member approves (r4-9, r4D-6)', () => {
	it('a Vela member elsewhere: never an outside wallet — the link, in Vela', async () => {
		fake.vela = [OTHER];
		fake.chain.items = [item()];
		const screen = show();
		await settle();
		expect(buttons()).not.toContain('external.signWithWallet');
		expect(buttons()).toContain('proposal.sendLink');
		expect(document.querySelector('[data-testid="approves-in-vela"]')).not.toBeNull();
		await screen.unmount();
	});

	it('enough approved: the others are "Not needed", and nothing asks them to sign', async () => {
		fake.chain = {
			chainId: 100,
			members: { ...members, owners: [ME, TO, OTHER] },
			mailbox: true,
			items: [
				item({
					approved: 2,
					status: 'ready',
					canApprove: [],
					approvals: [
						{ owner: ME, counts: true, approval: {}, createdMs: 1 },
						{ owner: TO, counts: true, approval: {}, createdMs: 1 }
					]
				})
			]
		};
		const screen = show();
		await settle();
		expect(document.body.textContent).toContain('proposal.notNeeded');
		expect(buttons()).not.toContain('external.signWithWallet');
		expect(buttons()).not.toContain('proposal.sendLink');
		await screen.unmount();
	});
});

describe('Reject (r4D-14)', () => {
	it('has a way back', async () => {
		fake.chain.items = [item()];
		const screen = show();
		await settle();
		press('proposal.reject');
		await settle();
		expect(buttons()).toContain('proposal.rejectCancel');
		press('proposal.rejectCancel');
		await settle();
		expect(buttons()).not.toContain('proposal.rejectCancel');
		expect(fake.calls).toEqual([]);
		await screen.unmount();
	});
});

describe('a dApp’s transaction on a device with no member (r4C-10)', () => {
	it('says "Propose", never "Propose and approve": nobody here approves it', async () => {
		const THIRD = '0x00000000000000000000000000000000000000cc';
		const strangers = { owners: [OTHER, THIRD], threshold: 2, nonce: 0n, deployed: true };
		fake.draft = {
			chain: { chainId: 100, members: strangers, items: [], mailbox: true },
			item: item({ canApprove: [], canExecute: [] })
		};
		const draft = {
			rid: 'r1',
			address: SAFE,
			chainId: 100,
			method: 'eth_sendTransaction',
			calls: [{ to: TO, value: '0x0', data: '0x095ea7b3' }],
			origin: 'https://app.example',
			safeTxHash: null,
			propose: async () => HASH,
			decline: () => {},
			close: () => {}
		};
		const screen = show({ draft, safeTxHash: '' });
		await settle();
		expect(buttons()).toContain('send.proposeOnly');
		expect(buttons()).not.toContain('send.propose');
		await screen.unmount();
	});
});
