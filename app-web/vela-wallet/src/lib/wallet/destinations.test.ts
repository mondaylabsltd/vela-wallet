/**
 * The web has three destinations (spec 022). One list, one filter — the tab
 * bars and the sidebars on every route read it, so 探索 cannot come back on
 * one of them alone, which is how it did come back on 通讯录 and 设置.
 */
import { describe, expect, it } from 'vitest';
import { WEB_DESTINATIONS, webNavItems } from './destinations';

describe('the web shell', () => {
	it('has no 探索', () => {
		expect(WEB_DESTINATIONS).toEqual(['wallet', 'contacts', 'settings']);
	});

	it('reduces a drawn four-item nav to those three, in the drawn order', () => {
		const drawn = [
			{ id: 'wallet', label: 'Wallet' },
			{ id: 'contacts', label: 'Contacts' },
			{ id: 'explore', label: 'Explore' },
			{ id: 'settings', label: 'Settings' }
		];
		expect(webNavItems(drawn).map((item) => item.id)).toEqual(['wallet', 'contacts', 'settings']);
	});
});
