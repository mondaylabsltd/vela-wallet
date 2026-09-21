/**
 * What the Explore start page's scan button opens (issue 273).
 *
 * The founder's ruling: the button reads a **web address** and opens it in
 * this browser. Nothing else — a WalletConnect pairing code is not something
 * this wallet speaks, and a payment code belongs to send. Both are refused in
 * one line rather than "opened" as a page called `https://wc` or `https://0x…`,
 * which is what reading them as typed text would do.
 *
 * The same rule as iOS's `ExploreScan.url(from:)`.
 */

/**
 * The address a scanned payload opens, or `null` when it is not a web address.
 *
 * Accepted: an `http(s)://` URL with a host, and a bare domain
 * (`app.uniswap.org/swap`), which the search field would also open. Refused:
 * every other scheme (`wc:`, `ethereum:`), a bare account address, and
 * anything with whitespace in it.
 */
export function exploreScanUrl(payload: string): string | null {
	const text = payload.trim();
	if (text === '' || /\s/.test(text)) return null;

	const scheme = /^(https?):\/\//i.exec(text);
	if (scheme) {
		const url = `${scheme[1].toLowerCase()}://${text.slice(scheme[0].length)}`;
		return parse(url)?.hostname ? url : null;
	}

	// A bare domain, read as the search field reads it — `https://` in front —
	// and kept only when a real host name comes out: `ethereum:0x…` and
	// `wc:…@2` parse as a user name in front of a host, and fall out here.
	const bare = parse(`https://${text}`);
	if (!bare || bare.username || bare.password || !isDomain(bare.hostname)) return null;
	return `https://${text}`;
}

function parse(url: string): URL | null {
	try {
		return new URL(url);
	} catch {
		return null;
	}
}

/** Dot-separated labels of letters, digits and hyphens, ending in a letters-only label. */
function isDomain(host: string): boolean {
	const labels = host.split('.');
	const tld = labels[labels.length - 1];
	return (
		labels.length >= 2 &&
		/^[a-z]{2,}$/i.test(tld) &&
		labels.every((label) => /^[a-z0-9-]+$/i.test(label))
	);
}
