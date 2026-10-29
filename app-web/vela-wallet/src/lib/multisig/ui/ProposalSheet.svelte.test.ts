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
	/** §3.5: what this session's check of the network found (`[]`: only its members act). */
	checked: [] as any[],
	/** The core's reading of a member change (r4a-1). */
	memberReading: null as any,
	/** What an executed transaction's receipt says (r4a-8). */
	frozen: null as any,
	/** What this device recorded when it executed one. */
	remembered: [] as any[],
	/** The core's ruling on a token nobody vouches for (r4a-2). */
	tokenNote: null as any
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
	execute: async () => {
		fake.calls.push('execute');
		return fake.execution;
	},
	proposeCancel: async () => void fake.calls.push('reject'),
	draftCalls: async () => fake.draft,
	MailboxUnreadableError: class extends Error {}
}));
// §3.5: the network's check, as the chain would answer it (`recheck.test.ts` reads it).
vi.mock('../core/recheck', () => ({
	checkOn: async () => fake.checked,
	findingsOn: () => []
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
	memberWho: (a: string) => a.slice(0, 6),
	changeLines: (_m: unknown, reading: any, network: string) => ({
		changes: reading.changes.map((c: any) =>
			c.call === null ? `change.${c.kind}` : `#${c.call} · change.${c.kind}`
		),
		rule: `change.rule ${reading.before.threshold}/${reading.before.owners} → ${reading.after.threshold}/${reading.after.owners}`,
		caution: `change.caution ${network}`,
		doesNotFit: reading.fits ? null : 'change.doesNotFit'
	}),
	shortenAddress: (a: string) => a.slice(0, 8),
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 })
}));
vi.mock('../core/kernel', () => {
	const same = (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase();
	return {
		sameAddress: same,
		transfersOf: () => [],
		readMemberChanges: () => fake.memberReading,
		memberDrift: (was: { owners: string[] }, is: { owners: string[] }) => ({
			same:
				was.owners.length === is.owners.length &&
				was.owners.every((o) => is.owners.some((x) => same(x, o)))
		})
	};
});
vi.mock('$lib/wallet/identicon', () => ({ identiconSvgForClient: () => '<svg></svg>' }));
vi.mock('../core/executed', () => ({
	factsOfExecution: (
		approvers: string[],
		executor: string,
		_owner: boolean,
		threshold: number
	) => ({
		executor,
		threshold,
		approvers,
		executorApproved: false
	}),
	frozenFacts: async () => fake.frozen,
	rememberExecution: (...args: any[]) => void fake.remembered.push(args)
}));
vi.mock('$lib/services/token-provenance', () => ({
	provenanceNote: () => fake.tokenNote
}));
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
	fake.checked = [];
	fake.memberReading = null;
	fake.frozen = null;
	fake.remembered = [];
	fake.tokenNote = null;
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
		fake.memberReading = {
			changes: [
				{
					call: 2,
					kind: 'threshold',
					added: null,
					removed: null,
					before: { threshold: 2, owners: 2 },
					after: { threshold: 1, owners: 2 },
					fits: true
				}
			],
			before: { threshold: 2, owners: 2 },
			after: { threshold: 1, owners: 2 },
			fits: true
		};
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="multisig-call-review"]')).not.toBeNull();
		const change = document.querySelector('[data-testid="member-change"]')?.textContent ?? '';
		expect(change).toContain('change.caution Gnosis');
		expect(change).toContain('#2 · change.threshold');
		expect(change).toContain('change.rule 2/2 → 1/2');
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

describe('who can act for it on this network (§3.5)', () => {
	const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

	it('its genesis refused here: read-only — the reasons said, nothing to sign or withdraw', async () => {
		fake.checked = [{ from: 'genesis', chainId: 100, reason: { kind: 'module', address: MODULE } }];
		fake.chain = {
			chainId: 100,
			members,
			items: [item({ canExecute: [local], approved: 1 })],
			mailbox: true
		};
		show();
		await settle();
		const notice = document.querySelector('[data-testid="authority-refused"]');
		expect(notice?.textContent).toContain('authority.refusedOn');
		expect(notice?.textContent).toContain('authority.module');
		expect(buttons()).not.toContain('proposal.approve');
		expect(buttons()).not.toContain('proposal.execute');
		expect(buttons()).not.toContain('proposal.approveAndExecute');
		expect(buttons()).not.toContain('proposal.reject');
		expect(buttons()).not.toContain('external.signWithWallet');
	});

	it('a risk found here: said on the sheet, and signing still offered (each signature asks)', async () => {
		fake.checked = [{ from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } }];
		show();
		await settle();
		expect(document.querySelector('[data-testid="authority-risk"]')?.textContent).toContain(
			'authority.riskModule'
		);
		expect(buttons()).toContain('proposal.approve');
		press('proposal.approve');
		await settle();
		expect(fake.calls).toEqual(['approve']);
	});

	it("another network's findings say nothing here", async () => {
		fake.checked = [
			{ from: 'genesis', chainId: 8453, reason: { kind: 'module', address: MODULE } }
		];
		show();
		await settle();
		expect(document.querySelector('[data-testid="authority-refused"]')).toBeNull();
		expect(document.querySelector('[data-testid="authority-risk"]')).toBeNull();
		expect(buttons()).toContain('proposal.approve');
	});
});

/**
 * r4a-1: a member change alone reads as fully as one inside a batch — who,
 * the rule before → after, the network, the caution — and one the Safe would
 * refuse says executing it would fail.
 */
