import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { PASSKEY_ICONS, isHandheld, methodGlyph } from './passkey-icons';

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

	it('shows the DEVICE for "this device", the scanner for a phone, the key for a key', () => {
		const iphone =
			'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 Version/17.5 Mobile/15E148 Safari/604.1';
		const mac =
			'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15';
		expect(isHandheld(iphone, false)).toBe(true);
		expect(isHandheld(mac, false)).toBe(false);
		expect(isHandheld(mac, true)).toBe(true);
		expect(methodGlyph('platform', false)).toEqual({ kind: 'lucide', name: 'laptop' });
		expect(methodGlyph('platform', true)).toEqual({ kind: 'lucide', name: 'smartphone' });
		expect(methodGlyph('hybrid')).toEqual({ kind: 'lucide', name: 'scan-line' });
		expect(methodGlyph('security_key')).toEqual({ kind: 'mark', id: 'usb' });
	});
});
