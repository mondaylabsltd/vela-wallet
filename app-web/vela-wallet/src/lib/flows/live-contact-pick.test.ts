import { describe, expect, it } from 'vitest';
import type { Contact } from '$lib/core/generated/Contact';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { buildDesktopFlowState } from './fixtures';
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
