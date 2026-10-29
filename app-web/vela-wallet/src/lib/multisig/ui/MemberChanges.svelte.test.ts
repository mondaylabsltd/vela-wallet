/**
 * Changing the members in Settings (r4D-3, r4a-15), with the real core:
 * Remove, Replace, Add and the approvals stepper each make ONE proposal per
 * network the multisig is deployed on — never one where it is not (that
 * would deploy it) — refuse an impossible state per network in its own
 * words before anything is signed, and the changes still waiting read as
 * one change with each network's progress ("Arc 1/2 · Base 0/2").
 *
 * The words are their keys (a stand-in catalog), so what is asserted is
 * WHICH sentence is drawn.
 */
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const C = '0x3333333333333333333333333333333333333333';
const D = '0x4444444444444444444444444444444444444444';
const ARC = 5042002;
const BASE = 8453;
const GNOSIS = 100;

const seams = vi.hoisted(() => ({
	chains: [] as unknown[],
	items: [] as unknown[],
	proposed: [] as [string, number, unknown][],
	opened: [] as unknown[],
	refuseOn: null as number | null,
	contacts: [] as unknown[],
	/** Members' labels kept with the multisig (create's store, r4D-5). */
	named: [] as [string, Record<string, unknown>][]
}));
vi.mock('../core/store.svelte', () => ({
	multisig: {
		nameMembers: (address: string, known: Record<string, unknown>) =>
			void seams.named.push([address, known])
	}
}));
vi.mock('../core/proposals.svelte', async () => {
	const { MemberChangeRefusedError } = await import('../core/member-changes');
	return {
		proposals: {
			get chains() {
				return seams.chains;
			},
			get items() {
				return seams.items;
			}
		},
		MailboxUnreadableError: class extends Error {},
		proposeMemberChange: async (address: string, chainId: number, change: unknown) => {
			if (chainId === seams.refuseOn) {
				throw new MemberChangeRefusedError({ kind: 'changeWaiting' });
			}
			seams.proposed.push([address, chainId, change]);
			return { safeTxHash: `0xhash${chainId}` };
		}
	};
});
vi.mock('../core/display', () => ({
	chainName: (id: number) => ({ [ARC]: 'Arc', [BASE]: 'Base', [GNOSIS]: 'Gnosis' })[id] ?? `${id}`
}));
vi.mock('../core/ui.svelte', () => ({
	multisigUi: { open: (sheet: unknown) => seams.opened.push(sheet) }
}));
vi.mock('../core/names.svelte', () => ({
	names: { label: (a: string) => `name-${a.slice(2, 4)}`, isVela: () => false }
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
	out.action.changeThreshold = 'Require {{threshold}}';
	out.members.changeAdd = 'Add {{name}}';
	(out.members.refuse as Record<string, string>).belowThreshold =
		'belowThreshold {{owners}} {{threshold}}';
	return out as unknown as MultisigMessages;
}

const members = (owners: string[], threshold: number, deployed = true) => ({
	deployed,
	owners,
	threshold,
	nonce: 0n
});

/** Arc and Base deployed with the same 2 of 3; Gnosis not deployed. */
function twoNetworks() {
	seams.chains = [
		{ chainId: ARC, members: members([A, B, C], 2), items: [], mailbox: true },
		{ chainId: BASE, members: members([A, B, C], 2), items: [], mailbox: true },
		{ chainId: GNOSIS, members: members([A, B, C], 2, false), items: [], mailbox: true }
	];
}

function shown() {
	return render(MemberChanges, {
		props: {
			m: words(),
			account: { address: SAFE, genesis: {}, owners: [A, B, C], threshold: 2 } as never,
			cancelLabel: 'Cancel'
		}
	});
}

const byText = (root: HTMLElement, text: string) =>
	[...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === text);

beforeAll(async () => {
	await loadCore();
});

afterEach(() => {
	seams.chains = [];
	seams.items = [];
	seams.proposed = [];
	seams.named = [];
	seams.opened = [];
	seams.refuseOn = null;
	seams.contacts = [];
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
		expect(seams.proposed[0][2]).toEqual({ kind: 'remove', owner: A, threshold: undefined });
		// Two networks: no single proposal to jump to; the change shows as waiting.
		expect(seams.opened).toEqual([]);
	});

	it('refuses removing below the approvals needed, and lowering them makes it possible', async () => {
		seams.chains = [{ chainId: ARC, members: members([A, B, C], 3), items: [], mailbox: true }];
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
		expect(seams.opened).toEqual([{ kind: 'proposal', chainId: ARC, safeTxHash: `0xhash${ARC}` }]);
	});

	it('replaces a member (the lost-passkey path): the new account shown in full, its name kept here', async () => {
		twoNetworks();
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
		byText(screen.container, 'send.propose')?.click();
		await vi.waitFor(() => expect(seams.proposed).toHaveLength(2));
		expect(seams.proposed[1][2]).toEqual({ kind: 'replace', oldOwner: C, newOwner: D });
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { name: 'Dana', kind: 'wallet' } });
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
		expect(seams.proposed[0][2]).toEqual({ kind: 'add', owner: D, threshold: undefined });
		expect(seams.named.at(-1)?.[1]).toEqual({ [D]: { name: 'Dana', kind: 'wallet' } });
	});

	it('offers nothing when it is deployed nowhere', async () => {
		seams.chains = [
			{ chainId: GNOSIS, members: members([A, B], 2, false), items: [], mailbox: true }
		];
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
