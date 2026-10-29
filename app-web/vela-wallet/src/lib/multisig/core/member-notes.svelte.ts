/**
 * The person's own name for a member they added, and what kind of member it
 * is — a Vela account (approves with its passkey, in Vela) or an outside
 * wallet (MetaMask, a hardware wallet: signs through the connect page). A
 * note on THIS device, like a contact's name: never sent with a proposal,
 * never taken from one (docs/multisig-vault-design.md §5.3 — names are only
 * ever this device's own, or what a member registered for itself).
 */
const KEY = 'vela.multisigMemberNotes';

export type MemberKind = 'vela' | 'wallet';

export interface MemberNote {
	name: string;
	kind: MemberKind;
}

function read(): Record<string, MemberNote> {
	try {
		const raw = localStorage.getItem(KEY);
		const parsed = raw ? (JSON.parse(raw) as unknown) : null;
		return parsed && typeof parsed === 'object' ? (parsed as Record<string, MemberNote>) : {};
	} catch {
		return {};
	}
}

class MemberNotes {
	/**
	 * Read once, when the module loads — never lazily from a read: a name is
	 * asked for while the page draws, where no state may be written.
	 */
	#notes = $state<Record<string, MemberNote>>(typeof localStorage === 'undefined' ? {} : read());

	#all(): Record<string, MemberNote> {
		return this.#notes;
	}

	get(address: string): MemberNote | null {
		return this.#all()[address.toLowerCase()] ?? null;
	}

	/** Keep `note` for `address` (an empty name keeps only the kind). */
	set(address: string, note: MemberNote): void {
		const next = { ...this.#all(), [address.toLowerCase()]: { ...note, name: note.name.trim() } };
		this.#notes = next;
		try {
			localStorage.setItem(KEY, JSON.stringify(next));
		} catch {
			// best effort: the note stands for this page
		}
	}
}

export const memberNotes = new MemberNotes();
