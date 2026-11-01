/**
 * Changing the members in Settings (r4D-3, r4a-15), with the real core —
 * its members machine on the host, over a simulated world of chains (each
 * network's Safe, and what each member's address is):
 * Remove, Replace, Add and the approvals stepper each make ONE proposal per
 * network the multisig is deployed on — never one where it is not (that
 * would deploy it) — refuse an impossible state per network in its own
 * words before anything is signed, and the changes still waiting read as
 * one change with each network's progress ("Arc 1/2 · Base 0/2").
 *
 * The words are their keys (a stand-in catalog), so what is asserted is
 * WHICH sentence is drawn.
 */
import { tick as svelteTick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';
import type { WorldSafe } from '../shell/members.testing';

const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const C = '0x3333333333333333333333333333333333333333';
const D = '0x4444444444444444444444444444444444444444';
const ARC = 5042002;
const BASE = 8453;
const GNOSIS = 100;

const seams = vi.hoisted(() => ({
	/** Each network's Safe, as its chain answers. */
	chains: {} as Record<number, WorldSafe>,
	items: [] as unknown[],
	proposed: [] as [string, number, unknown][],
	opened: [] as unknown[],
	refuseOn: null as number | null,
	contacts: [] as unknown[],
	/** Members' labels kept with the multisig (the accounts machine's `name_members`, r4D-5). */
	named: [] as [string, Record<string, unknown>][],
	/** r7: what the chains say of each address added (unlisted: nobody answers). */
	evidence: {} as Record<string, 'vela' | 'contract' | 'wallet' | null>
}));
vi.mock('../core/store.svelte', () => ({
	MULTISIG_ACCOUNTS_KEY: 'vela.multisigAccounts',
	ACTIVE_MULTISIG_KEY: 'vela.activeMultisig',
	multisig: {
		accounts: [],
		attach: () => {},
		find: (address: string) => ({ address }),
		/** The members machine's notice, routed to the accounts machine. */
		send: (event: {
			type: string;
			address: string;
			update?: { kind: string; members?: Record<string, unknown> };
		}) => {
			if (event.type === 'apply' && event.update?.kind === 'name_members')
				seams.named.push([event.address, event.update.members ?? {}]);
		}
	}
}));
vi.mock('../core/proposals.svelte', () => ({
	proposals: {
		get chains() {
			return seams.chains;
		},
		get items() {
			return seams.items;
		}
	}
}));
// What the proposals machine says of a proposal that did not go, as words (`proposal-failure.ts`).
vi.mock('../shell/proposal-failure', () => ({
	memberRefusal: (reason: string) => {
		try {
			return JSON.parse(reason) as { kind: string };
		} catch {
			return null;
		}
	},
	actionError: () => null
}));
// The plan row's proposal goes to the proposals machine as it is (its `draft`).
vi.mock('../shell/proposals-host.svelte', () => {
	class ProposeFailedError extends Error {
		constructor(
			readonly state: unknown,
			readonly chainId: number
		) {
			super('not proposed');
		}
	}
	return {
		ProposeFailedError,
		proposalsHost: {
			detach: () => {},
			propose: async (
				account: { address: string },
				chainId: number,
				proposal: { change: unknown }
			) => {
				if (chainId === seams.refuseOn) {
					throw new ProposeFailedError(
						{ kind: 'refused', reason: JSON.stringify({ kind: 'changeWaiting' }) },
						chainId
					);
				}
				seams.proposed.push([account.address, chainId, proposal.change]);
				return { safeTxHash: `0xhash${chainId}` };
			}
		}
	};
});
// The wallet's network order, as these tests list it: Arc, Base, Gnosis.
vi.mock('$lib/services/networks', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/networks')>()),
	networkRank: (id: number) => [ARC, BASE, GNOSIS].indexOf(id)
}));
vi.mock('../core/display', () => ({
	chainName: (id: number) => ({ [ARC]: 'Arc', [BASE]: 'Base', [GNOSIS]: 'Gnosis' })[id] ?? `${id}`,
	shortenAddress: (a: string) => `${a.slice(0, 8)}…${a.slice(-6)}`
}));
vi.mock('../core/ui.svelte', () => ({
	multisigUi: { open: (sheet: unknown) => seams.opened.push(sheet) }
}));
vi.mock('../core/names.svelte', () => ({
	names: { label: (a: string) => `name-${a.slice(2, 4)}`, isVela: () => false },
	keptMember: () => ({})
}));
vi.mock('$lib/onboarding/core/storage', async (original) => ({
	...(await original<object>()),
	loadAccounts: () => []
}));
vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({
	contactsBook: {
		get view() {
			return { contacts: seams.contacts };
		}
	}
}));
vi.mock('$lib/wallet/identicon', () => ({
	identiconSvgForClient: (a: string) => `<svg data-seed="${a}"></svg>`
}));

