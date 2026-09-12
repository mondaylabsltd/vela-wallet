/**
 * The destinations THIS client has (spec 022 founder call).
 *
 * 探索 is the in-app dApp browser, and this client already lives inside a
 * browser: a page cannot host another site's dApp with a wallet injected into
 * it, so there is nothing behind that tab here. The native clients have it;
 * the web shows three tabs rather than a fourth that opens nothing. The
 * explore vocabulary still ships — the gallery boards are the design source
 * all four clients are reviewed against, and they draw all four.
 *
 * ONE list, read by every route's tab bar and sidebar. The wallet route used
 * to keep its own copy and the other two routes none, which is how 探索 came
 * back on 通讯录 and 设置 while the wallet had dropped it.
 */
export const WEB_DESTINATIONS = ['wallet', 'contacts', 'settings'] as const;

export type WebDestination = (typeof WEB_DESTINATIONS)[number];

/** A drawn nav (four items) reduced to the destinations the web has. */
export function webNavItems<T extends { id: string }>(nav: readonly T[]): T[] {
	return nav.filter((item) => (WEB_DESTINATIONS as readonly string[]).includes(item.id));
}
