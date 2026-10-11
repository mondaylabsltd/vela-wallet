/**
 * "Delete record" on a transaction's detail (device walk of 2026-10-11, item
 * 10).
 *
 * 「删除记录」 was the heaviest thing on the page: a full-width button filled
 * with the error colour, under an outlined "View on explorer". Deleting a
 * record is the one thing on this page nobody came to do. It is a quiet text
 * action now — destructive by its colour, not by its weight — below the useful
 * one, on the phone's sheet and the wide layout's column alike (both draw this
 * component), for a transfer and for a dApp's record.
 */
import '$lib/tokens/tokens.css';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { TxDetailModel } from '../model';
import TxDetail from './TxDetail.svelte';

function detail(partial: Partial<TxDetailModel> = {}): TxDetailModel {
	return {
		title: 'Sent BNB',
		status: { tone: 'success', label: 'Confirmed' },
		closeLabel: 'Close',
		amount: '−0.0003 BNB',
		fiat: '≈ $0.18',
		positive: false,
		facts: [{ label: 'To', value: '0x14fb…1d1e' }],
		viewOnExplorer: 'View on explorer',
		explorerUrl: 'https://bscscan.com/tx/0xabc',
		deleteLabel: 'Delete record',
		deleteQuiet: false,
		...partial
	} as TxDetailModel;
}

/** How heavy a control reads: what it is filled with, and how much room it takes. */
function weight(el: Element) {
	const style = getComputedStyle(el);
	const box = el.getBoundingClientRect();
	const [, , , alpha = '1'] = style.backgroundColor.match(/[\d.]+/g) ?? [];
	return {
		filled: style.backgroundColor !== 'rgba(0, 0, 0, 0)' && Number(alpha) > 0,
		bordered: parseFloat(style.borderTopWidth) > 0,
		width: box.width,
		top: box.top,
		color: style.color,
		fontSize: parseFloat(style.fontSize),
		fontWeight: Number(style.fontWeight)
	};
}

function drawn(model: TxDetailModel, width: number) {
	const host = document.createElement('div');
	host.style.width = `${width}px`;
	document.body.appendChild(host);
	const screen = render(TxDetail, { target: host, props: { model, ondelete: () => {} } });
	const remove = [...host.querySelectorAll('button')].find(
		(el) => el.textContent?.trim() === 'Delete record'
	)!;
	const explorer = host.querySelector('a')!;
	const result = { remove: weight(remove), explorer: weight(explorer), tag: remove.className };
	screen.unmount();
	host.remove();
	return result;
}

const token = (name: string) => {
	const probe = document.createElement('span');
	probe.style.color = `var(${name})`;
	document.body.appendChild(probe);
	const color = getComputedStyle(probe).color;
	probe.remove();
	return color;
};

/** The phone's sheet and the wide layout's column, as widths. */
const LAYOUTS = { phone: 358, wide: 400 };

describe('"Delete record" is not the heaviest thing on the page', () => {
	for (const [layout, width] of Object.entries(LAYOUTS)) {
		it(`${layout}: a text action — no fill, no border, no wider than its words`, () => {
			const { remove, explorer } = drawn(detail(), width);
			expect(remove.filled).toBe(false);
			expect(remove.bordered).toBe(false);
			// The useful control keeps the row; the delete takes its words' room.
			expect(explorer.width).toBeGreaterThan(width * 0.95);
			expect(remove.width).toBeLessThan(width * 0.6);
			// Smaller type than the control above it, and no bolder.
			expect(remove.fontSize).toBeLessThan(explorer.fontSize);
			expect(remove.fontWeight).toBeLessThanOrEqual(explorer.fontWeight);
		});

		it(`${layout}: below the useful one`, () => {
			const { remove, explorer } = drawn(detail(), width);
			expect(remove.top).toBeGreaterThan(explorer.top);
		});
	}

	it('says it is destructive by its colour, not by its weight', () => {
		expect(drawn(detail(), LAYOUTS.phone).remove.color).toBe(token('--color-error-base'));
	});

	it('on a record still pending it is quieter still: the record is the "don’t send it again" trace', () => {
		const pending = drawn(detail({ deleteQuiet: true }), LAYOUTS.phone).remove;
		expect(pending.filled).toBe(false);
		expect(pending.color).toBe(token('--color-fg-muted'));
	});

	it('a dApp’s record, and one with no explorer link, draw the same quiet action', () => {
		const dapp = drawn(
			detail({ title: 'Contract interaction on arcorbs.game3.app', amount: '', fiat: '' }),
			LAYOUTS.phone
		).remove;
		expect(dapp.filled).toBe(false);
		expect(dapp.color).toBe(token('--color-error-base'));

		const host = document.createElement('div');
		host.style.width = `${LAYOUTS.phone}px`;
		document.body.appendChild(host);
		const screen = render(TxDetail, {
			target: host,
			props: { model: detail({ explorerUrl: undefined }), ondelete: () => {} }
		});
		const only = [...host.querySelectorAll('button')].find(
			(el) => el.textContent?.trim() === 'Delete record'
		)!;
		expect(weight(only).filled).toBe(false);
		expect(host.querySelector('.button.danger')).toBeNull();
		screen.unmount();
		host.remove();
	});

	it('is still a button a finger can find: the control height, and it deletes', async () => {
		let deleted = 0;
		const host = document.createElement('div');
		host.style.width = `${LAYOUTS.phone}px`;
		document.body.appendChild(host);
		const screen = render(TxDetail, {
			target: host,
			props: { model: detail(), ondelete: () => (deleted += 1) }
		});
		const remove = [...host.querySelectorAll('button')].find(
			(el) => el.textContent?.trim() === 'Delete record'
		)!;
		const control = document.createElement('span');
		control.style.height = 'var(--size-control-md)';
		control.style.display = 'block';
		host.appendChild(control);
		expect(remove.getBoundingClientRect().height).toBeGreaterThanOrEqual(
			control.getBoundingClientRect().height
		);
		remove.click();
		expect(deleted).toBe(1);
		screen.unmount();
		host.remove();
	});
});
