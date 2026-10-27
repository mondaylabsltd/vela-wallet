/**
 * The mainnet space and the testnet space (spec 105) — WEB.
 *
 * One device-wide switch: which networks the whole wallet shows. The mainnet
 * space holds networks whose assets have real value; the testnet space,
 * testnets and developer networks with nothing of real value. Every list a
 * person sees — balances and the total, assets, activity, receive, send's
 * networks, the chains a dApp can use — reads `spaceNetworksSync()`
 * (`$lib/services/networks`), never the whole list.
 *
 * Which space a stored value names is the core's to say (`networkSpaceRead`);
 * before the core has loaded, the one value it accepts besides the default is
 * read directly, so the first frame already shows the right space.
 */
import { browser } from '$app/environment';
import { loadCore, networkSpaceRead } from '$lib/core/client';
import type { NetworkSpaceMessages } from './messages';

export type NetworkSpace = 'mainnet' | 'testnet';

/** Where the space is stored: one value for the device, every wallet's. */
export const NETWORK_SPACE_KEY = 'vela.networkSpace';

function stored(): string | null {
	if (!browser) return null;
	try {
		return localStorage.getItem(NETWORK_SPACE_KEY);
	} catch {
		return null;
	}
}

class NetworkSpaceStore {
	current = $state<NetworkSpace>(stored() === 'testnet' ? 'testnet' : 'mainnet');
	/** The words, from the layout's server data (`NetworkSpaceLayer`). */
	words = $state<NetworkSpaceMessages | null>(null);
	/** The switch prompt is up (the header label, Settings → Networks). */
	switcherOpen = $state(false);

	/** Ask to switch: the prompt says what the other space is, then switches. */
	openSwitcher(): void {
		this.switcherOpen = true;
	}

	#listeners = new Set<(space: NetworkSpace) => void>();
	#booted: Promise<void> | null = null;

	/** Read the stored space through the core. Idempotent. */
	boot(): Promise<void> {
		this.#booted ??= (async () => {
			await loadCore();
			const space = networkSpaceRead(stored() ?? undefined) as NetworkSpace;
			if (space !== this.current) this.#change(space);
		})();
		return this.#booted;
	}

	/** The person's switch (through the `set_network_space` intent). */
	set(space: NetworkSpace): void {
		if (browser) {
			try {
				localStorage.setItem(NETWORK_SPACE_KEY, space);
			} catch {
				/* Blocked storage: the switch holds for this visit. */
			}
		}
		if (space !== this.current) this.#change(space);
	}

	/** Hear every switch — the network list, the balances, a dApp's chains. */
	subscribe(listener: (space: NetworkSpace) => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	get testnet(): boolean {
		return this.current === 'testnet';
	}

	#change(space: NetworkSpace): void {
		this.current = space;
		for (const listener of this.#listeners) listener(space);
	}
}

export const networkSpace = new NetworkSpaceStore();
