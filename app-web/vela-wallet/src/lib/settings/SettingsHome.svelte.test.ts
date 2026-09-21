/**
 * The phone settings layout, in a browser (spec 072): removing a custom
 * network asks first, naming it — the bin removed it on one tap — and the
 * questions and opening events it already had still hold now that the wide
 * layout shares them.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
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
	const buttonIn = (scope: ParentNode, text: string) =>
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
