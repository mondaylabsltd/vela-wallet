/**
 * The wide settings layout, in a browser (spec 072): what the phone already
 * did, it now does too — showing the endpoints is a core event, and every
 * destructive row asks first, naming what goes.
 */
import { createRawSnippet, tick } from 'svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import { words } from './__fixtures__/words';
import { buildDesktopState } from './fixtures';
import { withEraseFailure } from './live';
import type { SettingsDesktopModel } from './model';
import type { SettingsNetEvent } from './net-events';
import type { SettingsPrefEvent } from './pref-events';
import SettingsDesktop from './SettingsDesktop.svelte';

const m = words();
// The venue rows' refusals are the core's sentences (`venueBlockLine`), so
// the fixtures are built over the loaded core.
beforeAll(() => loadCore());
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
		// Gone once it has played its way out (vis-19).
		await vi.waitFor(() => expect(view.dialog()).toBeNull());
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
		// Gone once it has played its way out (vis-19).
		await vi.waitFor(() => expect(view.dialog()).toBeNull());
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
		await view.screen.rerender({ model: withEraseFailure(model, m, 'incomplete') });
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
		// Gone once it has played its way out (vis-19).
		await vi.waitFor(() => expect(view.dialog()).toBeNull());
	});
});

describe('rows that go somewhere, go there', () => {
	it('About’s links are links', async () => {
		const view = await drawn();
		await view.click(view.nav(String(m.about.title)));
		const hrefs = [...view.root.querySelectorAll<HTMLAnchorElement>('a.kv')].map((a) => a.href);
		expect(hrefs).toEqual([
			// Spec 095 (App Review 5.1.1(i)), then the code (owner, 2026-10-29).
			'https://getvela.app/support',
			'https://getvela.app/privacy',
			'https://getvela.app/terms',
			'https://github.com/mondaylabsltd/vela-wallet'
		]);
		// No usage-statistics switch here any more: it has the Privacy panel.
		expect(view.root.querySelector('[role="switch"]')).toBe(null);
	});
});

// Owner, 2026-10-29: "Share anonymous usage statistics" left About for its
// own panel beside the AI agent's, and the account says it is a Safe.
describe('Privacy and the Safe line, on the desktop', () => {
	const ANALYTICS = {
		id: 'settings-analytics',
		icon: 'eye-off' as const,
		title: 'Share anonymous usage statistics',
		subtitle: 'Which screens open and which steps finish.',
		on: true
	};

	it('opens a Privacy panel holding the switch, which reports a flip', async () => {
		const base = buildDesktopState('dst1', m, IDENTICON);
		const prefs: SettingsPrefEvent[] = [];
		const screen = render(SettingsDesktop, {
			props: {
				model: {
					...base,
					nav: [...base.nav, { id: 'privacy', icon: 'eye-off', label: String(m.sections.privacy) }]
				},
				analytics: ANALYTICS,
				onprefevent: (event: SettingsPrefEvent) => prefs.push(event)
			}
		});
		await tick();
		expect(screen.component.open('privacy')).toBe(true);
		await tick();
		expect(screen.container.querySelector('.panel h1')?.textContent?.trim()).toBe(
			String(m.sections.privacy)
		);
		const toggle = screen.container.querySelector<HTMLButtonElement>('[role="switch"]');
		expect(toggle?.getAttribute('aria-checked')).toBe('true');
		toggle?.click();
		expect(prefs).toEqual([{ kind: 'analytics', on: false }]);
		await screen.unmount();
	});

	it('has no Privacy panel where no switch is given (the gallery)', async () => {
		const view = await drawn();
		expect(view.screen.component.open('privacy')).toBe(false);
	});

	it('says under the account’s keys that it is a Safe, linking Safe{Wallet}', async () => {
		const base = buildDesktopState('dst1', m, IDENTICON);
		const screen = render(SettingsDesktop, {
			props: {
				model: base,
				accountKeys: createRawSnippet(() => ({ render: () => '<p class="members">members</p>' }))
			}
		});
		await tick();
		const line = screen.container.querySelector('.safe-wallet');
		expect(line?.textContent).toContain('Safe{Wallet}');
		const link = line?.querySelector('a');
		expect(link?.getAttribute('href')).toBe('https://app.safe.global');
		expect(link?.getAttribute('target')).toBe('_blank');
		// Under the account's own part: after the multisig's members.
		const members = screen.container.querySelector('.members');
		expect(
			members && line && members.compareDocumentPosition(line) & Node.DOCUMENT_POSITION_FOLLOWING
		).toBeTruthy();
		await screen.unmount();
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

// set-5: closing the add-network dialog with something entered asks first.
describe('closing the add-network dialog', () => {
	const DISCARD = {
		title: 'Discard this network?',
		body: 'What you entered here will be lost.',
		confirm: 'Discard',
		cancel: 'Keep editing',
		tone: 'danger' as const
	};

	it('asks; Keep editing brings the dialog back, Discard closes it and empties the wizard', async () => {
		const base = buildDesktopState('dst4b', m, IDENTICON);
		const view = await drawn({ ...base, addNetwork: { ...base.addNetwork, discard: DISCARD } });
		// Dialogs fade out (`transition:…|global`): read the ones still coming in.
		const dialogs = () => [...view.root.querySelectorAll<HTMLElement>('[role="dialog"]')];
		const asking = () => dialogs().some((d) => d.textContent?.includes(DISCARD.body));
		const close = () => dialogs().at(-1)?.querySelector<HTMLElement>('button.close');
		await view.click(close());
		await vi.waitFor(() => expect(asking()).toBe(true));
		expect(view.net).toEqual([]);
		await view.click(view.buttonIn(view.root, DISCARD.cancel));
		await vi.waitFor(() => expect(asking()).toBe(false));
		expect(dialogs().length).toBeGreaterThan(0);
		expect(view.net).toEqual([]);

		await view.click(close());
		await vi.waitFor(() => expect(asking()).toBe(true));
		await view.click(view.buttonIn(view.root, DISCARD.confirm));
		expect(view.net).toEqual([{ kind: 'discard-add' }]);
		await vi.waitFor(() => expect(dialogs()).toEqual([]));
	});

	it('with nothing entered, just closes', async () => {
		const view = await drawn(buildDesktopState('dst4b', m, IDENTICON));
		await view.click(view.root.querySelector<HTMLElement>('[role="dialog"] button.close'));
		await vi.waitFor(() => expect(view.dialog()).toBeNull());
		expect(view.net).toEqual([]);
	});
});
