/**
 * wallet.getvela.app's security.txt is pinned, and it is held equal to
 * getvela.app's on every line but `Canonical`, so the two hosts never send
 * reporters to different places.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

const EXPECTED = `# Vela Wallet: how to report a security vulnerability (RFC 9116).
# Please do not open public issues for security problems.
Contact: https://github.com/mondaylabsltd/vela-wallet/security/advisories/new
# The address below is being set up and is not confirmed yet. Until it is, use the link above.
Contact: mailto:security@getvela.app
Expires: 2027-11-01T00:00:00.000Z
Preferred-Languages: en, zh
Canonical: https://wallet.getvela.app/.well-known/security.txt
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

/** getvela.app's served text, read from its route's source. */
function siteText(): string {
	const source = readFileSync(
		fileURLToPath(
			new URL(
				'../../../../../getvela.app/src/routes/.well-known/security.txt/+server.ts',
				import.meta.url
			)
		),
		'utf8'
	);
	const match = /const SECURITY_TXT = `([^`]*)`;/.exec(source);
	if (!match) throw new Error('getvela.app security.txt route has no SECURITY_TXT literal');
	return match[1];
}

describe('/.well-known/security.txt (wallet.getvela.app)', () => {
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

	it('is canonical for this host', async () => {
		const all = fields(await GET().text());
		for (const name of ['Expires', 'Preferred-Languages', 'Canonical', 'Policy']) {
			expect(all.filter(([n]) => n === name)).toHaveLength(1);
		}
		expect(all.find(([n]) => n === 'Canonical')?.[1]).toBe(
			'https://wallet.getvela.app/.well-known/security.txt'
		);
	});

	it('matches getvela.app on every line but Canonical', async () => {
		const ours = (await GET().text()).replace(
			'Canonical: https://wallet.getvela.app/',
			'Canonical: https://getvela.app/'
		);
		expect(ours).toBe(siteText());
	});

	it('has not expired: renew Expires (and this test) before it lapses', async () => {
		const expires = fields(await GET().text()).find(([n]) => n === 'Expires')?.[1] ?? '';
		const at = Date.parse(expires);
		expect(Number.isNaN(at)).toBe(false);
		expect(at).toBeGreaterThan(Date.now());
	});
});
