/**
 * The phone settings layout, in a browser (spec 072): removing a custom
 * network asks first, naming it — the bin removed it on one tap — and the
 * questions and opening events it already had still hold now that the wide
 * layout shares them.
 */
import { createRawSnippet, tick } from 'svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { words } from './__fixtures__/words';
import { buildMobileState } from './fixtures';
import type { MobileSettingsStateId, SettingsHomeModel, SettingsRowModel } from './model';
import type { SettingsNetEvent } from './net-events';
import type { SettingsPrefEvent } from './pref-events';
import SettingsHome from './SettingsHome.svelte';

const m = words();
// The venue rows' refusals are the core's sentences (`venueBlockLine`), so
// the fixtures are built over the loaded core.
beforeAll(() => loadCore());
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

// set-5: back from the add-network wizard with something entered asks first.
describe('leaving the add-network wizard', () => {
	const DISCARD = {
		title: 'Discard this network?',
		body: 'What you entered here will be lost.',
		confirm: 'Discard',
		cancel: 'Keep editing',
		tone: 'danger' as const
	};

	async function wizard(discard: typeof DISCARD | undefined) {
		const net: SettingsNetEvent[] = [];
		const base = buildMobileState('st10', m, IDENTICON);
		const screen = render(SettingsHome, {
			props: {
				model: { ...base, addNetwork: { ...base.addNetwork, discard } },
				onnetevent: (event: SettingsNetEvent) => net.push(event)
			}
		});
		await tick();
		const root = screen.container;
		const back = () =>
			root.querySelector<HTMLButtonElement>(`button[aria-label="${String(base.closeLabel)}"]`);
		const title = () => root.querySelector('h1, h2')?.textContent?.trim();
		const buttonIn = (text: string) =>
			[...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === text);
		return { net, root, back, title, buttonIn, addTitle: base.addNetwork.title };
	}

	it('with nothing entered, back just goes back', async () => {
		const view = await wizard(undefined);
		view.back()?.click();
		await tick();
		expect(view.root.querySelector('[role="dialog"]')).toBeNull();
		expect(view.title()).not.toBe(String(view.addTitle));
		expect(view.net).toEqual([]);
	});

	it('with something entered, asks — Keep editing stays, Discard empties and leaves', async () => {
		const view = await wizard(DISCARD);
		view.back()?.click();
		await tick();
		expect(view.root.querySelector('[role="dialog"]')?.getAttribute('aria-label')).toBe(
			DISCARD.title
		);
		view.buttonIn(DISCARD.cancel)?.click();
		await tick();
		expect(view.root.querySelector('[role="dialog"]')).toBeNull();
		expect(view.title()).toBe(String(view.addTitle));
		expect(view.net).toEqual([]);

		view.back()?.click();
		await tick();
		view.buttonIn(DISCARD.confirm)?.click();
		await tick();
		expect(view.net).toEqual([{ kind: 'discard-add' }]);
		expect(view.title()).not.toBe(String(view.addTitle));
	});
});

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
	it('opening the endpoints page raises endpoints-open', async () => {
		const view = await drawn('st1b');
		const row = [...view.root.querySelectorAll('button')].find((b) =>
			b.textContent?.includes(String(m.advanced.endpointsTitle))
		);
		await view.click(row);
		expect(view.net).toEqual([{ kind: 'endpoints-open' }]);
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
		// Sign Out is a row too (issue #322), on its own after the list.
		const rows = [...view.root.querySelectorAll<HTMLElement>('.scroll .row')]
			.filter((r) => !r.closest('.sign-out'))
			.map((r) => r.querySelector('.title')?.textContent?.trim());
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

// Issue #322: Sign Out read as a caption — centred grey text under About, with
// a bordered card below it that did look like a control.
describe('Sign Out', () => {
	it('is a settings row like its neighbours: glyph, title, chevron, the row height', async () => {
		await import('../../app.css');
		const signedOut: string[] = [];
		const screen = render(SettingsHome, {
			props: {
				model: buildMobileState('st1', m, IDENTICON),
				onsignout: () => signedOut.push('sign-out')
			}
		});
		await tick();
		const rows = [...screen.container.querySelectorAll<HTMLElement>('.scroll .row')];
		const title = (row: HTMLElement) => row.querySelector('.title')?.textContent?.trim();
		const signOut = rows.find((row) => title(row) === String(m.signOut.button));
		const about = rows.find((row) => title(row) === String(m.about.title));
		expect(signOut, 'Sign Out is drawn as a settings row').toBeTruthy();
		expect(signOut?.tagName).toBe('BUTTON');
		expect(signOut?.querySelector('.glyph svg'), 'it carries a glyph').toBeTruthy();
		expect(signOut?.querySelector('.trailing svg'), 'it carries the chevron').toBeTruthy();
		expect(signOut?.getBoundingClientRect().height).toBe(about?.getBoundingClientRect().height);

		signOut?.click();
		await tick();
		expect(signedOut).toEqual(['sign-out']);
	});
});

/**
 * Spec 102: where THIS account reviews and signs is a row under the account
 * (the venue is the account's, not the device's), and the pages this device
 * trusts are a row in Advanced. Both push their page.
 */
describe('where you review and sign (spec 102)', () => {
	it('the row under the account says where it signs now, and opens the choice', async () => {
		const view = await drawn('st1');
		const row = [...view.root.querySelectorAll<HTMLElement>('button')].find((b) =>
			b.textContent?.includes(String(m.venue.title))
		);
		// One line under the title: where it signs now, and on which host.
		expect(row?.querySelector('.subtitle')?.textContent).toContain(String(m.venue.page));
		expect(row?.querySelector('.subtitle')?.textContent).toContain('sign.getvela.app');
		await view.click(row);
		expect(view.root.querySelector('[role="radiogroup"]')).not.toBeNull();
		// Vela's sheet, the official page, and the two self-hosted pages saved here.
		expect(view.root.querySelectorAll('[role="radio"]')).toHaveLength(4);
	});

	it('Signing pages sits in Advanced and opens the list', async () => {
		const view = await drawn('st1b');
		const row = [...view.root.querySelectorAll<HTMLElement>('button')].find((b) =>
			b.textContent?.includes(String(m.signing.title))
		);
		await view.click(row);
		// The official page, then the two self-hosted ones.
		expect(view.root.querySelectorAll('li.page')).toHaveLength(3);
	});

	it('a model with no Signing pages (the web) has no such page to open', async () => {
		const model = buildMobileState('st18', m, IDENTICON);
		const screen = render(SettingsHome, {
			props: { model: { ...model, signingPages: undefined } }
		});
		await tick();
		expect(screen.container.querySelectorAll('li.page')).toHaveLength(0);
	});
});

/**
 * dapp-11: connected sites were only under Advanced › Device storage. They
 * have their own page now (the route adds its row beside the AI agent), each
 * with a Disconnect that asks first and acts only on its confirm.
 */
describe('Connected sites', () => {
	const sites = {
		title: 'Connected sites',
		subtitle: 'Sites that can see your address',
		empty: 'No site is connected.',
		group: {
			label: 'Disconnecting a site takes back what it may see.',
			items: [
				{
					id: 'https://app.example',
					label: 'app.example',
					meta: '0xAbC…123',
					action: 'Disconnect',
					destructive: true
				}
			]
		}
	};

	async function opened(panel: typeof sites) {
		const cleared: string[] = [];
		const screen = render(SettingsHome, {
			props: {
				model: buildMobileState('st1', m, IDENTICON),
				sites: panel,
				onstorageclear: (id: string) => cleared.push(id)
			}
		});
		await tick();
		expect(screen.component.open('sites')).toBe(true);
		await tick();
		return { cleared, root: screen.container };
	}

	it('lists every site with its account, and disconnects one only on confirm', async () => {
		const { cleared, root } = await opened(sites);
		expect(root.textContent).toContain('Connected sites');
		expect(root.textContent).toContain('app.example');
		expect(root.textContent).toContain('0xAbC…123');
		[...root.querySelectorAll('li button')]
			.find((b) => b.textContent?.trim() === 'Disconnect')
			?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		await tick();
		expect(cleared).toEqual([]);
		const sheet = root.querySelector<HTMLElement>('[role="dialog"]');
		expect(sheet?.getAttribute('aria-label')).toBe('app.example');
		expect(sheet?.textContent).toContain(sites.group.label);
		[...(sheet?.querySelectorAll('button') ?? [])]
			.find((b) => b.textContent?.trim() === 'Disconnect')
			?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		await tick();
		expect(cleared).toEqual(['https://app.example']);
	});

	it('says so when no site is connected', async () => {
		const { root } = await opened({ ...sites, group: { ...sites.group, items: [] } });
		expect(root.textContent).toContain('No site is connected.');
	});
});

// Owner, 2026-10-29: "Share anonymous usage statistics" left About for 高级,
// under the AI agent and the connected sites — the rows about who sees this
// wallet — and the account says it is a Safe.
describe('usage statistics and the Safe line, on the phone', () => {
	const ANALYTICS = {
		id: 'settings-analytics',
		icon: 'eye-off' as const,
		title: 'Share anonymous usage statistics',
		subtitle: 'Which screens open and which steps finish.',
		on: false
	};
	const AGENT: SettingsRowModel = {
		id: 'agent',
		icon: 'link-2',
		title: 'AI agent',
		subtitle: 'Let an AI agent use this wallet',
		trailing: 'chevron'
	};

	/** The agent's row after the endpoints — as the route adds it. */
	function withAgent(model: SettingsHomeModel): SettingsHomeModel {
		return {
			...model,
			sections: model.sections.map((section) => {
				const at = section.rows.findIndex((row) => row.id === 'endpoints');
				if (at < 0) return section;
				const rows = [...section.rows.slice(0, at + 1), AGENT, ...section.rows.slice(at + 1)];
				return { ...section, rows };
			})
		};
	}

	it('stands in Advanced right under the AI agent, and reports a flip', async () => {
		const prefs: SettingsPrefEvent[] = [];
		const screen = render(SettingsHome, {
			props: {
				model: withAgent(buildMobileState('st1b', m, IDENTICON)),
				analytics: ANALYTICS,
				onprefevent: (event: SettingsPrefEvent) => prefs.push(event)
			}
		});
		await tick();
		const toggle = screen.container.querySelector<HTMLButtonElement>('[role="switch"]');
		expect(toggle?.getAttribute('aria-checked')).toBe('false');
		const before = toggle?.previousElementSibling;
		expect(before?.querySelector('.title')?.textContent?.trim()).toBe('AI agent');
		// One of the group: a hairline under it, as between the rows.
		expect(toggle?.classList.contains('divider')).toBe(true);
		toggle?.click();
		expect(prefs).toEqual([{ kind: 'analytics', on: true }]);
		await screen.unmount();
	});

	it('is not in About any more', async () => {
		const screen = render(SettingsHome, {
			props: { model: withAgent(buildMobileState('st14', m, IDENTICON)), analytics: ANALYTICS }
		});
		await tick();
		expect(screen.container.textContent).toContain(String(m.about.footer));
		expect(screen.container.querySelector('[role="switch"]')).toBe(null);
		await screen.unmount();
	});

	it('says under the account’s keys that it is a Safe, linking Safe{Wallet}', async () => {
		const screen = render(SettingsHome, {
			props: {
				model: buildMobileState('st1', m, IDENTICON),
				accountKeys: createRawSnippet(() => ({ render: () => '<p class="members">members</p>' }))
			}
		});
		await tick();
		const line = screen.container.querySelector('.keys-block .safe-wallet');
		expect(line?.textContent).toContain('Safe{Wallet}');
		expect(line?.querySelector('a')?.getAttribute('href')).toBe('https://app.safe.global');
		expect(line?.previousElementSibling?.classList.contains('members')).toBe(true);
		await screen.unmount();
	});
});
