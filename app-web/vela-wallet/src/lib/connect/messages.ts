/**
 * Vela Connect's window — its words (spec 109). Client-safe: names keys and
 * shapes only; `resolveConnectMessages` (engine.server.ts) resolves them at
 * build time, like every other manifest here.
 *
 * The consent card and the signing sheet keep their own words (the request
 * window's, `RequestMessages`, and 026's): a site's request reads the same
 * wherever it came from. These are only what this window says around them.
 */
export const CONNECT_KEYS = {
	doneTitle: 'connect.velaConnect.doneTitle',
	doneBody: 'connect.velaConnect.doneBody',
	close: 'connect.velaConnect.close',
	goneTitle: 'connect.velaConnect.goneTitle',
	goneBody: 'connect.velaConnect.goneBody',
	directTitle: 'connect.velaConnect.directTitle',
	directBody: 'connect.velaConnect.directBody',
	openWallet: 'connect.velaConnect.openWallet',
	badLinkTitle: 'connect.velaConnect.badLinkTitle',
	badLinkBody: 'connect.velaConnect.badLinkBody',
	unverifiedTitle: 'connect.velaConnect.unverifiedTitle',
	unverifiedBody: 'connect.velaConnect.unverifiedBody',
	continueLabel: 'connect.velaConnect.continueLabel',
	returning: 'connect.velaConnect.returning',
	spaceAsk: 'connect.velaConnect.spaceAsk'
} as const;

/** Templates take `{{host}}`: the requester's host, the browser's fact; `spaceAsk` also `{{network}}`. */
export type ConnectMessages = Record<keyof typeof CONNECT_KEYS, string>;
