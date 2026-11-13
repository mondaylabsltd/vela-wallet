/**
 * Which addresses the wallet may fetch or link to — the draw-time guard.
 *
 * The rule is the core's (`network_admin::is_allowed_endpoint_url`): an
 * `https://` address with a real host, or plain `http://` to this computer
 * (localhost, 127.0.0.1, [::1]). Settings refuses anything else when it is
 * typed. This guards the OTHER way in: bytes already in storage — written
 * before that gate existed, by another client, or by hand — which the
 * service readers and the explorer links would otherwise use as they are. A
 * `javascript:` explorer was one tap from running as the `href` of "View on
 * explorer". Read synchronously, so it cannot wait for the wasm: the cases
 * are pinned in `url-policy.test.ts` against the core's own test list.
 */

const LOOPBACK = /^http:\/\/(localhost|127\.0\.0\.1|\[::1\])(:\d+)?(\/|$)/i;

export function isAllowedEndpointURL(url: string): boolean {
	// eslint-disable-next-line no-control-regex -- CR/LF and friends are exactly what is refused
	if (/[\u0000-\u001f\u007f-\u009f]/.test(url)) return false;
	if (LOOPBACK.test(url)) return true;
	if (!/^https:\/\//i.test(url)) return false;
	const authority = url.slice('https://'.length).split(/[/?#]/)[0] ?? '';
	const hostPort = authority.slice(authority.lastIndexOf('@') + 1);
	let host: string;
	let port: string | undefined;
	if (hostPort.startsWith('[')) {
		const close = hostPort.indexOf(']');
		if (close < 0) return false;
		host = hostPort.slice(1, close);
		if (host === '' || !/^[0-9a-f:]+$/i.test(host)) return false;
		const after = hostPort.slice(close + 1);
		if (after !== '' && !after.startsWith(':')) return false;
		port = after === '' ? undefined : after.slice(1);
	} else {
		const colon = hostPort.indexOf(':');
		host = colon < 0 ? hostPort : hostPort.slice(0, colon);
		port = colon < 0 ? undefined : hostPort.slice(colon + 1);
		if (host === '' || host.startsWith('.') || !/^[\p{L}\p{N}._-]+$/u.test(host)) return false;
	}
	return port === undefined || /^\d+$/.test(port);
}

/** `url` when the rule allows it, else `null` — for a reader that falls back. */
export function allowedOrNull(url: string | null | undefined): string | null {
	if (typeof url !== 'string') return null;
	const trimmed = url.trim();
	return trimmed !== '' && isAllowedEndpointURL(trimmed) ? trimmed : null;
}
