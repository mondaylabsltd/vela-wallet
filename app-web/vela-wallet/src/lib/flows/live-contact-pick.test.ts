import { describe, expect, it } from 'vitest';
import type { Contact } from '$lib/core/generated/Contact';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildDesktopFlowState, buildFlowState } from './fixtures';
import { withLiveDesktopFlow, withLiveFlow, type FlowsLiveInputs } from './live';
import { liveContactPick } from './live-contact-pick';

const m = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;

function contact(address: string, name: string | null): Contact {
	return {
		address,
		name,
		resolved_name: null,
		resolved_source: null,
		kind: 'unknown',
		favorite: false,
		note: null,
		tx_count: 0,
		last_used_ms: 0,
		first_seen_ms: 0,
		source: 'manual'
	};
}

const ALICE = contact('0x' + 'a1'.repeat(20), 'Alice');
const RECENT = contact('0x' + 'c1'.repeat(20), null);

const VIEW: ContactsView = {
	loaded: true,
	contacts: [ALICE, RECENT],
	sections: [
		{ letter: 'A', addresses: [ALICE.address] },
		{ letter: '#', addresses: [RECENT.address] }
	],
	groups: [{ id: 'g1', name: 'Payroll', color: null, members: [ALICE] }],
	last_import: null,
	import_failure: null,
	import_failure_key: null,
	export: null,
	recipient: null
};

describe('liveContactPick', () => {
	it('replaces the drawn people with the book, groups first', () => {
		const drawn = buildDesktopFlowState('dsd2e', m, identicon);
		if (drawn.body.kind !== 'contact-pick') throw new Error('dsd2e is the picker');
		const live = liveContactPick(drawn.body.model, { view: VIEW, m, identicon });
		expect(live.title).toBe(drawn.body.model.title);
		expect(live.groups.map((g) => g.name)).toEqual(['Payroll']);
		expect(live.groups[0].count).toContain('1');
		expect(live.contacts.map((c) => [c.name, c.group])).toEqual([
			['Alice', 'Payroll'],
			// An unsaved recent recipient introduces itself by its short address.
			[expect.stringMatching(/^0xc1c1/i), undefined]
		]);
		expect(live.contacts[0].identiconSvg).toContain(ALICE.address);
	});
});

/**
 * A row answers with its ADDRESS (issue 467). While the book has no view yet
 * the live picker used to keep the gallery's three people — and their full,
 * real-looking addresses — so a tap put a stranger's address into a real
 * send. Until the book is read the live picker lists nobody.
 */
describe('the live picker before the book is read', () => {
	const BALANCE = {
		address: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
		display_total_usd: 0,
		balance_unknown: false,
		balance_partial: false,
		unreachable: false,
		notice: null,
		hidden: false,
		refreshing: false,
		last_refreshed_at_ms: null,
		tokens: [],
		unpriced_tokens: [],
		failed_chain_ids: [],
		rate_limited_chain_ids: [],
		unreachable_networks: [],
		unreachable_key: null,
		internal_chain_ids: [],
		internal_key: null,
		checking_key: null,
		live_key: null,
		empty_key: null,
		holdings_loading: false,
		cached_total_usd: null,
		switcher: { open: false, loading: false, balances: [], hidden: false }
	} satisfies BalanceView;
	/** A live page with a send open and no book: `contactPick` absent. */
	const unread: FlowsLiveInputs = {
		balance: BALANCE,
		currency: { code: 'USD', rate: 1, committed: true, pending: null },
		m: resolveWalletMessages('en'),
		emptyCopy: undefined,
		feed: null,
		fm: m
	};

	it('the phone sheet draws none of the board’s people', () => {
		const drawn = buildFlowState('sd2e', m, identicon);
		if (drawn.sheet?.kind !== 'contact-pick') throw new Error('sd2e raises the picker');
		const fixtures = drawn.sheet.model.contacts.map((c) => c.addressFull);
		expect(fixtures.length).toBeGreaterThan(0);

		const live = withLiveFlow(drawn, unread);
		if (live.sheet?.kind !== 'contact-pick') throw new Error('still the picker');
		expect(live.sheet.model.contacts).toEqual([]);
		expect(live.sheet.model.groups).toEqual([]);
		for (const address of fixtures) {
			expect(JSON.stringify(live.sheet)).not.toContain(address);
		}
		// Search stays: it does not wait for the book. Scanning is not in this
		// sheet at all (issue 471) — each recipient row has its own scan icon.
		expect(live.sheet.model.searchPlaceholder).toBe(drawn.sheet.model.searchPlaceholder);
		expect(live.sheet.model).not.toHaveProperty('scanRow');
	});

	it('the wide column draws none of them either', () => {
		const drawn = buildDesktopFlowState('dsd2e', m, identicon);
		if (drawn.body.kind !== 'contact-pick') throw new Error('dsd2e is the picker');
		const fixtures = drawn.body.model.contacts.map((c) => c.addressFull);
		const live = withLiveDesktopFlow(drawn, unread);
		if (live.body.kind !== 'contact-pick') throw new Error('still the picker');
		expect(live.body.model.contacts).toEqual([]);
		for (const address of fixtures) expect(JSON.stringify(live.body)).not.toContain(address);
	});

	it('the book, once read, is what it lists', () => {
		const live = withLiveFlow(buildFlowState('sd2e', m, identicon), {
			...unread,
			contactPick: { view: VIEW, m, identicon }
		});
		if (live.sheet?.kind !== 'contact-pick') throw new Error('still the picker');
		expect(live.sheet.model.contacts.map((c) => c.addressFull)).toEqual([
			ALICE.address,
			RECENT.address
		]);
	});
});
