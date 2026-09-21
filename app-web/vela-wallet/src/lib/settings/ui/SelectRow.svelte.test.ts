/**
 * The picker row, in a browser (spec 023, extended by spec 068).
 *
 * The thing worth pinning here is a LAYOUT contract that three different
 * sheets depend on at once, and that no view-model test can see:
 *
 *   - the currency sheet's `caption` stays INLINE after the label (美元),
 *   - the speed sheet's `detail` takes a line of its OWN under the label,
 *   - and neither one displaces the right-aligned `note` or the checkmark.
 *
 * They are one row because the app should have one picker, but they are three
 * independent promises — and `detail` was added by widening this row, so a
 * regression would land on the currency and language sheets, which is exactly
 * where nobody would be looking.
 *
 * Checked at 320px, the narrowest phone this ships to.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SelectRowModel } from '../model';
import Dropdown from './Dropdown.svelte';
import SelectRow from './SelectRow.svelte';

/** The three speed options, as Settings builds them (en). */
const SPEED_ROWS: SelectRowModel[] = [
	{
		id: 'fast',
		label: 'Fast',
		detail: 'First to confirm, even when the network is busy',
		selected: true
	},
	{ id: 'standard', label: 'Standard', detail: 'Balanced for everyday transfers' },
	{ id: 'slow', label: 'Slow', detail: 'Lowest fee, if you can wait' }
];

const NARROW = 320;

async function drawRow(row: SelectRowModel, onselect?: (id: string) => void) {
	const screen = render(SelectRow, { props: { row, onselect } });
	screen.container.style.width = `${NARROW}px`;
	await tick();
	const button = screen.container.querySelector('button.select-row') as HTMLButtonElement;
	return {
		button,
		label: button.querySelector('.label') as HTMLElement,
		caption: button.querySelector('.caption') as HTMLElement | null,
		detail: button.querySelector('.detail') as HTMLElement | null,
		note: button.querySelector('.note') as HTMLElement | null,
		check: button.querySelector('.check') as HTMLElement | null
	};
}

describe('SelectRow', () => {
	it('puts the detail on its own line under the label', async () => {
		const { label, detail } = await drawRow(SPEED_ROWS[0]);
		expect(detail?.textContent).toBe('First to confirm, even when the network is busy');
		const [labelBox, detailBox] = [label.getBoundingClientRect(), detail!.getBoundingClientRect()];
		expect(detailBox.top).toBeGreaterThanOrEqual(labelBox.bottom);
		// Under the label, not indented away from it: the two are one block.
		expect(Math.round(detailBox.left)).toBe(Math.round(labelBox.left));
	});

	// The regression this row's shape is most likely to cause. `caption` is the
	// currency sheet's 美元 and has always sat BESIDE the label; `detail` was
	// added next to it in the DOM, so "still inline" has to be said out loud.
	it('keeps a caption inline after the label, the way the currency sheet needs', async () => {
		const { label, caption } = await drawRow({
			id: 'USD',
			label: 'USD',
			caption: '美元',
			glyph: '$',
			note: '$1,234.56'
		});
		const [labelBox, captionBox] = [
			label.getBoundingClientRect(),
			caption!.getBoundingClientRect()
		];
		expect(captionBox.left).toBeGreaterThan(labelBox.left);
		// Same line: their vertical spans overlap rather than stacking.
		expect(captionBox.top).toBeLessThan(labelBox.bottom);
	});

	it('leaves the note right-aligned and the check drawn when a detail is present', async () => {
		const { button, detail, note, check } = await drawRow({
			...SPEED_ROWS[0],
			note: '~12s'
		});
		expect(detail).not.toBeNull();
		expect(check).not.toBeNull();
		const [buttonBox, noteBox] = [button.getBoundingClientRect(), note!.getBoundingClientRect()];
		// The note sits in the row's right half, past its middle — which is what
		// "right-aligned" means once a two-line block is sharing the row.
		expect(noteBox.left).toBeGreaterThan(buttonBox.left + buttonBox.width / 2);
		expect(button.scrollWidth).toBeLessThanOrEqual(Math.ceil(buttonBox.width));
	});

	it('draws no second line for a row that has no advantage to state', async () => {
		const { detail } = await drawRow({ id: 'en', label: 'English', note: 'System' });
		expect(detail).toBeNull();
	});

	it('emits the row id, not its label', async () => {
		const picked: string[] = [];
		const { button } = await drawRow(SPEED_ROWS[2], (id) => picked.push(id));
		button.click();
		await tick();
		expect(picked).toEqual(['slow']);
	});
});

describe('Dropdown (the desktop popup the same rows fill)', () => {
	async function drawMenu() {
		const screen = render(Dropdown, {
			props: { value: 'Fast', label: 'Transaction speed', open: true, rows: SPEED_ROWS }
		});
		await tick();
		return {
			container: screen.container,
			options: [...screen.container.querySelectorAll<HTMLButtonElement>('button.select-row')]
		};
	}

	// The owner's screenshot was of THIS control, so the ruling has to land
	// here and not only on the phone sheet: three speed names, each with the
	// reason somebody would choose it.
	it('shows every option with both its name and its description', async () => {
		const { options } = await drawMenu();
		expect(options.map((o) => o.querySelector('.label')?.textContent)).toEqual([
			'Fast',
			'Standard',
			'Slow'
		]);
		expect(options.map((o) => o.querySelector('.detail')?.textContent)).toEqual([
			'First to confirm, even when the network is busy',
			'Balanced for everyday transfers',
			'Lowest fee, if you can wait'
		]);
	});

	// Every name is a speed — none of them is a price. "Economy" under a
	// heading that asks about speed was the defect; this is the guard against
	// it coming back through the desktop surface.
	it('names a speed in every option, never what it costs', async () => {
		const { options } = await drawMenu();
		for (const option of options) {
			expect(option.querySelector('.label')?.textContent).not.toMatch(/econom/i);
		}
	});

	// The menu is `width: max-content` capped at the row measure, and it now
	// has a sentence in it. Nothing it draws may end up outside its own edge —
	// the checkmark least of all, since it is the only mark saying which speed
	// is in force.
	it('keeps every option inside the menu it is drawn in', async () => {
		const { container, options } = await drawMenu();
		const menu = container.querySelector('.menu') as HTMLElement;
		const measure = Number.parseFloat(
			getComputedStyle(document.documentElement).getPropertyValue('--layout-rowMeasure')
		);
		expect(menu.getBoundingClientRect().width).toBeLessThanOrEqual(measure);
		for (const option of options) {
			const box = option.getBoundingClientRect();
			for (const child of option.querySelectorAll('.label, .caption, .detail, .note, .check')) {
				const childBox = child.getBoundingClientRect();
				expect(childBox.left, `${option.textContent} / ${child.className}`).toBeGreaterThanOrEqual(
					box.left - 1
				);
				expect(childBox.right, `${option.textContent} / ${child.className}`).toBeLessThanOrEqual(
					box.right + 1
				);
			}
		}
	});
});
