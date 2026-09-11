import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { PASSKEY_ICONS, methodIcon, platformIcon } from './passkey-icons';

const contract = JSON.parse(
	readFileSync(
		join(
			import.meta.dirname,
			'..',
			'..',
			'..',
			'..',
			'..',
			'specs',
			'038-first-run-parity',
			'contracts',
			'passkey-icons.json'
		),
		'utf8'
	)
) as {
	icons: {
		id: string;
		viewBox: number[];
		elements: { tag: string; role: string; [k: string]: string }[];
	}[];
};

describe('passkey icons', () => {
	it('is the contract, element for element', () => {
		for (const icon of contract.icons) {
			const port = PASSKEY_ICONS[icon.id as keyof typeof PASSKEY_ICONS];
			expect(port, icon.id).toBeDefined();
			expect(port.viewBox).toEqual(icon.viewBox);
			expect(port.elements.length).toBe(icon.elements.length);
			icon.elements.forEach((el, i) => {
				const { tag, role, ...attrs } = el;
				expect(port.elements[i].tag).toBe(tag);
				expect(port.elements[i].role).toBe(role);
				expect(port.elements[i].attrs).toEqual(attrs);
			});
		}
	});

	it('resolves "this device" to the platform the app is running on', () => {
		const safariMac =
			'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15';
		const chromeMac =
			'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36';
		const chromeWin =
			'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36';
		const android =
			'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/152.0.0.0 Mobile Safari/537.36';
		const linuxFirefox = 'Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0';
		expect(platformIcon(safariMac)).toBe('apple');
		expect(platformIcon(chromeMac)).toBe('chrome-mac');
		expect(platformIcon(chromeWin)).toBe('windows');
		expect(platformIcon(android)).toBe('google');
		expect(platformIcon(linuxFirefox)).toBe('fido2');
		expect(methodIcon('hybrid')).toBe('google');
		expect(methodIcon('security_key')).toBe('usb');
	});
});
