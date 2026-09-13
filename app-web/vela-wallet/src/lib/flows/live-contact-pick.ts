/**
 * The send flow's recipient picker, live (spec 028 US5): the book the
 * `contacts` core rules on, in the shape 021 drew for SD2e / DSD2e.
 *
 * Until now the picker inside a LIVE send showed the gallery's three fixture
 * people — a drawn promise in the middle of a real transfer. The wallet route
 * constructs its own `ContactsCore` session while a send is open (024 D8: a
 * route-scoped session, not a global ledger) and hands its view here.
 */
import { displayName } from '$lib/contacts/live';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { CHAIN_COLORS, chainColor } from '$lib/wallet/fixtures';
import { shortenAddress } from '$lib/wallet/identity';
import { fill } from '$lib/wallet/messages';
import type { WalletFlowMessages } from './messages';
import type { ContactPickModel } from './model';

export interface ContactPickLiveInputs {
	view: ContactsView;
	m: WalletFlowMessages;
	identicon: (seed: string, name?: string) => string;
}

/** The two-disc group swatch cycles the chain palette — decoration, not identity. */
const SWATCHES = Object.values(CHAIN_COLORS) as string[];

export function liveContactPick(
	model: ContactPickModel,
	inputs: ContactPickLiveInputs
): ContactPickModel {
	const { view, m, identicon } = inputs;
	return {
		...model,
		groups: view.groups.map((group, i) => ({
			name: group.name,
			count: fill(m['contacts.groupMembers'], { count: group.members.length }),
			// The palette is never empty; the fallback only satisfies the index type.
			colors: [
				SWATCHES[(2 * i) % SWATCHES.length] ?? chainColor(1),
				SWATCHES[(2 * i + 1) % SWATCHES.length] ?? chainColor(1)
			]
		})),
		contacts: view.contacts.map((contact) => ({
			name: displayName(contact),
			group: view.groups.find((g) => g.members.some((mb) => mb.address === contact.address))?.name,
			addressDisplay: shortenAddress(contact.address),
			addressFull: contact.address,
			identiconSvg: identicon(contact.address, displayName(contact))
		}))
	};
}