const { default: MemberChanges } = await import('./MemberChanges.svelte');
const { machines } = await import('../shell/machines.svelte');
const { startMembers, membersSettled, worldMultisig } = await import('../shell/members.testing');

/** The machine's reads answered, and what they changed drawn. */
async function tick() {
	await membersSettled();
	await svelteTick();
}

function words(): MultisigMessages {
	const walk = (node: Record<string, unknown>, prefix: string): Record<string, unknown> =>
		Object.fromEntries(
			Object.entries(node).map(([key, value]) => [
				key,
				typeof value === 'string' || 'forms' in (value as object)
					? `${prefix}${key}`
					: walk(value as Record<string, unknown>, `${prefix}${key}.`)
			])
		);
	const out = walk(MULTISIG_SHAPE as unknown as Record<string, unknown>, '') as Record<
		string,
		Record<string, unknown>
	>;
	out.members.ruleValue = '{{m}} of {{n}}';
	out.members.onNetwork = '{{network}}: {{before}} → {{after}}';
	out.members.failedOn = 'failedOn {{network}}: {{reason}}';
	out.settings.undeployedOn = 'undeployedOn {{network}}';
	out.members.recovery = 'recovery {{m}}/{{n}}';
	out.action.changeThreshold = {
		locale: 'en',
		forms: { _one: 'Require {{count}}', _other: 'Require {{count}}' }
	};
	out.members.changeAdd = 'Add {{name}}';
	out.members.changeReplace = 'Replace {{old}} with {{new}}';
	(out.members.refuse as Record<string, string>).belowThreshold =
		'belowThreshold {{owners}} {{threshold}}';
	return out as unknown as MultisigMessages;
}

const members = (owners: string[], threshold: number, deployed = true): WorldSafe => ({
	deployed,
	owners,
	threshold
});

/** The multisig: founded by A, B and C, 2 of 3 (a real genesis). */
let SAFE = '';
let GENESIS: unknown = null;

/** Arc and Base deployed with the same 2 of 3; Gnosis not deployed. */
function twoNetworks() {
	seams.chains = {
		[ARC]: members([A, B, C], 2),
		[BASE]: members([A, B, C], 2),
		[GNOSIS]: members([A, B, C], 2, false)
	};
}

const world = () => ({
	safe: SAFE,
	chains: seams.chains,
	signers: seams.evidence,
	asked: [] as { chainId: number; method: string; params: unknown }[]
});
let asked: ReturnType<typeof world>['asked'] = [];

function shown() {
	const w = world();
	asked = w.asked;
	startMembers(w);
	const screen = render(MemberChanges, {
		props: {
			m: words(),
			account: {
				address: SAFE,
				genesis: GENESIS,
				owners: [A, B, C],
				threshold: 2,
				chainIds: Object.keys(seams.chains).map(Number),
				recorded: true
			} as never,
			cancelLabel: 'Cancel'
		}
	});
	// What the proposals machine's view says waits, as the one host feeds it.
	machines.membersWaiting(seams.items as never);
	return screen;
}

const byText = (root: HTMLElement, text: string) =>
	[...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === text);

beforeAll(async () => {
	await loadCore();
	const made = worldMultisig([A, B, C], 2);
	SAFE = made.address;
	GENESIS = made.genesis;
});

afterEach(() => {
	machines.resetForTests();
	seams.chains = {};
	seams.items = [];
	seams.proposed = [];
	seams.named = [];
	seams.opened = [];
	seams.refuseOn = null;
	seams.contacts = [];
	seams.evidence = {};
});

