/**
 * Members' names: this device's own name for one of its accounts, else the
 * name the member registered for itself in the passkey index — never text
 * that came with a link or a proposal. Looked up once per address; the label
 * is the short address until (unless) a name answers.
 */
import { queryWalletName } from '$lib/services/public-key-index';
import { localName, shortenAddress } from './display';

class Names {
	#registered = $state<Record<string, string>>({});
	#asked = new Set<string>();

	/** The name to show for `address`. Starts the lookup the first time it is asked. */
	label(address: string): string {
		const local = localName(address);
		if (local) return local;
		const key = address.toLowerCase();
		const known = this.#registered[key];
		if (known) return known;
		if (!this.#asked.has(key)) {
			this.#asked.add(key);
			void queryWalletName(address).then((hit) => {
				if (hit?.name) this.#registered = { ...this.#registered, [key]: hit.name };
			});
		}
		return shortenAddress(address);
	}
}

export const names = new Names();
