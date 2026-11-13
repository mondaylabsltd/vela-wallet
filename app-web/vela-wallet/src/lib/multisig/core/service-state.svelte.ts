/**
 * Which multisig service this device uses.
 *
 * The address is one of the service endpoints (Settings → Service Endpoints,
 * `vela.serviceEndpoints`' `genesisServiceURL`): set, probed, refused and
 * reset by the core as the others are, and read here the way every endpoint
 * reader reads its own (`services/endpoints.ts`: the default when the record
 * holds none, or one the rule refuses). The old `vela.genesisServiceURL` key
 * is moved into the record when it is read (`loadServiceEndpoints`).
 *
 * Whether it answers — the outage, its capped back-off and the one probe —
 * is the core's (`multisig_shell::Outage`), per machine.
 */
import { SERVICE_ENDPOINTS_CHANGED, STORAGE_KEYS } from '$lib/onboarding/core/storage';
import { DEFAULT_SERVICE_ENDPOINTS, getGenesisServiceURL } from '$lib/services/endpoints';

export const DEFAULT_GENESIS_SERVICE_URL = DEFAULT_SERVICE_ENDPOINTS.genesisServiceURL;

/** `127.0.0.1:8788` for `http://127.0.0.1:8788`: what names a service in a line. */
export function serviceHost(url: string | null): string | null {
	if (url === null) return null;
	try {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a parse, read once and dropped
		return new URL(url).host || url;
	} catch {
		return url;
	}
}

class GenesisServiceSetting {
	/** Bumped whenever the endpoints are written, here or in another tab. */
	#version = $state(0);

	constructor() {
		if (typeof window === 'undefined') return;
		window.addEventListener(SERVICE_ENDPOINTS_CHANGED, () => this.changed());
		window.addEventListener('storage', (event) => {
			if (event.key === STORAGE_KEYS.serviceEndpoints || event.key === null) this.changed();
		});
	}

	private read(): string {
		try {
			return getGenesisServiceURL();
		} catch {
			return DEFAULT_GENESIS_SERVICE_URL;
		}
	}

	/** The service in use. */
	get url(): string {
		void this.#version;
		return this.read();
	}

	/** The person's own service, or `null`: the default is in use. */
	get custom(): string | null {
		const url = this.url;
		return url === DEFAULT_GENESIS_SERVICE_URL ? null : url;
	}

	private changed(): void {
		// Another service: the machines are told (`service`) and forget what
		// they knew of the last one's outage.
		this.#version += 1;
	}
}

export const genesisService = new GenesisServiceSetting();