describe('changing the members (Settings)', () => {
	it('removes a member with one proposal per deployed network, none where it is not deployed', async () => {
		twoNetworks();
		const screen = shown();
		await tick();
		const remove = screen.container.querySelector<HTMLButtonElement>(
			`button[aria-label="members.removeLabel"]`
		);
		// The rows: every deployed network's members, each with Replace and Remove.
		expect(screen.container.querySelectorAll('.rows li')).toHaveLength(3);
		remove?.click();
		await tick();
		// 2 of 3 minus one: the rule still fits (2 of 2) on each network.
		const plan = screen.container.querySelector('[data-testid="member-plan"]');
		expect(plan?.querySelector(`[data-testid="plan-${ARC}"]`)?.textContent).toContain(
			'Arc: 2 of 3 → 2 of 2'
		);
		expect(plan?.querySelector(`[data-testid="plan-${BASE}"]`)?.textContent).toContain(
			'Base: 2 of 3 → 2 of 2'
		);
		expect(plan?.querySelector(`[data-testid="plan-undeployed-${GNOSIS}"]`)?.textContent).toBe(
			'undeployedOn Gnosis'
		);
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.proposed.map(([, chainId]) => chainId)).toEqual([ARC, BASE]);
		expect(seams.proposed[0][2]).toEqual({ kind: 'remove', owner: A, threshold: null });
		// The new member's name and kind are kept only for Add and Replace.
		expect(seams.named).toEqual([]);
		// Two networks: no single proposal to jump to; the change shows as waiting.
		expect(seams.opened).toEqual([]);
	});

	it('refuses removing below the approvals needed, and lowering them makes it possible', async () => {
		seams.chains = { [ARC]: members([A, B, C], 3) };
		const screen = shown();
		await tick();
		screen.container
			.querySelectorAll<HTMLButtonElement>(`button[aria-label="members.removeLabel"]`)[1]
			.click();
		await tick();
		const arc = () =>
			screen.container.querySelector(`[data-testid="plan-${ARC}"]`)?.textContent ?? '';
		expect(arc()).toContain('belowThreshold 2 3');
		expect(byText(screen.container, 'send.propose')?.disabled).toBe(true);
		// The stepper under it: one fewer approval.
		screen.container
			.querySelector<HTMLButtonElement>(
				'[data-testid="member-compose"] button[aria-label="create.fewer"]'
			)
			?.click();
		await tick();
		expect(arc()).toContain('Arc: 3 of 3 → 2 of 2');
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(1));
		expect(seams.proposed[0][2]).toEqual({ kind: 'remove', owner: B, threshold: 2 });
		// One network: straight to its proposal.
		await vi.waitFor(() =>
			expect(seams.opened).toEqual([{ kind: 'proposal', chainId: ARC, safeTxHash: `0xhash${ARC}` }])
		);
	});

	it('replaces a member (the lost-passkey path): the new account shown in full, its name kept here', async () => {
		twoNetworks();
		// r7: no Vela account found at D — "Other wallet" only hinted: not picked, no kind kept.
		seams.evidence[D] = 'wallet';
		const screen = shown();
		await tick();
		screen.container
			.querySelectorAll<HTMLButtonElement>(`button[aria-label="members.replaceLabel"]`)[2]
			.click();
		await tick();
		const input = screen.container.querySelector<HTMLInputElement>('[data-testid="member-entry"]');
		if (!input) throw new Error('no entry');
		// The multisig itself is no member: refused for every network.
		input.value = SAFE;
		input.dispatchEvent(new Event('input', { bubbles: true }));
		input.dispatchEvent(new Event('blur'));
		await tick();
		expect(screen.container.querySelector(`[data-testid="plan-${ARC}"]`)?.textContent).toContain(
			'members.refuse.theMultisig'
		);
		input.value = D;
		input.dispatchEvent(new Event('input', { bubbles: true }));
		input.dispatchEvent(new Event('blur'));
		await tick();
		expect(
			screen.container.querySelector('[data-testid="member-resolved"]')?.textContent
		).toContain(D);
		const name = screen.container.querySelector<HTMLInputElement>('[data-testid="member-name"]');
		if (!name) throw new Error('no name field');
		name.value = 'Dana';
		name.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		// Round 5: the preview names who replaces whom, by the name just typed.
		expect(
			screen.container.querySelector('[data-testid="member-replace-preview"]')?.textContent?.trim()
		).toBe('Replace name-33 with Dana');
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.proposed[1][2]).toEqual({ kind: 'replace', oldOwner: C, newOwner: D });
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { name: 'Dana' } });
	});

	it('changes the approvals needed with the stepper, and says a network that refuses', async () => {
		twoNetworks();
		seams.refuseOn = BASE;
		const screen = shown();
		await tick();
		expect(screen.container.querySelector('[data-testid="recovery"]')?.textContent).toContain(
			'recovery 2/3'
		);
		screen.container.querySelector<HTMLButtonElement>('button[aria-label="create.more"]')?.click();
		await tick();
		expect(screen.container.querySelector('[data-testid="rule-value"]')?.textContent).toBe(
			'3 of 3'
		);
		expect(screen.container.querySelector(`[data-testid="plan-${ARC}"]`)?.textContent).toContain(
			'Arc: 2 of 3 → 3 of 3'
		);
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() =>
			expect(screen.container.querySelector('[role="alert"]')?.textContent).toBe(
				'failedOn Base: members.refuse.changeWaiting'
			)
		);
		expect(seams.proposed).toEqual([[SAFE, ARC, { kind: 'threshold', threshold: 3 }]]);
	});

	it('adds a member picked from the contacts: its address shown, its name and kind kept', async () => {
		twoNetworks();
		seams.evidence[D] = 'wallet';
		seams.contacts = [
			{ address: D, name: 'Dana', resolved_name: null, kind: 'eoa' },
			{ address: '0x5555555555555555555555555555555555555555', name: 'Eve', kind: 'eoa' }
		];
		const screen = shown();
		await tick();
		byText(screen.container, 'members.add')?.click();
		await tick();
		const input = screen.container.querySelector<HTMLInputElement>('[data-testid="member-entry"]');
		if (!input) throw new Error('no entry');
		input.value = 'dan';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		// A name searched is no wrong address: no error while it matches a contact.
		input.dispatchEvent(new Event('blur'));
		await tick();
		expect(screen.container.querySelector('[role="alert"]')).toBeNull();
		const suggestions = screen.container.querySelectorAll<HTMLButtonElement>('.suggestions button');
		expect(suggestions).toHaveLength(1);
		suggestions[0].click();
		await tick();
		expect(
			screen.container.querySelector('[data-testid="member-resolved"]')?.textContent
		).toContain(D);
		expect(
			screen.container.querySelector<HTMLInputElement>('[data-testid="member-name"]')?.value
		).toBe('Dana');
		expect(screen.container.querySelector(`[data-testid="plan-${ARC}"]`)?.textContent).toContain(
			'Arc: 2 of 3 → 2 of 4'
		);
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.proposed[0][2]).toEqual({ kind: 'add', owner: D, threshold: null });
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { name: 'Dana' } });
	});

	it('offers nothing when it is deployed nowhere', async () => {
		seams.chains = { [GNOSIS]: members([A, B], 2, false) };
		const screen = shown();
		await tick();
		expect(
			screen.container.querySelector('[data-testid="members-none-deployed"]')?.textContent
		).toBe('members.noneDeployed');
		expect(byText(screen.container, 'members.add')).toBeUndefined();
		expect(screen.container.querySelectorAll('.rows li')).toHaveLength(0);
	});

	it('shows a change waiting on several networks as one, with each one’s progress', async () => {
		twoNetworks();
		const add = { kind: 'addOwner', owner: D, threshold: '2' };
		const item = (chainId: number, approved: number) => ({
			chainId,
			safeTxHash: `0x${chainId}`,
			approved,
			threshold: 2,
			status: 'pending',
			review: { safeTxHash: `0x${chainId}`, nonce: '0', action: add, refusal: null }
		});
		seams.items = [item(ARC, 1), item(BASE, 0)];
		const screen = shown();
		await tick();
		const group = screen.container.querySelector('[data-testid="member-group"]');
		expect(group?.getAttribute('data-progress')).toBe('Arc 1/2 · Base 0/2');
		expect(group?.textContent).toContain('Add name-44');
		[...(group?.querySelectorAll<HTMLButtonElement>('button.part') ?? [])][1].click();
		expect(seams.opened).toEqual([{ kind: 'proposal', chainId: BASE, safeTxHash: `0x${BASE}` }]);
	});
});

