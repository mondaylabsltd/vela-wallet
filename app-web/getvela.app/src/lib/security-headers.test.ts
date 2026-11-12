/**
 * §35 (review 1-1): getvela.app's headers are one list. The static assets'
 * `_headers` `/*` block is exactly `SITE_HEADERS`; the Worker's responses get
 * the same from `applySiteHeaders`, which keeps a response's own CSP. And the
 * privacy page says what the wallet's usage statistics now are (review 1-8):
 * opt-in, and never about a transaction.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { SITE_HEADERS, applySiteHeaders } from './security-headers';

function block(file: string, path: string): Record<string, string> {
	const lines = readFileSync(new URL(file, import.meta.url), 'utf8').split('\n');
	const start = lines.findIndex((l) => l.trim() === path);
	expect(start, `${path} in ${file}`).toBeGreaterThanOrEqual(0);
	const out: Record<string, string> = {};
	for (const line of lines.slice(start + 1)) {
		if (!/^\s+\S/.test(line)) break;
		const at = line.indexOf(':');
		out[line.slice(0, at).trim()] = line.slice(at + 1).trim();
	}
	return out;
}

describe('getvela.app headers', () => {
	it('the static rules are the one list', () => {
		expect(block('../../_headers', '/*')).toEqual(SITE_HEADERS);
	});

	it('deny passkeys and framing on every response', () => {
		expect(SITE_HEADERS['Permissions-Policy']).toBe(
			'publickey-credentials-get=(), publickey-credentials-create=()'
		);
		expect(SITE_HEADERS['Content-Security-Policy']).toBe("frame-ancestors 'none'");
		expect(SITE_HEADERS['X-Frame-Options']).toBe('DENY');
		expect(SITE_HEADERS['X-Content-Type-Options']).toBe('nosniff');
	});

	it("keeps a response's own CSP and adds the framing rule to it", () => {
		const own = new Headers({ 'Content-Security-Policy': "default-src 'none'" });
		applySiteHeaders(own);
		expect(own.get('Content-Security-Policy')).toBe("default-src 'none'; frame-ancestors 'none'");
		expect(own.get('X-Frame-Options')).toBe('DENY');
		const bare = new Headers();
		applySiteHeaders(bare);
		expect(bare.get('Content-Security-Policy')).toBe("frame-ancestors 'none'");
	});
});

describe('the privacy page', () => {
	const page = readFileSync(
		new URL('../routes/privacy/+page.svelte', import.meta.url),
		'utf8'
	).replace(/\s+/g, ' ');

	it('says usage statistics are off unless turned on, and never about a transaction', () => {
		expect(page).toContain('only if you turn them on');
		expect(page).toContain('They are off until you switch on');
		expect(page).toContain('Nothing about a transaction is ever sent');
		expect(page).toContain("A site's request to send a transaction is never counted.");
		// Final review: an approval is counted bare, and the page says so
		// rather than claiming nothing can be matched to a transaction.
		expect(page).toContain(
			'only the approval is counted: not what kind of request it was, and not its network'
		);
		expect(page).not.toContain('no statistic can be matched to a transaction on chain');
		// The words of the opt-out era are gone.
		expect(page).not.toContain('"send confirmed"');
		expect(page).not.toContain('turns it off on that browser');
		expect(page).not.toContain('Settings → About turns it off');
	});
});
