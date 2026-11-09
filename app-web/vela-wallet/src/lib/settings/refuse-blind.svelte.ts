/**
 * R22 (review 1-3): "Refuse blind signing" — the person's own preference
 * that every request Vela can't read or check in full is refused instead of
 * offered "… anyway". OFF unless switched on (the core's decision,
 * `vela_core::prefs::refuse_blind_signing`).
 *
 * The store key and how a stored value reads are the core's
 * (`refuseBlindSigningKey()` = `vela.refuseBlindSigning`, values `on` / `off`,
 * `refuseBlindSigning(raw)`); this file keeps it in localStorage like the
 * other display-time preferences — read synchronously at the moment a
 * confirm is drawn — and hands it to the two gates that read it: the dApp
 * sheet's (`signConfirmState(… refuseBlind)`) and the multisig door
 * (`review.door {refuseBlind}`). Erase This Device sweeps the `vela.`
 * namespace, so an erased browser starts again from the default — off.
 */
import { browser } from '$app/environment';
import { refuseBlindSigning, refuseBlindSigningKey } from '$lib/core/kernels';

const ON = 'on';
const OFF = 'off';

class RefuseBlind {
	/** Bumped on every change here or in another tab / the side panel. */
	#version = $state(0);
	/** A choice made this visit, kept where storage refuses it. */
	#chosen: boolean | null = null;
	#listening = false;

	#listen(): void {
		if (this.#listening || !browser) return;
		this.#listening = true;
		window.addEventListener('storage', (event) => {
			if (event.key === null || event.key === safeKey()) this.#version += 1;
		});
	}

	/** Whether it is on now (reactive): the core's reading of the stored value. */
	current(): boolean {
		void this.#version;
		this.#listen();
		if (this.#chosen !== null) return this.#chosen;
		if (!browser) return false;
		const key = safeKey();
		if (key === null) return false;
		let raw: string | null;
		try {
			raw = localStorage.getItem(key);
		} catch {
			raw = null;
		}
		// The key came from the core, so the core is aboard to read the value.
		return refuseBlindSigning(raw);
	}

	set(on: boolean): void {
		this.#chosen = on;
		this.#version += 1;
		const key = safeKey();
		if (!browser || key === null) return;
		try {
			localStorage.setItem(key, on ? ON : OFF);
		} catch {
			/* Blocked storage: the choice holds for this visit (`#chosen`). */
		}
	}
}

/** The core's key, or `null` before the core is aboard. */
function safeKey(): string | null {
	try {
		return refuseBlindSigningKey();
	} catch {
		return null;
	}
}

export const refuseBlind = new RefuseBlind();
