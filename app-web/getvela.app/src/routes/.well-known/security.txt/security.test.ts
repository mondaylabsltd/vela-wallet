/**
 * getvela.app's security.txt is pinned: a change to where reports go is a
 * deliberate edit here, never drift.
 */
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

const EXPECTED = `# Vela Wallet: how to report a security vulnerability (RFC 9116).
# Please do not open public issues for security problems.
Contact: https://github.com/mondaylabsltd/vela-wallet/security/advisories/new
# The address below is being set up and is not confirmed yet. Until it is, use the link above.
Contact: mailto:security@getvela.app
Expires: 2027-11-01T00:00:00.000Z
Preferred-Languages: en, zh
Canonical: https://getvela.app/.well-known/security.txt
Policy: https://github.com/mondaylabsltd/vela-wallet/blob/main/SECURITY.md
`;

/** The `Name: value` lines, comments dropped (RFC 9116 §2.3). */
function fields(body: string): [string, string][] {
	return body
		.split('\n')
		.filter((line) => line && !line.startsWith('#'))
		.map((line) => {
			const at = line.indexOf(': ');
			return [line.slice(0, at), line.slice(at + 2)];
		});
}

describe('/.well-known/security.txt (getvela.app)', () => {
	it('is served as text/plain in UTF-8', () => {
		const response = GET();
		expect(response.status).toBe(200);
		expect(response.headers.get('content-type')).toBe('text/plain; charset=utf-8');
	});

	it('says exactly where to report', async () => {
		expect(await GET().text()).toBe(EXPECTED);
	});

	it('names GitHub private reporting first, and the mailbox second', async () => {
		const contacts = fields(await GET().text())
			.filter(([name]) => name === 'Contact')
			.map(([, value]) => value);
		expect(contacts).toEqual([
			'https://github.com/mondaylabsltd/vela-wallet/security/advisories/new',
			'mailto:security@getvela.app'
		]);
	});

	it('has each RFC 9116 single-use field once, and Expires and Canonical as the RFC requires', async () => {
		const all = fields(await GET().text());
		for (const name of ['Expires', 'Preferred-Languages', 'Canonical', 'Policy']) {
			expect(all.filter(([n]) => n === name)).toHaveLength(1);
		}
		const canonical = all.find(([n]) => n === 'Canonical')?.[1];
		expect(canonical).toBe('https://getvela.app/.well-known/security.txt');
	});

	it('has not expired: renew Expires (and this test) before it lapses', async () => {
		const expires = fields(await GET().text()).find(([n]) => n === 'Expires')?.[1] ?? '';
		const at = Date.parse(expires);
		expect(Number.isNaN(at)).toBe(false);
		expect(at).toBeGreaterThan(Date.now());
	});
});
