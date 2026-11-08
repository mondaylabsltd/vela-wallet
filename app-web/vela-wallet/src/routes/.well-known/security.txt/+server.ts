/**
 * `https://wallet.getvela.app/.well-known/security.txt` (RFC 9116): where to
 * report a vulnerability. The policy behind it is the repository's
 * `SECURITY.md`.
 *
 * This is getvela.app's file
 * (`app-web/getvela.app/src/routes/.well-known/security.txt`) with this host's
 * `Canonical` line. `security.test.ts` pins this text and checks that the two
 * agree on every other line. Change both together.
 *
 * `Expires` must stay in the future, and RFC 9116 recommends less than a year
 * ahead. Renew it before it lapses; the test fails once it has.
 */
export const prerender = false;

const SECURITY_TXT = `# Vela Wallet: how to report a security vulnerability (RFC 9116).
# Please do not open public issues for security problems.
Contact: https://github.com/mondaylabsltd/vela-wallet/security/advisories/new
# The address below is being set up and is not confirmed yet. Until it is, use the link above.
Contact: mailto:security@getvela.app
Expires: 2027-11-01T00:00:00.000Z
Preferred-Languages: en, zh
Canonical: https://wallet.getvela.app/.well-known/security.txt
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