describe('a member change', () => {
	const add = { kind: 'addOwner', owner: TO, threshold: '2' };
	const reading = (fits: boolean) => ({
		changes: [
			{
				call: null,
				kind: 'add',
				added: TO,
				removed: null,
				before: { threshold: 2, owners: 2 },
				after: { threshold: 2, owners: 3 },
				fits
			}
		],
		before: { threshold: 2, owners: 2 },
		after: { threshold: 2, owners: 3 },
		fits
	});

	it('says who, the rule before and after, the network, and the caution', async () => {
		fake.chain.items = [item({ review: { action: add, refusal: null } })];
		fake.memberReading = reading(true);
		const screen = show();
		await settle();
		const block = document.querySelector('[data-testid="member-change"]');
		expect(block?.querySelector('[role="alert"]')?.textContent?.trim()).toBe(
			'change.caution Gnosis'
		);
		expect(block?.textContent).toContain('change.add');
		// The whole address of whom it adds, beside its name.
		expect(block?.textContent).toContain(TO);
		expect(document.querySelector('[data-testid="member-change-rule"]')?.textContent?.trim()).toBe(
			'change.rule 2/2 → 2/3'
		);
		expect(block?.textContent).not.toContain('change.doesNotFit');
		await screen.unmount();
	});

	it('one that does not fit the members now says executing it would fail', async () => {
		fake.chain.items = [item({ review: { action: add, refusal: null } })];
		fake.memberReading = reading(false);
		const screen = show();
		await settle();
		expect(document.querySelector('[data-testid="member-change"]')?.textContent).toContain(
			'change.doesNotFit'
		);
		await screen.unmount();
	});
});

/** r4a-22: the whole proposal hash, to compare with another wallet's: shown and copied. */
describe('the proposal hash', () => {
	it('is there whole, behind its short form, with a copy', async () => {
		const written: string[] = [];
		Object.defineProperty(navigator, 'clipboard', {
			configurable: true,
			value: { writeText: async (text: string) => void written.push(text) }
		});
		const screen = show();
		await settle();
		const hash = document.querySelector('[data-testid="proposal-hash"]');
		expect(hash?.querySelector('summary')?.textContent).toContain('…');
		expect(hash?.querySelector('code')?.textContent).toBe(HASH);
		(document.querySelector('[data-testid="proposal-hash-copy"]') as HTMLButtonElement).click();
		await settle();
		expect(written).toEqual([HASH]);
		await screen.unmount();
	});
});

/**
 * r4a-8: an executed transaction reads the facts frozen when it ran — the
 * approvals that counted then, the threshold then, who sent it — never
 * today's owners re-counted, never "Waiting", never more than were needed.
 */
describe('an executed transaction', () => {
	const THEN = '0xcc000000000000000000000000000000000000cc';
	const done = () => item({ status: 'done', canApprove: [], approvals: [], approved: 0 });

	it('lists whose approvals counted then — a member removed since too — and who sent it', async () => {
		fake.chain.items = [done()];
		fake.executed = `0x${'ee'.repeat(32)}`;
		fake.frozen = {
			executor: ME,
			threshold: 2,
			approvers: [THEN, ME],
			executorApproved: true,
			source: 'chain'
		};
		const screen = show();
		await settle();
		const approvals = document.querySelector('section.approvals')?.textContent ?? '';
		// THEN is no member now (members are ME and OTHER): it approved it,
		// then — and OTHER, a member now, did not.
		expect(approvals).toContain(THEN.slice(0, 6));
		expect(document.querySelectorAll('section.approvals li')).toHaveLength(2);
		// The executor approved by sending it: counted once, as the executor.
		expect(approvals.match(/proposal\.executed/g)?.length).toBe(2);
		expect(approvals).not.toContain('status.progress');
		expect(approvals).not.toContain('proposal.waiting');
		expect(document.querySelector('[data-testid="approvals-count"]')?.textContent).toBe(
			'proposal.executed'
		);
		const facts = document.querySelector('[data-testid="executed-facts"]')?.textContent ?? '';
		expect(facts).toContain('proposal.executedBy');
		expect(facts).toContain('proposal.countedThen');
		await screen.unmount();
	});

	it('says so when which approvals counted is not on record', async () => {
		fake.chain.items = [done()];
		fake.executed = `0x${'ee'.repeat(32)}`;
		fake.frozen = null;
		const screen = show();
		await settle();
		expect(
			document.querySelector('[data-testid="executed-facts-unknown"]')?.textContent?.trim()
		).toBe('proposal.approvalsSigned');
		expect(document.body.textContent).not.toContain('status.progress');
		await screen.unmount();
	});
});

/** r4a-2: a transfer of a token nobody vouches for shows its contract on the sheet. */
describe('a transfer of an unverified token', () => {
	it('carries the note the core rules, with the contract', async () => {
		const FAKE = '0x57e64022d8d9a420dFE08a22d6a687caaBc6B7C6';
		fake.chain.items = [
			item({
				review: { action: { kind: 'transfer', token: FAKE, to: TO, amount: '1' }, refusal: null }
			})
		];
		fake.tokenNote = {
			text: 'Not the real USDC',
			tone: 'danger',
			contract: FAKE,
			short: '0x57e6…B7C6',
			explorerUrl: `https://explorer.example/address/${FAKE}`
		};
		const screen = show();
		await settle();
		const note = document.querySelector('[data-testid="token-provenance"]');
		expect(note?.getAttribute('data-tone')).toBe('danger');
		expect(note?.textContent).toContain('Not the real USDC');
		expect(note?.textContent).toContain('0x57e6…B7C6');
		expect(note?.querySelector('a')?.getAttribute('href')).toContain(FAKE);
		await screen.unmount();
	});
});
