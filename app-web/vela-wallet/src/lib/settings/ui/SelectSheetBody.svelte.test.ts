/**
 * The picker sheet's search field, in a browser (spec 072): the currency
 * sheet drew one that filtered nothing. It narrows the rows it is drawn over,
 * by code, label or caption; a sheet without one draws none.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SelectSheetModel } from '../model';
import SelectSheetBody from './SelectSheetBody.svelte';

const CURRENCIES: SelectSheetModel = {
	title: 'Currency',
	searchPlaceholder: 'Search currency',
	rows: [
		{ id: 'USD', label: 'USD', glyph: '$', caption: 'US Dollar', selected: true },
		{ id: 'EUR', label: 'EUR', glyph: '€', caption: 'Euro' },
		{ id: 'JPY', label: 'JPY', glyph: '¥', caption: 'Japanese Yen' },
		{ id: 'CNY', label: 'CNY', glyph: '¥', caption: 'Chinese Yuan' }
	]
};

async function drawn(sheet: SelectSheetModel) {
	const picked: string[] = [];
	const screen = render(SelectSheetBody, {
		props: { sheet, onselect: (id: string) => picked.push(id) }
	});
	await tick();
	const input = () => screen.container.querySelector<HTMLInputElement>('input[type="search"]');
	return {
		picked,
		input,
		async type(text: string) {
			const field = input() as HTMLInputElement;
			field.value = text;
			field.dispatchEvent(new Event('input', { bubbles: true }));
			await tick();
		},
		options: () => [...screen.container.querySelectorAll<HTMLElement>('[role="option"]')]
	};
}

describe('the currency search', () => {
	it('narrows the list by code, and by the name in the caption', async () => {
		const view = await drawn(CURRENCIES);
		expect(view.options()).toHaveLength(4);

		await view.type('eur');
		expect(view.options().map((row) => row.textContent)).toEqual([expect.stringContaining('EUR')]);

		await view.type('yen');
		expect(view.options().map((row) => row.textContent)).toEqual([expect.stringContaining('JPY')]);

		await view.type('  ');
		expect(view.options()).toHaveLength(4);
	});

	it('a narrowed row is still the row: picking it picks it', async () => {
		const view = await drawn(CURRENCIES);
		await view.type('yuan');
		view.options()[0]?.click();
		expect(view.picked).toEqual(['CNY']);
	});

	it('a sheet with no search draws none', async () => {
		const view = await drawn({ ...CURRENCIES, searchPlaceholder: undefined });
		expect(view.input()).toBeNull();
		expect(view.options()).toHaveLength(4);
	});
});