/**
 * r7: Add member asks the member's kind — "Vela account" or "Other wallet" —
 * pre-filled where the wallet can tell, asked where it can't, and kept with
 * the member as a label (never authority). It used to read "Other wallet"
 * whenever nothing said.
 */
describe('Add member: the kind, declared then verified (r7)', () => {
	async function typeMember(address: string) {
		twoNetworks();
		const screen = shown();
		await tick();
		byText(screen.container, 'members.add')?.click();
		await tick();
		const input = screen.container.querySelector<HTMLInputElement>('[data-testid="member-entry"]');
		if (!input) throw new Error('no entry');
		input.value = address;
		input.dispatchEvent(new Event('input', { bubbles: true }));
		input.dispatchEvent(new Event('blur'));
		await tick();
		const kind = () => screen.container.querySelector<HTMLElement>('[data-testid="member-kind"]');
		const radios = () =>
			[...screen.container.querySelectorAll<HTMLButtonElement>('[role="radio"]')].map((r) => [
				r.textContent?.trim(),
				r.getAttribute('aria-checked')
			]);
		return { screen, kind, radios };
	}

	it('it can’t tell: nothing picked, the person asked — and left unsaid, no kind is kept', async () => {
		const { screen, kind, radios } = await typeMember(D);
		expect(kind()?.dataset.kind).toBe('unknown');
		expect(radios()).toEqual([
			['members.kindVela', 'false'],
			['members.kindWallet', 'false']
		]);
		expect(screen.container.querySelector('[data-testid="member-kind-ask"]')?.textContent).toBe(
			'members.kindAsk'
		);
		// Looked for on the multisig's networks: its code on each.
		const codeOn = asked
			.filter((a) => a.method === 'eth_getCode' && (a.params as string[])[0] === D)
			.map((a) => a.chainId);
		expect([...new Set(codeOn)].sort()).toEqual([GNOSIS, BASE, ARC].sort());
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: {} });
	});

	it('asked, the person says a Vela account: kept as one', async () => {
		const { screen, kind } = await typeMember(D);
		byText(screen.container, 'members.kindVela')?.click();
		await tick();
		expect(kind()?.dataset.kind).toBe('passkey');
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { kind: 'passkey' } });
	});

	it('no Vela account found: "Other wallet" only hinted and said — not picked, nothing kept', async () => {
		seams.evidence[D] = 'wallet';
		const { screen, radios } = await typeMember(D);
		expect(radios()).toEqual([
			['members.kindVela', 'false'],
			['members.kindWallet', 'false']
		]);
		const hinted = [
			...screen.container.querySelectorAll<HTMLButtonElement>('[role="radio"][data-suggested]')
		].map((b) => b.textContent?.trim());
		expect(hinted).toEqual(['members.kindWallet']);
		expect(screen.container.querySelector('[data-testid="member-kind-likely"]')).not.toBeNull();
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		// Unsure: offered both ways later.
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: {} });
	});

	it('the hint, picked: kept as another wallet', async () => {
		seams.evidence[D] = 'wallet';
		const { screen, radios } = await typeMember(D);
		byText(screen.container, 'members.kindWallet')?.click();
		await tick();
		expect(radios()).toEqual([
			['members.kindVela', 'false'],
			['members.kindWallet', 'true']
		]);
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { kind: 'wallet' } });
	});

	it('another Safe (code, no index record): a smart contract account, said honestly, kept as one', async () => {
		seams.evidence[D] = 'contract';
		const { screen, kind, radios } = await typeMember(D);
		expect(kind()?.dataset.kind).toBe('contract');
		expect(radios()).toEqual([]);
		expect(kind()?.textContent).toContain('members.kindContract');
		expect(kind()?.textContent).toContain('members.kindContractNote');
		expect(kind()?.textContent).not.toContain('members.kindVela');
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { kind: 'contract' } });
	});

	it('evidence of a Vela account: shown as found, nothing to pick, kept as a Vela account', async () => {
		seams.evidence[D] = 'vela';
		const { screen, kind, radios } = await typeMember(D);
		expect(kind()?.dataset.kind).toBe('passkey');
		expect(radios()).toEqual([]);
		expect(kind()?.textContent).toContain('members.kindKnown');
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { kind: 'passkey' } });
	});
});
