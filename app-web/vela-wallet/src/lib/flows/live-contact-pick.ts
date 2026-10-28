/**
 * The send flow's recipient picker, live (spec 028 US5): the book the
 * `contacts` core rules on, in the shape 021 drew for SD2e / DSD2e.
 *
 * Until now the picker inside a LIVE send showed the gallery's three fixture
 * people — a drawn promise in the middle of a real transfer. The wallet route
 * constructs its own `ContactsCore` session while a send is open (024 D8: a
 * route-scoped session, not a global ledger) and hands its view here.
 */
import { addressLine, displayName } from '$lib/contacts/live';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { CHAIN_COLORS, chainColor } from '$lib/wallet/fixtures';
import { fill } from '$lib/wallet/messages';
import { pluralForm, type PluralCopy } from '$lib/i18n/plural';
import type { WalletFlowMessages } from './messages';
import type { ContactPickModel } from './model';

export interface ContactPickLiveInputs {
	view: ContactsView;
	m: WalletFlowMessages;
	/**
	 * set-16: "1 member" / "2 members" / the locale's own forms. Absent (the
	 * gallery), the flat "{{count}} members" the fixtures draw.
	 */
	memberCount?: PluralCopy;
	identicon: (seed: string) => string;
}

/** The two-disc group swatch cycles the chain palette — decoration, not identity. */
const SWATCHES = Object.values(CHAIN_COLORS) as string[];

export function liveContactPick(
	model: ContactPickModel,
	inputs: ContactPickLiveInputs
): ContactPickModel {
	const { view, m, identicon, memberCount } = inputs;
	const members = (count: number) =>
		fill(memberCount ? pluralForm(memberCount, count) : m['contacts.groupMembers'], { count });
	return {
		...model,
		groups: view.groups.map((group, i) => ({
			name: group.name,
			count: members(group.members.length),
			// The palette is never empty; the fallback only satisfies the index type.
			colors: [
				SWATCHES[(2 * i) % SWATCHES.length] ?? chainColor(1),
				SWATCHES[(2 * i + 1) % SWATCHES.length] ?? chainColor(1)
			]
		})),
		contacts: view.contacts.map((contact) => ({
			name: displayName(contact),
			group: view.groups.find((g) => g.members.some((mb) => mb.address === contact.address))?.name,
			// An unnamed contact is titled by its short address already: the
			// line under it would only say it again (send-20). Asked of the
			// name, not by comparing spellings: the title is checksummed
			// (vis-22) and the book's address is not.
			addressDisplay: addressLine(contact),
			addressFull: contact.address,
			identiconSvg: identicon(contact.address)
		}))
	};
}

/**
 * The picker inside a live send before the book has been read: nobody.
 *
 * The board's three fixture people carry real-looking full addresses, and a
 * row answers with its address (issue 467) — so a tap on a drawn stranger
 * would put that address into a real transfer. Until the `contacts` core has
 * a view, the picker lists no one (scan and search stay); the book's people
 * arrive with it.
 */
export function unreadContactPick(model: ContactPickModel): ContactPickModel {
	return { ...model, groups: [], contacts: [] };
}
