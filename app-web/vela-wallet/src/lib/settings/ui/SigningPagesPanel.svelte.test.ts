/**
 * Settings → Signing pages (spec 102), in a real browser.
 *
 * Every row must say, before anyone trusts it, which keys the page can reach
 * and what was checked about it; the official page is first and cannot be
 * renamed or removed; a refused address says why; and what is typed is what
 * is handed on — the core, not this panel, judges it.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SigningPagesModel } from '../model';
import SigningPagesPanel from './SigningPagesPanel.svelte';

const PANEL: SigningPagesModel = {
	title: 'Signing pages',
	subtitle: 'Pages you trust to show and sign requests.',
	officialTag: 'Official',
	rows: [
		{
			url: 'https://sign.getvela.app/',
			name: 'Official',
			host: 'sign.getvela.app',
			keysOn: 'Keys on getvela.app',
			official: true,
			integrity: {
				text: "Version 0ba8ee8c · matches Vela's published build list · checked 14:32",
				tone: 'ok'
			}
		},
		{
			url: 'https://sign.example.com/',
			name: 'My page',
			host: 'sign.example.com',
			keysOn: 'Keys on sign.example.com',
			official: false,
			integrity: {
				text: "Version 7d41e0b9 isn't on Vela's published build list. Not opened.",
				tone: 'error'
			}
		}
	],
	add: {
		id: 'signing-page-add',
		label: 'Add a page',
		value: '',
		placeholder: 'https://',
		hint: 'That page is already saved.',
		tone: 'error'
	},
	addAction: 'Save',
	renameLabel: 'Rename',
	removeLabel: 'Remove'
};

function drawn() {
	const added: string[] = [];
	const renamed: string[] = [];
	const removed: string[] = [];
	const screen = render(SigningPagesPanel, {
		props: {
			panel: PANEL,
			onadd: (url: string) => added.push(url),
			onrename: (url: string) => renamed.push(url),
			onremove: (url: string) => removed.push(url)
		}
	});
	const rows = () => [...screen.container.querySelectorAll<HTMLElement>('li.page')];
	return { root: screen.container, rows, added, renamed, removed };
}

describe('Settings → Signing pages', () => {
	it('every row says where it lives and which keys it can reach', () => {
		const view = drawn();
		expect(view.rows().map((row) => row.querySelector('.where')?.textContent)).toEqual([
			'sign.getvela.app·Keys on getvela.app',
			'sign.example.com·Keys on sign.example.com'
		]);
	});

	it('the official page cannot be renamed or removed; a saved one can', () => {
		const view = drawn();
		const [official, own] = view.rows();
		expect(official.querySelectorAll('button')).toHaveLength(0);
		const [rename, remove] = [...own.querySelectorAll<HTMLButtonElement>('button')];
		rename.click();
		remove.click();
		expect(view.renamed).toEqual(['https://sign.example.com/']);
		expect(view.removed).toEqual(['https://sign.example.com/']);
	});

	it('a page that will not open says so in its own colour, with a warning mark', () => {
		const view = drawn();
		const refused = view.rows()[1].querySelector<HTMLElement>('.integrity');
		expect(refused?.dataset.tone).toBe('error');
		expect(refused?.textContent).toContain('Not opened.');
		const ok = view.rows()[0].querySelector<HTMLElement>('.integrity');
		expect(getComputedStyle(refused as Element).color).not.toBe(
			getComputedStyle(ok as Element).color
		);
	});

	it('hands on what was typed, and shows why the last one was refused', async () => {
		const view = drawn();
		expect(view.root.textContent).toContain('That page is already saved.');
		const input = view.root.querySelector<HTMLInputElement>('input');
		input!.value = 'https://pages.example.org/sign/';
		input!.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		[...view.root.querySelectorAll<HTMLButtonElement>('button')]
			.find((button) => button.textContent?.trim() === 'Save')
			?.click();
		expect(view.added).toEqual(['https://pages.example.org/sign/']);
	});
});
