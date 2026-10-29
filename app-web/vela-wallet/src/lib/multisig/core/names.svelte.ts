/**
 * Members' names: this device's own name for one of its accounts, else the
 * name a multisig on this device keeps for it (typed by its creator, or
 * carried by an invite — r4D-5), else the name the member registered for
 * itself in the passkey index. Labels only, never authority: who may act is
 * the genesis's and each chain's owners. Looked up once per address; the
 * label is the short address until (unless) a name answers.
 */
import { queryWalletName } from '$lib/services/public-key-index';
import { localName, shortenAddress } from './display';
import { multisig, type MemberInfo } from './store.svelte';

/** What the multisigs on this device keep of `address` (the first that names it). */
export function keptMember(address: string): MemberInfo {
	const key = address.toLowerCase();
	const out: MemberInfo = {};
	for (const account of multisig.accounts) {
		const info = account.members?.[key];
		if (!info) continue;
		out.name ??= info.name;
		out.kind ??= info.kind;
	}
	return out;
}

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

	/**
	 * What `address` is, as far as this device can tell (r4D-5): a Vela
	 * account (an account here, or the passkey index holds it), what a
	 * multisig here keeps it as, or `null`: not known (the index's silence is
	 * no answer — it may only have been out of reach).
	 */
	kindOf(address: string): 'passkey' | 'wallet' | null {
		if (localName(address)) return 'passkey';
		void this.label(address);
		const key = address.toLowerCase();
		if (this.#vela[key]) return 'passkey';
		return keptMember(address).kind ?? null;
	}

	/** The name to show for `address`. Starts the lookup the first time it is asked. */
	label(address: string): string {
		const local = localName(address);
		if (local) return local;
		const key = address.toLowerCase();
		const kept = keptMember(address).name;
		const known = this.#registered[key];
		if (!this.#asked.has(key)) {
			this.#asked.add(key);
			void queryWalletName(address).then((hit) => {
				if (hit) this.#vela = { ...this.#vela, [key]: true };
				if (hit?.name) this.#registered = { ...this.#registered, [key]: hit.name };
			});
		}
		return kept ?? known ?? shortenAddress(address);
	}
}

export const names = new Names();
