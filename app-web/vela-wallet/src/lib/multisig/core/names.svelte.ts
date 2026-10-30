/**
 * Members' names — the core's rule (`multisig_rules::names`): this device's
 * own name for one of its accounts, else the name a multisig on this device
 * keeps for it (the accounts machine's `known_members`), else the name the
 * member registered for itself in the passkey index, else the short address.
 * Labels only, never authority. Here only the passkey index is asked (once
 * per address) and its answers kept, reactively, for the rule.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { lookupWalletName } from '$lib/services/public-key-index';
import { localName, memberLabel } from './display';
import { multisig, type MemberInfo } from './store.svelte';

/** An index lookup nobody answered is asked again after this long, not on every redraw. */
const ASK_AGAIN_MS = 10_000;

function rule<T>(method: string, args: Record<string, unknown>): T {
	return JSON.parse(wasm.multisigLinkCall(`names.${method}`, JSON.stringify(args))) as T;
}

/** What the multisigs on this device keep of `address` (the machine's `known_members`). */
export function keptMember(address: string): MemberInfo {
	const kept = multisig.view?.known_members[address.toLowerCase()];
	return kept ? (kept as MemberInfo) : {};
}

class Names {
	#registered = $state<Record<string, string>>({});
	/** Addresses the passkey index holds: Vela accounts, whose key is a passkey. */
	#vela = $state<Record<string, true>>({});
	/** Addresses the index has answered for (a hit or a miss). */
	#answered = $state<Record<string, true>>({});
	#asked = new Set<string>();

	/** Ask the passkey index about `address`, once (again a while after no answer). */
	#ask(address: string): void {
		const key = address.toLowerCase();
		if (this.#asked.has(key)) return;
		this.#asked.add(key);
		void lookupWalletName(address).then((hit) => {
			// r4a-9: nobody answered — no verdict; asked again a while later,
			// and meanwhile `answered` stays false (nothing offered on it).
			if (hit === undefined) {
				setTimeout(() => this.#asked.delete(key), ASK_AGAIN_MS);
				return;
			}
			this.#answered = { ...this.#answered, [key]: true };
			if (hit) this.#vela = { ...this.#vela, [key]: true };
			if (hit?.name) this.#registered = { ...this.#registered, [key]: hit.name };
		});
	}

	/** Whether the passkey index has answered for `address` yet (so `isVela` is its word). */
	answered(address: string): boolean {
		this.#ask(address);
		return this.#answered[address.toLowerCase()] === true;
	}

	/**
	 * Whether `address` is a Vela account somewhere (the passkey index holds
	 * it). `false` until (unless) the index says so.
	 */
	isVela(address: string): boolean {
		this.#ask(address);
		return this.#vela[address.toLowerCase()] === true;
	}

	/** What `address` is, as far as this device can tell (the core's rule); `null`: not known. */
	kindOf(address: string): NonNullable<MemberInfo['kind']> | null {
		this.#ask(address);
		if (!multisig.view) return null;
		return rule<NonNullable<MemberInfo['kind']> | null>('kind', {
			own: localName(address) !== null,
			vela: this.#vela[address.toLowerCase()] === true,
			kept: keptMember(address).kind ?? null
		});
	}

	/** The name to show for `address` (the core's rule). Starts the lookup the first time. */
	label(address: string): string {
		this.#ask(address);
		// Before the core is here: this device's own name, else the short address.
		if (!multisig.view) return memberLabel(address);
		return rule<string>('label', {
			address,
			own: localName(address),
			kept: keptMember(address).name ?? null,
			registered: this.#registered[address.toLowerCase()] ?? null
		});
	}
}

export const names = new Names();
