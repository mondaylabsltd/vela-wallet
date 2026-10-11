/**
 * The address's mark, as it is DRAWN (device walk of 2026-10-11, item 11).
 *
 * `site-mark.test.ts` holds the rule and the words; this holds the picture, in
 * the four places a site's lock is drawn — the phone's address bar, the wide
 * toolbar, the site menu and the connection panel. For `http://127.0.0.1:8137`
 * each drew a padlock (two of them drew one for every page); none does now.
 */
import '$lib/tokens/tokens.css';
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { loadCore } from '$lib/core/client';
import { UTILITY_ICONS, type IconDef } from '$lib/wallet/icons';
import type { ExploreMessages } from '../messages';
import type { BrowserModel, ConnectionModel, ExploreDesktopModel, SiteModel } from '../model';
import { siteMark } from '../site-mark';
import AddressBar from './AddressBar.svelte';
import ConnectionPanel from './ConnectionPanel.svelte';
import DesktopToolbar from './DesktopToolbar.svelte';
import SiteMenuSheet from './SiteMenuSheet.svelte';

/** The four strings the mark is worded from; the rest of the bar's copy is not under test. */
const copy = {
	secureSite: 'Secure site',
	localSite: 'Local page',
	a11yLocal: 'Local page — not encrypted',
	a11yInsecure: 'Insecure site — not encrypted',
	account: 'Account',
	connectedTag: 'Connected',
	back: 'Back',
	addressBar: 'Address',
	tabs: 'Tabs',
	siteMenu: 'Menu'
} as unknown as ExploreMessages;

const LOCAL = 'http://127.0.0.1:8137';
const HTTPS = 'https://app.uniswap.org';
const PUBLIC_HTTP = 'http://example.com';

const SITE: SiteModel = {
	id: 's',
	name: 'Test dApp',
	host: '127.0.0.1:8137',
	letter: 'T',
	tint: 'blue'
};

function browser(origin: string): BrowserModel {
	return {
		url: `${origin}/`,
		host: new URL(origin).host,
		mark: siteMark(origin, copy),
		connected: true,
		canBack: true,
		canForward: false,
		bookmarked: false,
		account: { name: 'Main', identiconSvg: '<svg></svg>' },
		tabCount: 1,
		page: { title: '', fields: [], cta: '', ctaTint: '' }
	};
}

/** A stroke of each icon, as its `<svg>` carries it — what tells a padlock from an "i". */
const strokeOf = (icon: IconDef): string => {
	if (icon.style === 'fill') return icon.paths[0];
	const path = icon.elements.find((el) => el.tag === 'path');
	if (path?.tag !== 'path') throw new Error('the icon has no path to tell it by');
	return path.d;
};
const PADLOCK = strokeOf(UTILITY_ICONS.lock);
const WARNING = strokeOf(UTILITY_ICONS['triangle-alert']);
const INFO = strokeOf(UTILITY_ICONS.info);

/** What the mark in `container` is: its lock, the icon it draws, and its name. */
function markIn(container: Element) {
	const mark = container.querySelector('[data-testid="site-mark"]');
	const svg = mark?.querySelector('svg');
	return {
		lock: mark?.getAttribute('data-lock') ?? null,
		svg: svg?.innerHTML ?? null,
		name: svg?.getAttribute('aria-label') ?? null,
		color: mark ? getComputedStyle(mark).color : null
	};
}

/** The drawn form of an icon, read off a mark that is known to draw it. */
function drawnIcon(origin: string): string {
	const screen = render(AddressBar, { props: { browser: browser(origin), copy } });
	const svg = markIn(screen.container).svg ?? '';
	screen.unmount();
	return svg;
}

beforeAll(() => loadCore());

