/**
 * The wide settings layout, in a browser (spec 072): what the phone already
 * did, it now does too — showing the providers or the endpoints is a core
 * event, and every destructive row asks first, naming what goes.
 *
 * The P0 is the first test. Without `providers-open` the core never seeded
 * its drafts, and leaving a saved key's field saved an empty draft over the
 * key it showed.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { words } from './__fixtures__/words';
import { buildDesktopState } from './fixtures';
import { withEraseFailure } from './live';
import type { SettingsDesktopModel } from './model';
import type { SettingsNetEvent } from './net-events';
import type { SettingsPrefEvent } from './pref-events';
import SettingsDesktop from './SettingsDesktop.svelte';

const m = words();
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;

async function drawn(model: SettingsDesktopModel = buildDesktopState('dst1', m, IDENTICON)) {
	const net: SettingsNetEvent[] = [];
	const prefs: SettingsPrefEvent[] = [];
	const cleared: string[] = [];
	const screen = render(SettingsDesktop, {
		props: {
			model,
			onnetevent: (event: SettingsNetEvent) => net.push(event),
			onprefevent: (event: SettingsPrefEvent) => prefs.push(event),
			onstorageclear: (id: string) => cleared.push(id)
		}
	});
	await tick();
	const root = screen.container;
	const buttonIn = (scope: Element, text: string) =>
		[...scope.querySelectorAll('button')].find((b) => b.textContent?.trim() === text);
	const dialog = () => root.querySelector<HTMLElement>('[role="dialog"]');
	return {
		screen,
		net,
		prefs,
		cleared,
		dialog,
		async click(element: HTMLElement | null | undefined) {
			expect(element, 'the control is drawn').toBeTruthy();
			element?.click();
			await tick();
		},
		nav: (label: string) => buttonIn(root.querySelector('nav.settings-nav') ?? root, label),
		buttonIn,
		root
	};
}

describe('showing a panel is the core event the phone raises', () => {
	it('the RPC providers panel raises providers-open (P0)', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.advanced.rpcProvidersTitle)));
		expect(view.net).toEqual([{ kind: 'providers-open' }]);
	});

	it('the endpoints panel raises endpoints-open', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.advanced.endpointsTitle)));
		expect(view.net).toEqual([{ kind: 'endpoints-open' }]);
	});

	it('other panels raise nothing', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.sections.appearance)));
		await view.click(view.nav(String(m.storage.title)));
		expect(view.net).toEqual([]);
	});
});

describe('a destructive row asks first', () => {
	it('a storage row names itself, and clears only on confirm', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.storage.title)));
		const row = [...view.root.querySelectorAll('li')].find((li) =>
			li.textContent?.includes(String(m.storage.itemContacts))
		);
		await view.click(row?.querySelector('button'));

		// Asked, not done: the whole address book used to go on this one click.
		expect(view.cleared).toEqual([]);
		expect(view.dialog()?.getAttribute('aria-label')).toBe(String(m.storage.itemContacts));

		await view.click(view.buttonIn(view.dialog() as HTMLElement, String(m.storage.clear)));
		expect(view.cleared).toEqual(['contacts']);
		expect(view.dialog()).toBeNull();
	});

	it('cancelling a storage row clears nothing', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.storage.title)));
		const row = [...view.root.querySelectorAll('li')].find((li) =>
			li.textContent?.includes(String(m.storage.itemTransactions))
		);
		await view.click(row?.querySelector('button'));
		await view.click(view.buttonIn(view.dialog() as HTMLElement, String(m.common.cancel)));
		expect(view.cleared).toEqual([]);
		expect(view.dialog()).toBeNull();
	});

	it('removing a custom network names it, and removes only on confirm', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.advanced.networksTitle)));
		await view.click(
			view.root.querySelector<HTMLElement>(`button[aria-label="${String(m.networks.remove)}"]`)
		);
		expect(view.net).toEqual([]);
		expect(view.dialog()?.getAttribute('aria-label')).toBe('X Layer');
		expect(view.dialog()?.textContent).toContain(String(m.networks.removeBody));

		await view.click(view.buttonIn(view.dialog() as HTMLElement, String(m.networks.removeConfirm)));
		expect(view.net).toEqual([{ kind: 'delete-network', id: 'xlayer' }]);
	});
});

describe('erase', () => {
	it('asks the phone’s question, erases on confirm, and says a failure where it was asked', async () => {
		const model = buildDesktopState('dst1', m, IDENTICON);
		const view = await drawn(model);
		// It was drawn here and did nothing.
		const card = [...view.root.querySelectorAll('button')].find((b) =>
			b.textContent?.includes(String(m.erase.subtitle))
		);
		await view.click(card);
		expect(view.dialog()?.getAttribute('aria-label')).toBe(String(m.erase.title));
		expect(view.dialog()?.textContent).toContain(String(m.erase.desc));
		expect(view.prefs).toEqual([]);

		await view.click(view.buttonIn(view.dialog() as HTMLElement, String(m.erase.confirm)));
		expect(view.prefs).toEqual([{ kind: 'erase' }]);
		// Still up: the route reports a failure through this dialog.
		await view.screen.rerender({ model: withEraseFailure(model, m, true) });
		expect(view.dialog()?.textContent).toContain(String(m.erase.failed));
	});

	it('cancelling erases nothing', async () => {
		const view = await drawn();
		const card = [...view.root.querySelectorAll('button')].find((b) =>
			b.textContent?.includes(String(m.erase.subtitle))
		);
		await view.click(card);
		await view.click(view.buttonIn(view.dialog() as HTMLElement, String(m.erase.cancel)));
		expect(view.prefs).toEqual([]);
		expect(view.dialog()).toBeNull();
	});
});

describe('rows that go somewhere, go there', () => {
	it('"Get key" opens the provider’s key page; the key test is its own action', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.advanced.rpcProvidersTitle)));
		const getKey = [...view.root.querySelectorAll<HTMLAnchorElement>('a.action')].find((a) =>
			a.closest('section')?.textContent?.includes('dRPC')
		);
		expect(getKey?.textContent?.trim()).toBe(String(m.rpcProviders.getKey));
		expect(getKey?.href).toBe('https://drpc.org/');
		expect(getKey?.target).toBe('_blank');

		const check = view.buttonIn(view.root, String(m.rpcProviders.checkKey));
		await view.click(check);
		expect(view.net).toContainEqual({ kind: 'provider-test', provider: 'alchemy' });
	});

	it('About’s links are links', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.about.title)));
		const hrefs = [...view.root.querySelectorAll<HTMLAnchorElement>('a.kv')].map((a) => a.href);
		expect(hrefs).toEqual([
			'https://getvela.app/',
			'https://github.com/mondaylabsltd/vela-wallet',
			'https://safe.global/'
		]);
	});
});

// 078: the official accounts, one click from Settings; the last group is
// About, then Send feedback — the same on all four shells.
describe('Community and the report, on the desktop', () => {
	it('the nav ends Community → About → Send feedback', async () => {
		const view = await drawn();
		const labels = [...view.root.querySelectorAll('nav.settings-nav button')].map((b) =>
			b.textContent?.trim()
		);
		expect(labels.slice(-3)).toEqual([
			String(m.sections.community),
			String(m.about.title),
			String(m.feedback.title)
		]);
	});

	it('Community opens a panel of three links, each leaving in a new tab', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.sections.community)));
		expect(view.root.querySelector('.panel h1')?.textContent?.trim()).toBe(
			String(m.sections.community)
		);
		const links = [...view.root.querySelectorAll<HTMLAnchorElement>('.community a')];
		expect(links.map((a) => a.getAttribute('href'))).toEqual([
			'https://x.com/realvelawallet',
			'https://t.me/velawallet',
			'https://discord.gg/23gWrtaYSa'
		]);
		for (const link of links) {
			expect(link.getAttribute('target')).toBe('_blank');
			expect(link.getAttribute('rel')).toBe('noopener noreferrer');
		}
		expect(links[0].textContent).toContain('@realvelawallet');
	});

	it('tells the route when the report panel is on screen, and when it is not', async () => {
		const seen: boolean[] = [];
		const screen = render(SettingsDesktop, {
			props: {
				model: buildDesktopState('dst1', m, IDENTICON),
				onfeedbackopen: (open: boolean) => seen.push(open)
			}
		});
		await tick();
		const nav = (label: string) =>
			[...screen.container.querySelectorAll('nav.settings-nav button')].find(
				(b) => b.textContent?.trim() === label
			) as HTMLButtonElement;
		nav(String(m.feedback.title)).click();
		await tick();
		nav(String(m.about.title)).click();
		await tick();
		expect(seen.filter((v, i) => i === 0 || v !== seen[i - 1])).toEqual([false, true, false]);
		screen.unmount();
		expect(seen.at(-1)).toBe(false);
	});
});
