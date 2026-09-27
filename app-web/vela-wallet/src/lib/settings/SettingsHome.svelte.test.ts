/**
 * The phone settings layout, in a browser (spec 072): removing a custom
 * network asks first, naming it — the bin removed it on one tap — and the
 * questions and opening events it already had still hold now that the wide
 * layout shares them.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { words } from './__fixtures__/words';
import { buildMobileState } from './fixtures';
import type { MobileSettingsStateId } from './model';
import type { SettingsNetEvent } from './net-events';
import SettingsHome from './SettingsHome.svelte';

const m = words();
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;

async function drawn(state: MobileSettingsStateId) {
	const net: SettingsNetEvent[] = [];
	const cleared: string[] = [];
	const screen = render(SettingsHome, {
		props: {
			model: buildMobileState(state, m, IDENTICON),
			onnetevent: (event: SettingsNetEvent) => net.push(event),
			onstorageclear: (id: string) => cleared.push(id)
		}
	});
	await tick();
	const root = screen.container;
	const sheet = () => root.querySelector<HTMLElement>('[role="dialog"]');
	const buttonIn = (scope: Element, text: string) =>
		[...scope.querySelectorAll('button')].find((b) => b.textContent?.trim() === text);
	return {
		net,
		cleared,
		root,
		sheet,
		buttonIn,
		async click(element: HTMLElement | null | undefined) {
			expect(element, 'the control is drawn').toBeTruthy();
			element?.click();
			await tick();
		}
	};
}

describe('removing a custom network', () => {
	it('asks first, titled with the network, and removes only on confirm', async () => {
		const view = await drawn('st9');
		await view.click(
			view.root.querySelector<HTMLElement>(`button[aria-label="${String(m.networks.remove)}"]`)
		);
		expect(view.net).toEqual([]);
		expect(view.sheet()?.getAttribute('aria-label')).toBe('X Layer');
		expect(view.sheet()?.textContent).toContain(String(m.networks.removeBody));

		await view.click(view.buttonIn(view.sheet() as HTMLElement, String(m.networks.removeConfirm)));
		expect(view.net).toEqual([{ kind: 'delete-network', id: 'xlayer' }]);
		expect(view.sheet()).toBeNull();
	});

	it('cancelled, removes nothing', async () => {
		const view = await drawn('st9');
		await view.click(
			view.root.querySelector<HTMLElement>(`button[aria-label="${String(m.networks.remove)}"]`)
		);
		await view.click(view.buttonIn(view.sheet() as HTMLElement, String(m.networks.removeCancel)));
		expect(view.net).toEqual([]);
		expect(view.sheet()).toBeNull();
	});
});

describe('what the phone already did', () => {
	it('opening the providers page raises providers-open', async () => {
		const view = await drawn('st1b');
		const row = [...view.root.querySelectorAll('button')].find((b) =>
			b.textContent?.includes(String(m.advanced.rpcProvidersTitle))
		);
		await view.click(row);
		expect(view.net).toEqual([{ kind: 'providers-open' }]);
	});

	it('a storage row asks first, naming itself', async () => {
		const view = await drawn('st13');
		const row = [...view.root.querySelectorAll('li')].find((li) =>
			li.textContent?.includes(String(m.storage.itemContacts))
		);
		await view.click(row?.querySelector('button'));
		expect(view.cleared).toEqual([]);
		expect(view.sheet()?.getAttribute('aria-label')).toBe(String(m.storage.itemContacts));
		await view.click(view.buttonIn(view.sheet() as HTMLElement, String(m.storage.clear)));
		expect(view.cleared).toEqual(['contacts']);
	});
});

// 078: Advanced → Community → About, Send feedback — the same on all four shells.
describe('Community and the report, on the phone', () => {
	it('lists the three official accounts as links that leave in a new tab', async () => {
		const view = await drawn('st1');
		const links = [...view.root.querySelectorAll<HTMLAnchorElement>('.scroll a.row')];
		expect(links.map((a) => [a.getAttribute('href'), a.target, a.rel])).toEqual([
			['https://x.com/realvelawallet', '_blank', 'noopener noreferrer'],
			['https://t.me/velawallet', '_blank', 'noopener noreferrer'],
			['https://discord.gg/23gWrtaYSa', '_blank', 'noopener noreferrer']
		]);
		// Each reads as its brand and handle.
		expect(links.map((a) => a.textContent?.replace(/\s+/g, ' ').trim())).toEqual([
			'X (Twitter) @realvelawallet',
			'Telegram @velawallet',
			'Discord discord.gg/23gWrtaYSa'
		]);
	});

	it('ends Community → About → Send feedback, with the report the last row', async () => {
		const view = await drawn('st1');
		const rows = [...view.root.querySelectorAll<HTMLElement>('.scroll .row')].map((r) =>
			r.querySelector('.title')?.textContent?.trim()
		);
		expect(rows.slice(-5)).toEqual([
			'X (Twitter)',
			'Telegram',
			'Discord',
			String(m.about.title),
			String(m.feedback.title)
		]);
	});
});

describe('the report sheet and its viewer', () => {
	async function image(): Promise<File> {
		const canvas = new OffscreenCanvas(320, 640);
		const context = canvas.getContext('2d') as OffscreenCanvasRenderingContext2D;
		context.fillStyle = '#3a6ea5';
		context.fillRect(0, 0, 320, 640);
		return new File([await canvas.convertToBlob({ type: 'image/png' })], 'a.png', {
			type: 'image/png'
		});
	}

	it('Escape closes the viewer alone — the sheet under it stays, with focus back on the tile', async () => {
		const seen: boolean[] = [];
		const screen = render(SettingsHome, {
			props: {
				model: buildMobileState('st15', m, IDENTICON),
				onfeedbacksend: () => {},
				onfeedbackopen: (open: boolean) => seen.push(open)
			}
		});
		await tick();
		const root = screen.container;
		const sheet = () => root.querySelector<HTMLElement>('.sheet[role="dialog"]');
		expect(sheet()).not.toBeNull();
		expect(seen.at(-1)).toBe(true);
		const input = root.querySelector('input[type="file"]') as HTMLInputElement;
		const transfer = new DataTransfer();
		transfer.items.add(await image());
		input.files = transfer.files;
		input.dispatchEvent(new Event('change', { bubbles: true }));
		await vi.waitFor(() => expect(root.querySelector('li.tile .view')).not.toBeNull());
		const tile = root.querySelector('li.tile .view') as HTMLButtonElement;
		tile.click();
		await tick();
		expect(document.querySelector('.viewer')).not.toBeNull();
		document.activeElement?.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })
		);
		await vi.waitFor(() => expect(document.querySelector('.viewer')).toBeNull());
		expect(sheet()).not.toBeNull();
		await vi.waitFor(() => expect(document.activeElement).toBe(tile));
		// A second Escape is the sheet's: it closes, and the route hears it.
		document.activeElement?.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })
		);
		await vi.waitFor(() => expect(sheet()).toBeNull());
		expect(seen.at(-1)).toBe(false);
	});
});

// 078 design review: rows read the same on every shell.
describe('the settings row', () => {
	it('writes its subtitle in fg-muted (fg-subtle failed AA on the light page)', async () => {
		const view = await drawn('st1');
		const subtitle = view.root.querySelector('.scroll .row .subtitle') as HTMLElement;
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-fg-muted)';
		document.body.appendChild(probe);
		expect(getComputedStyle(subtitle).color).toBe(getComputedStyle(probe).color);
		probe.remove();
	});

	it('on a two-line row, the glyph sits beside the TITLE, not the middle of the block', async () => {
		// The page's own box model (app.css: border-box), as the real screen has it.
		await import('../../app.css');
		const view = await drawn('st1');
		const row = [...view.root.querySelectorAll<HTMLElement>('.scroll a.row')][0];
		const glyph = (row.querySelector('.glyph') as HTMLElement).getBoundingClientRect();
		const title = (row.querySelector('.title') as HTMLElement).getBoundingClientRect();
		const subtitle = (row.querySelector('.subtitle') as HTMLElement).getBoundingClientRect();
		const middle = (box: DOMRect) => box.top + box.height / 2;
		expect(Math.abs(middle(glyph) - middle(title))).toBeLessThanOrEqual(1.5);
		expect(glyph.bottom).toBeLessThan(subtitle.top + 2);
		// A one-line row keeps its glyph centred on its title.
		const about = [...view.root.querySelectorAll<HTMLElement>('.scroll .row')].find(
			(r) => r.querySelector('.title')?.textContent?.trim() === String(m.about.title)
		) as HTMLElement;
		const aboutGlyph = (about.querySelector('.glyph') as HTMLElement).getBoundingClientRect();
		const aboutTitle = (about.querySelector('.title') as HTMLElement).getBoundingClientRect();
		expect(Math.abs(middle(aboutGlyph) - middle(aboutTitle))).toBeLessThanOrEqual(1.5);
	});

	it('draws the three brand marks inset to the icon column, as iOS and Android do', async () => {
		const view = await drawn('st1');
		const marks = [...view.root.querySelectorAll<HTMLElement>('.scroll a.row .glyph svg')];
		expect(marks).toHaveLength(3);
		for (const mark of marks) {
			expect(getComputedStyle(mark).transform).not.toBe('none');
			// Drawn at the row glyph's 20, then inset to its middle 20/24.
			expect(Math.round(mark.getBoundingClientRect().width)).toBe(17);
		}
	});
});
