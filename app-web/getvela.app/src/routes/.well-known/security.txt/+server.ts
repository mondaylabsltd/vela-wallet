/**
 * `https://getvela.app/.well-known/security.txt` (RFC 9116): where to report a
 * vulnerability. The policy behind it is the repository's `SECURITY.md`.
 *
 * `security.test.ts` pins this text exactly, and the web wallet serves the same
 * file except for its `Canonical` line
 * (`app-web/vela-wallet/src/routes/.well-known/security.txt`). Change both
 * together.
 *
 * - Contact order is preference order: GitHub private vulnerability reporting
 *   first. The mailbox is listed, but it is not confirmed yet (go-live item).
 * - `Expires` must stay in the future, and RFC 9116 recommends less than a year
 *   ahead. Renew it before it lapses; the test fails once it has.
 */
export const prerender = false;

const SECURITY_TXT = `# Vela Wallet: how to report a security vulnerability (RFC 9116).
# Please do not open public issues for security problems.
Contact: https://github.com/mondaylabsltd/vela-wallet/security/advisories/new
# The address below is being set up and is not confirmed yet. Until it is, use the link above.
Contact: mailto:security@getvela.app
Expires: 2027-11-01T00:00:00.000Z
Preferred-Languages: en, zh
Canonical: https://getvela.app/.well-known/security.txt
Policy: https://github.com/mondaylabsltd/vela-wallet/blob/main/SECURITY.md
`;

export function GET(): Response {
	return new Response(SECURITY_TXT, {
		status: 200,
		headers: {
			'content-type': 'text/plain; charset=utf-8',
			'cache-control': 'public, max-age=3600'
		}
	});
}
