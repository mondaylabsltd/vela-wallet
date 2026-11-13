/**
 * Round 5: the connect page opened from an approve-later link says "Reading
 * the transaction…" from its first frame — `app.html` reads the fragment
 * before any module runs (the prerendered page never sees it). Run here
 * against links as `connectLink` builds them.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { connectLink } from '../core/link';

function boot(href: string): boolean {
	const html = readFileSync(new URL('../../../app.html', import.meta.url), 'utf8');
	const block = html.slice(
		html.indexOf('/* connect-boot:begin */'),
		html.indexOf('/* connect-boot:end */')
	);
	const url = new URL(href);
	const dataset: Record<string, string> = {};
	new Function('location', 'document', block)(
		{ pathname: url.pathname, hash: url.hash },
		{ documentElement: { dataset } }
	);
	return dataset.connectTx === '1';
}

const TX = {
	chainId: 5042002,
	multisig: '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83',
	safeTxHash: `0x${'ab'.repeat(32)}`
};
const MEMBER = '0x70997970C51812dc3A010C7d01b50e0d17dc79C8';
const INVITE = `wss://relay.peerconnect.dev/#${'A'.repeat(86)}`;
const PAGE = 'https://getvela.app/en/connect';

describe('the connect page’s first frame', () => {
	it('an approve-later link: reading its transaction', () => {
		expect(boot(connectLink(PAGE, null, [MEMBER], TX))).toBe(true);
		expect(boot(connectLink(`${PAGE}/`, null, [], TX))).toBe(true);
	});

	it('an invite (with or without its transaction), or no transaction: waiting on Vela', () => {
		expect(boot(connectLink(PAGE, INVITE, [MEMBER], TX))).toBe(false);
		expect(boot(connectLink(PAGE, INVITE, [MEMBER]))).toBe(false);
		expect(boot(connectLink(PAGE, null, [MEMBER]))).toBe(false);
	});

	it('any other page with such a fragment is left alone', () => {
		expect(boot(connectLink('https://getvela.app/en/wallet', null, [], TX))).toBe(false);
	});
});