describe('the mark beside a site’s address', () => {
	it('is a padlock for https, an "i" for a local page, the warning for public http — three different pictures', () => {
		const [padlock, local, warning] = [HTTPS, LOCAL, PUBLIC_HTTP].map(drawnIcon);
		expect(new Set([padlock, local, warning]).size).toBe(3);
		// And they ARE those three icons, not three of anything.
		expect(new Set([PADLOCK, INFO, WARNING]).size).toBe(3);
		expect(padlock).toContain(PADLOCK);
		expect(local).toContain(INFO);
		expect(local).not.toContain(PADLOCK);
		expect(warning).toContain(WARNING);
		expect(warning).not.toContain(PADLOCK);
	});

	it('the phone’s address bar: no padlock on http://127.0.0.1:8137, and the mark is named', () => {
		const padlock = drawnIcon(HTTPS);
		const screen = render(AddressBar, { props: { browser: browser(LOCAL), copy } });
		const mark = markIn(screen.container);
		expect(mark.lock).toBe('local');
		expect(mark.svg).not.toBe(padlock);
		expect(mark.name).toBe('Local page — not encrypted');
		screen.unmount();

		const secure = render(AddressBar, { props: { browser: browser(HTTPS), copy } });
		expect(markIn(secure.container)).toMatchObject({ lock: 'secure', name: 'Secure site' });
		secure.unmount();

		const open = render(AddressBar, { props: { browser: browser(PUBLIC_HTTP), copy } });
		expect(markIn(open.container)).toMatchObject({
			lock: 'insecure',
			name: 'Insecure site — not encrypted'
		});
		open.unmount();
	});

	it('the wide toolbar: the page’s own mark, where it drew a padlock for every page', () => {
		const padlock = drawnIcon(HTTPS);
		const toolbar = {
			back: 'Back',
			forward: 'Forward',
			reload: 'Reload',
			searchPlaceholder: 'Search',
			bookmark: 'Bookmark',
			menu: 'Menu'
		} as ExploreDesktopModel['toolbar'];
		const drawn = (origin: string) => {
			const screen = render(DesktopToolbar, {
				props: {
					toolbar,
					browser: browser(origin),
					browsing: true,
					accountLabel: 'Account',
					connectedLabel: 'Connected'
				}
			});
			const mark = markIn(screen.container);
			screen.unmount();
			return mark;
		};
		expect(drawn(HTTPS)).toMatchObject({ lock: 'secure', svg: padlock });
		const local = drawn(LOCAL);
		expect(local.lock).toBe('local');
		expect(local.svg).not.toBe(padlock);
		expect(local.name).toBe('Local page — not encrypted');
		expect(drawn(PUBLIC_HTTP).lock).toBe('insecure');
	});

	it('the site menu: the words and the mark agree, and a line that is only a host has no mark', () => {
		const padlock = drawnIcon(HTTPS);
		const drawn = (origin: string | null) => {
			const mark = origin === null ? undefined : siteMark(origin, copy);
			const screen = render(SiteMenuSheet, {
				props: {
					site: SITE,
					mark,
					statusLine: mark?.words ?? SITE.host,
					items: [],
					closeLabel: 'Close'
				}
			});
			const status = screen.container.querySelector('.status')!;
			const result = {
				mark: markIn(screen.container),
				text: status.textContent?.trim(),
				tone: getComputedStyle(status).color
			};
			screen.unmount();
			return result;
		};
		const secure = drawn(HTTPS);
		const local = drawn(LOCAL);
		const open = drawn(PUBLIC_HTTP);
		expect(secure).toMatchObject({ text: 'Secure site', mark: { lock: 'secure', svg: padlock } });
		expect(local.text).toBe('Local page');
		expect(local.mark.lock).toBe('local');
		expect(local.mark.svg).not.toBe(padlock);
		// The words beside it name it already: the mark is not read twice.
		expect(local.mark.name).toBeNull();
		expect(open.text).toBe('Insecure site — not encrypted');
		// Three tones: a local page is not praised and not warned about.
		expect(new Set([secure.tone, local.tone, open.tone]).size).toBe(3);
		// A history row's menu: its line is a host, and it draws no mark.
		const recent = drawn(null);
		expect(recent.mark.lock).toBeNull();
		expect(recent.text).toBe(SITE.host);
		expect(recent.tone).toBe(local.tone);
	});

	it('the connection panel: "Local page · Connected" beside the local mark, never a padlock', () => {
		const padlock = drawnIcon(HTTPS);
		const connection = (origin: string): ConnectionModel => {
			const mark = siteMark(origin, copy);
			return {
				title: 'Connection',
				site: SITE,
				mark,
				statusLine: `${mark.words} · Connected`,
				account: { name: 'Main', address: '0x14fb…1d1e', identiconSvg: '<svg></svg>' },
				switchLabel: 'Switch account',
				networkLabel: 'Network',
				network: { name: 'Gnosis', dot: 'green' },
				explainer: '',
				disconnect: 'Disconnect',
				footnote: ''
			};
		};
		const screen = render(ConnectionPanel, { props: { connection: connection(LOCAL) } });
		expect(screen.container.querySelector('.status')?.textContent?.trim()).toBe(
			'Local page · Connected'
		);
		const mark = markIn(screen.container);
		expect(mark.lock).toBe('local');
		expect(mark.svg).not.toBe(padlock);
		screen.unmount();

		const secure = render(ConnectionPanel, { props: { connection: connection(HTTPS) } });
		expect(markIn(secure.container)).toMatchObject({ lock: 'secure', svg: padlock });
		secure.unmount();
	});
});
