/**
 * How the address book hands a person to the money flows (spec 028 US5).
 *
 * 转账 / 收款 / 群发转账 on a contact or group live on /contacts; the send and
 * receive flows live on /wallet, as pushed screens inside that one route
 * (spec 021). The hand-off is the URL — `?to=` for a recipient, `?group=` for
 * a batch, `?flow=receive` for the receive card — because a URL is the one
 * carrier both routes already share, survives a reload mid-way, and is what
 * the phone's deep links (`prefilled_recipient`) already are. The wallet
 * route reads it once, opens the flow, and the core is told exactly what a
 * scanned code would have told it.
 */

export type FlowHandoff =
	| { kind: 'send'; recipient: string }
	| { kind: 'receive' }
	| { kind: 'group-send'; groupId: string };

const ADDRESS = /^0x[0-9a-fA-F]{40}$/;

/** The hand-off a wallet URL carries, or `null` for a plain visit. */
export function readFlowHandoff(search: string): FlowHandoff | null {
	const params = new URLSearchParams(search);
	const to = params.get('to');
	if (to !== null && ADDRESS.test(to)) return { kind: 'send', recipient: to };
	const group = params.get('group');
	if (group !== null && group !== '') return { kind: 'group-send', groupId: group };
	if (params.get('flow') === 'receive') return { kind: 'receive' };
	return null;
}

/**
 * The query that carries `handoff`. Typed as a `?…` literal so it can ride
 * INSIDE `resolve('/[locale]/wallet?…')` — SvelteKit's `resolve` accepts a
 * route id with a search part, and the navigation lint accepts nothing else.
 */
export function flowHandoffQuery(handoff: FlowHandoff): `?${string}` {
	const params = new URLSearchParams();
	switch (handoff.kind) {
		case 'send':
			params.set('to', handoff.recipient);
			break;
		case 'group-send':
			params.set('group', handoff.groupId);
			break;
		case 'receive':
			params.set('flow', 'receive');
			break;
	}
	return `?${params.toString()}`;
}
