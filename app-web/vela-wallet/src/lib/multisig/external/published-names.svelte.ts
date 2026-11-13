/**
 * r11-6: the outside page names a member the way every Vela screen does —
 * by the name the member published for itself in the passkey index (public,
 * written with its founding key, the same answer on every device), else by
 * its short form (`names::who`). The page runs no accounts machine, so it
 * asks the index itself, once per address; the label order is the core's
 * (`names.label`, no kept or local name here: an outside owner's page holds
 * neither). A name is a hint, never authority.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { lookupWalletName } from '$lib/services/public-key-index';

/** An index lookup nobody answered is asked again after this long. */
const ASK_AGAIN_MS = 10_000;

class PublishedNames {
	#names = $state<Record<string, string>>({});
	#asked = new Set<string>();

	#ask(address: string): void {
		const key = address.toLowerCase();
		if (this.#asked.has(key)) return;
		this.#asked.add(key);
		void lookupWalletName(address)
			.then((hit) => {
				if (hit === undefined) {
					setTimeout(() => this.#asked.delete(key), ASK_AGAIN_MS);
					return;
				}
				if (hit?.name) this.#names = { ...this.#names, [key]: hit.name };
			})
			.catch(() => setTimeout(() => this.#asked.delete(key), ASK_AGAIN_MS));
	}

	/**
	 * The member's published name, as the core's label rule gives it, or
	 * `null` while none is known (the title then says the short form alone).
	 * Starts the lookup the first time.
	 */
	label(address: string): string | null {
		this.#ask(address);
		const registered = this.#names[address.toLowerCase()] ?? null;
		if (registered === null) return null;
		try {
			return JSON.parse(
				wasm.multisigLinkCall(
					'names.label',
					JSON.stringify({ address, own: null, kept: null, registered })
				)
			) as string;
		} catch {
			return registered;
		}
	}
}

export const published = new PublishedNames();
