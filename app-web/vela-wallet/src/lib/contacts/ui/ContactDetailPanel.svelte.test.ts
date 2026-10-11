/**
 * The contact detail column a click opens (issues 334 and 310).
 *
 * 334: Edit was only at the column's foot and in the row's right-click menu;
 * nothing beside the name said it could be changed. 310: that foot was pinned
 * to the bottom of the column, a screen-tall gap below the content it acts on.
 *
 * A `.svelte.test.ts`: where a control lands is the browser's to answer.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { ContactDetailModel } from '../model';
import ContactDetailPanel from './ContactDetailPanel.svelte';

const ADDRESS = '0x2c1c9470e6a6fc6340c9e24670361fec4c347c23';

function model(name = 'hold on'): ContactDetailModel {
	return {
		contact: {
			name,
			addressDisplay: '0x2c1c94…347c23',
			addressFull: ADDRESS,
			identiconSvg: '<svg></svg>',
			groups: ['team-1']
		},
		chips: ['team-1'],
		addChipLabel: 'Move to group',
		actions: { send: 'Send' },
		address: {
			label: 'Address',
			lines: [ADDRESS.slice(0, 21), ADDRESS.slice(21)],
			full: ADDRESS,
			copyLabel: 'Copy address'
		},
		activityTitle: 'Recent activity',
		activityAction: 'All',
		activityLink: 'View all activity',
		rows: [],
		emptyActivity: 'No Transactions Yet',
		editLabel: 'Edit',
		deleteLabel: 'Delete contact'
	};
}

/** Drawn in a column as tall as a desktop window's third panel. */
async function drawn(detail = model(), width = '420px') {
	let edits = 0;
	let deletes = 0;
	const screen = render(ContactDetailPanel, {
		props: { detail, onedit: () => (edits += 1), ondelete: () => (deletes += 1) }
	});
	screen.container.style.width = width;
	screen.container.style.height = '1600px';
	await tick();
	const root = screen.container;
	const named = (label: string) =>
		[...root.querySelectorAll('button')].filter(
			(b) => b.getAttribute('aria-label') === label || b.textContent?.trim() === label
		);
	return {
		root,
		named,
		name: root.querySelector('.name') as HTMLElement,
		activity: root.querySelector('section.activity') as HTMLElement,
		edits: () => edits,
		deletes: () => deletes
	};
}

describe('ContactDetailPanel', () => {
	it('puts one Edit beside the name, and it opens the form (issue 334)', async () => {
		const { named, name, edits } = await drawn();
		const edit = named('Edit');
		expect(edit).toHaveLength(1);
		const pencil = edit[0].getBoundingClientRect();
		const label = name.getBoundingClientRect();
		// On the name's line, after it — not at the column's foot.
		expect(pencil.left).toBeGreaterThanOrEqual(label.right - 1);
		expect(pencil.top).toBeLessThan(label.bottom);
		expect(pencil.bottom).toBeGreaterThan(label.top);
		edit[0].click();
		expect(edits()).toBe(1);
	});

	it('keeps Delete right after the content in a tall column (issue 310)', async () => {
		const { named, activity, root, deletes } = await drawn();
		const remove = named('Delete contact');
		expect(remove).toHaveLength(1);
		const gap = remove[0].getBoundingClientRect().top - activity.getBoundingClientRect().bottom;
		// A hairline and its spacing — the old foot sat ~1400px further down.
		expect(gap).toBeGreaterThanOrEqual(0);
		expect(gap).toBeLessThan(100);
		expect(remove[0].getBoundingClientRect().bottom).toBeLessThan(
			root.getBoundingClientRect().top + 800
		);
		remove[0].click();
		expect(deletes()).toBe(1);
	});

	it('an unnamed contact gets the worded action instead, still one Edit (issue 191)', async () => {
		const { named, name, edits } = await drawn({
			...model('0x2c1c94…347c23'),
			nameAction: 'Edit'
		});
		const edit = named('Edit');
		expect(edit).toHaveLength(1);
		expect(edit[0].classList.contains('name-action')).toBe(true);
		// Directly under the name it stands in for.
		const gap = edit[0].getBoundingClientRect().top - name.getBoundingClientRect().bottom;
		expect(gap).toBeGreaterThanOrEqual(0);
		expect(gap).toBeLessThan(30);
		edit[0].click();
		expect(edits()).toBe(1);
	});

	it('offers Send alone, the width of the column (issue 479)', async () => {
		const { root } = await drawn();
		const actions = [...root.querySelectorAll('.actions button')] as HTMLElement[];
		expect(actions.map((b) => b.textContent?.trim())).toEqual(['Send']);
		const row = (root.querySelector('.actions') as HTMLElement).getBoundingClientRect();
		const send = actions[0].getBoundingClientRect();
		// One action takes the whole row; it is not a third of it with two gaps.
		expect(Math.abs(send.width - row.width)).toBeLessThan(1);
		// The address stays on the page, with its copy.
		expect(root.querySelector('button[aria-label="Copy address"]')).not.toBeNull();
	});

	it('keeps the pencil inside a narrow column when the name is long', async () => {
		const { named, root } = await drawn(model('x'.repeat(80)), '320px');
		const pencil = named('Edit')[0].getBoundingClientRect();
		expect(pencil.right).toBeLessThanOrEqual(root.getBoundingClientRect().right + 0.5);
		expect(pencil.width).toBeGreaterThan(0);
	});
});
