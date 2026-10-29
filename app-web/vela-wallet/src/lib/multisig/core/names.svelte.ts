/**
 * Members' names: this device's own name for one of its accounts, else the
 * name the member registered for itself in the passkey index — never text
 * that came with a link or a proposal. Looked up once per address; the label
 * is the short address until (unless) a name answers.
 */
import { queryWalletName } from '$lib/services/public-key-index';
import { localName, shortenAddress } from './display';
import { memberNotes } from './member-notes.svelte';

class Names {
	#registered = $state<Record<string, string>>({});
	/** Addresses the passkey index holds: Vela accounts, whose key is a passkey. */
	#vela = $state<Record<string, true>>({});
	#asked = new Set<string>();

	/**
	 * Whether `address` is a Vela account somewhere (the passkey index holds
	 * it): such a member approves in Vela with its passkey, never through an
	 * outside wallet (ms-11). `false` until (unless) the index says so.
	 */
	isVela(address: string): boolean {
		void this.label(address);
		return this.#vela[address.toLowerCase()] === true;
	}

	/** The name to show for `address`. Starts the lookup the first time it is asked. */
	label(address: string): string {
		const local = localName(address);
		if (local) return local;
		// The name the person gave a member they added (this device's note).
		const noted = memberNotes.get(address)?.name;
		if (noted) return noted;
		const key = address.toLowerCase();
		const known = this.#registered[key];
		if (known) return known;
		if (!this.#asked.has(key)) {
			this.#asked.add(key);
			void queryWalletName(address).then((hit) => {
				if (hit) this.#vela = { ...this.#vela, [key]: true };
				if (hit?.name) this.#registered = { ...this.#registered, [key]: hit.name };
			});
		}
		return shortenAddress(address);
	}
}

export const names = new Names();
