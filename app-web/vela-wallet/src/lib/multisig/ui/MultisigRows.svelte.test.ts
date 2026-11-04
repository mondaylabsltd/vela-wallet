/* eslint-disable @typescript-eslint/no-explicit-any -- stand-ins for the core's rows */
/**
 * The switcher's multisig rows, in a browser (r11-5): each row draws its
 * rule chip ("Needs 2 of 4") as the core keeps it (`AccountRow.rule`, r9-14),
 * and says it is the last known where a chain said it in an earlier session
 * and none has this one (`from_chain && last_known`). The followed one wears
 * the proposals machine's live chip. Android, iOS and the desktop drew it;
 * the web drew none.
 */
import { tick } from 'svelte';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OTHER_SAFE = '0x00000000000000000000000000000000000000c5';
const A = '0x00000000000000000000000000000000000000aa';
const B = '0x00000000000000000000000000000000000000bb';
const C = '0x00000000000000000000000000000000000000cc';
const D = '0x00000000000000000000000000000000000000dd';

const fake = vi.hoisted(() => ({
	accounts: [] as any[],
	rows: {} as Record<string, any>,
	followed: null as string | null,
	live: null as any
}));

vi.mock('../core/store.svelte', () => ({
	multisig: {
		get accounts() {
			return fake.accounts;
		},
		get active() {
			return null;
		},
		row: (address: string) => fake.rows[address.toLowerCase()] ?? null,
		remove: () => {}
	}
}));
vi.mock('../core/presence.svelte', () => ({
	presence: { parts: () => [], refreshOthers: async () => {}, othersWaiting: false },
	waitingWords: () => ''
}));
vi.mock('../core/proposals.svelte', () => ({
	proposals: {
		get followed() {
			return fake.followed;
		}
	},
	membersAnywhere: (account: { owners: string[] }) => account.owners
}));
vi.mock('../shell/proposals-host.svelte', () => ({
	proposalsHost: {
		get view() {
			return fake.live ? { rule_chip: fake.live } : null;
		}
	}
}));
vi.mock('../core/owners', () => ({
	localOwners: (owners: string[]) => owners.filter((o) => o === A).map((address) => ({ address }))
}));

const { default: MultisigRows } = await import('./MultisigRows.svelte');
const { loadCore } = await import('$lib/core/client');
beforeAll(() => loadCore());

/** Every word is its own key; the rule's carry their numbers. */
function words(): MultisigMessages {
	const out: any = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const [key, value] of Object.entries(keys as Record<string, unknown>)) {
			out[group][key] =
				typeof value === 'object' && value !== null && !('forms' in value)
					? Object.fromEntries(Object.keys(value).map((leaf) => [leaf, `${group}.${key}.${leaf}`]))
					: typeof value === 'object' && value !== null
						? { locale: 'en', forms: { _one: `${group}.${key}`, _other: `${group}.${key}` } }
						: `${group}.${key}`;
		}
	}
	out.panel.chip = 'Needs {{m}} of {{n}}';
	out.panel.title = '{{m}} of {{n}} members approve';
	out.presence.ruleLastKnown = 'Last known';
	return out;
}

const account = (address: string, owners: string[], threshold: number) => ({
	address,
	genesis: { name: 'Treasury' },
	owners,
	threshold,
	chainIds: [100]
});
const rule = (m: number, n: number, from_chain: boolean, last_known: boolean) => ({
	chip: { kind: 'needs', rule: { m, n, solo: n === 1 } },
	from_chain,
	last_known
});
const rowOf = (address: string, r: unknown) => ({
	account: { address },
	name: null,
	shared: true,
	record_waiting: false,
	local_members: [A],
	high_risk_on: [],
	refused_on: [],
	checked_on: [],
	checking_on: [],
	invite_shared: false,
	invite_members: [],
	rule: r
});

const settle = async () => {
	for (let i = 0; i < 3; i += 1) {
		await tick();
		await new Promise((r) => setTimeout(r, 0));
	}
};

function show() {
	return render(MultisigRows, {
		props: {
			m: words(),
			removeLabel: 'Remove',
			cancelLabel: 'Cancel',
			onselect: () => {}
		}
	});
}

const chips = () =>
	[...document.querySelectorAll('[data-testid="row-rule"]')].map((c) =>
		(c.textContent ?? '').replace(/\s+/g, ' ').trim()
	);

beforeEach(() => {
	fake.accounts = [];
	fake.rows = {};
	fake.followed = null;
	fake.live = null;
});

describe('a multisig row in the switcher (r11-5)', () => {
	it('draws the rule the chains last said — not the founding one', async () => {
		// Founded 1 of 2; a chain since said 2 of 4, this session.
		fake.accounts = [account(SAFE, [A, B], 1)];
		fake.rows = { [SAFE.toLowerCase()]: rowOf(SAFE, rule(2, 4, true, false)) };
		const screen = show();
		await settle();
		expect(chips()).toEqual(['· Needs 2 of 4']);
		expect(document.querySelector('[data-testid="row-rule"]')?.getAttribute('title')).toBe(
			'2 of 4 members approve'
		);
		// r12 S4: a space before the "·" — never "e56405· Needs 2 of 4".
		const line = document.querySelector('[data-testid="row-rule"]')?.parentElement?.textContent;
		expect(line).toMatch(/[0-9a-fA-F] · Needs 2 of 4$/);
		await screen.unmount();
	});

	it('a rule a chain said in an earlier session, not read in this one: "Last known"', async () => {
		fake.accounts = [account(SAFE, [A, B, C, D], 2)];
		fake.rows = { [SAFE.toLowerCase()]: rowOf(SAFE, rule(2, 4, true, true)) };
		const screen = show();
		await settle();
		expect(chips()).toEqual(['· Needs 2 of 4 · Last known']);
		await screen.unmount();
	});

	it('the founding rule before any chain said one is not called "last known"', async () => {
		fake.accounts = [account(SAFE, [A, B, C, D], 2)];
		fake.rows = { [SAFE.toLowerCase()]: rowOf(SAFE, rule(2, 4, false, true)) };
		const screen = show();
		await settle();
		expect(chips()).toEqual(['· Needs 2 of 4']);
		await screen.unmount();
	});

	it('the followed multisig wears the live chip; every other its row’s', async () => {
		fake.accounts = [account(SAFE, [A, B, C, D], 2), account(OTHER_SAFE, [A, B], 1)];
		fake.rows = {
			[SAFE.toLowerCase()]: rowOf(SAFE, rule(2, 4, true, true)),
			[OTHER_SAFE.toLowerCase()]: rowOf(OTHER_SAFE, rule(1, 2, true, true))
		};
		fake.followed = SAFE;
		fake.live = { kind: 'needs', rule: { m: 3, n: 4, solo: false } };
		const screen = show();
		await settle();
		expect(chips()).toEqual(['· Needs 3 of 4', '· Needs 1 of 2 · Last known']);
		await screen.unmount();
	});
});
