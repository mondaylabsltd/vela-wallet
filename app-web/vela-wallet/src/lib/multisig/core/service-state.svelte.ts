/**
 * The multisig service's live state: which service this device uses, and
 * whether it is answering.
 *
 * The address is one of the service endpoints (Settings → Service Endpoints,
 * `vela.serviceEndpoints`' `genesisServiceURL`): set, probed, refused and
 * reset by the core as the others are, and read here the way every endpoint
 * reader reads its own (`services/endpoints.ts`: the default when the record
 * holds none, or one the rule refuses). The old `vela.genesisServiceURL` key
 * is moved into the record when it is read (`loadServiceEndpoints`).
 *
 * The outage is the whole service's, not a network's: the first request that
 * finds nothing answering (refused, closed, offline, timed out) marks it
 * away, and background reads then ask nothing until the next step of a
 * capped schedule ({@link RETRY_STEPS_MS}), when ONE probe goes first. Any
 * answer, `online`, a new address and the person's Retry end it at once.
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
	/** The service last read: a write that changes it ends what was known of it. */
	#seen: string;

	constructor() {
		this.#seen = this.read();
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
		const now = this.read();
		// Another service: what was known about the last one says nothing.
		if (now !== this.#seen) serviceOutage.reset();
		this.#seen = now;
		this.#version += 1;
	}
}

/**
 * How long background reads wait after the service was found away, by how
 * many probes in a row found it so: capped, so a service that comes back is
 * found within five minutes without anyone asking.
 */
export const RETRY_STEPS_MS = [15_000, 30_000, 60_000, 120_000, 300_000] as const;

class ServiceOutage {
	/** Nothing answered the last request or probe: the service is away. */
	down = $state(false);
	#failures = 0;
	#retryAt = 0;
	#probe: Promise<boolean> | null = null;

	constructor() {
		if (typeof window === 'undefined') return;
		window.addEventListener('online', () => this.reset());
	}

	/** Away, and its next step has not come: background reads ask nothing. */
	blocked(now = Date.now()): boolean {
		return this.down && now < this.#retryAt;
	}

	/** When background reads may ask again (`0` when they may now). */
	get retryAt(): number {
		return this.down ? this.#retryAt : 0;
	}

	/** Nothing answered. Requests already out when it was marked count once. */
	failed(now = Date.now()): void {
		if (this.blocked(now)) return;
		this.#failures += 1;
		this.down = true;
		const step = RETRY_STEPS_MS[Math.min(this.#failures, RETRY_STEPS_MS.length) - 1];
		this.#retryAt = now + step;
	}

	/** It answered (whatever it said): back to asking as usual. */
	answered(): void {
		if (this.down || this.#failures > 0) this.reset();
	}

	reset(): void {
		this.down = false;
		this.#failures = 0;
		this.#retryAt = 0;
	}

	/**
	 * For a round of background reads: whether to make them. While the
	 * service is away, `false` until its next step, then one `probe` that
	 * every caller of that round shares.
	 */
	available(probe: () => Promise<boolean>): Promise<boolean> {
		if (!this.down) return Promise.resolve(true);
		if (this.blocked()) return Promise.resolve(false);
		this.#probe ??= probe()
			.catch(() => false)
			.then((up) => {
				if (up) this.answered();
				else this.failed();
				return up;
			})
			.finally(() => (this.#probe = null));
		return this.#probe;
	}
}

export const serviceOutage = new ServiceOutage();
export const genesisService = new GenesisServiceSetting();
