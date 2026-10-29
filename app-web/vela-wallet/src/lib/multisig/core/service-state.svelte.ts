/**
 * The multisig service's live state: which service this device uses
 * (Settings → Multisig service), and whether it is answering.
 *
 * The address is the person's (or a test's) own in `vela.genesisServiceURL`,
 * used only when the core's rule admits it (`serviceAddress`: `https://`, or
 * plain `http://` to this computer) — one stored by hand that the rule
 * refuses is not used, and Settings shows the default, which is what is.
 *
 * The outage is the whole service's, not a network's: the first request that
 * finds nothing answering (refused, closed, offline, timed out) marks it
 * away, and background reads then ask nothing until the next step of a
 * capped schedule ({@link RETRY_STEPS_MS}), when ONE probe goes first. Any
 * answer, `online`, a new address and the person's Retry end it at once.
 */
import { serviceAddress } from './kernel';

export const DEFAULT_GENESIS_SERVICE_URL = 'https://safe-genesis.getvela.app';
/** The person's own service (Settings), a developer's or a test's; unset means the default. */
export const GENESIS_SERVICE_KEY = 'vela.genesisServiceURL';

/** The stored address, as the core's rule admits it; `null` for none or a refused one. */
export function admittedServiceURL(raw: string | null): string | null {
	if (raw === null || raw.trim() === '') return null;
	try {
		const answer = serviceAddress(raw);
		return answer.kind === 'ok' ? answer.url : null;
	} catch {
		// The core is not loaded yet (nothing it re-verifies can run either):
		// the stored address's shape, as before the setting existed. Settings
		// writes only what the core admitted.
		const url = raw.trim().replace(/\/+$/, '');
		return /^https?:\/\/[^\s?#]+$/.test(url) ? url : null;
	}
}

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
	/** Bumped on every write here or in another tab: what reads `custom` reads again. */
	#version = $state(0);

	constructor() {
		if (typeof window === 'undefined') return;
		window.addEventListener('storage', (event) => {
			if (event.key === GENESIS_SERVICE_KEY || event.key === null) this.changed();
		});
	}

	/** The person's own service, or `null`: the default is used. */
	get custom(): string | null {
		void this.#version;
		try {
			return admittedServiceURL(localStorage.getItem(GENESIS_SERVICE_KEY));
		} catch {
			return null;
		}
	}

	/** The service in use. */
	get url(): string {
		return this.custom ?? DEFAULT_GENESIS_SERVICE_URL;
	}

	/** Use `url` (already admitted by the core), or the default (`null`). */
	set(url: string | null): void {
		try {
			if (url === null) localStorage.removeItem(GENESIS_SERVICE_KEY);
			else localStorage.setItem(GENESIS_SERVICE_KEY, url);
		} catch {
			// storage unavailable: nothing to keep it in
		}
		this.changed();
	}

	private changed(): void {
		this.#version += 1;
		// Another service: what was known about the last one says nothing.
		serviceOutage.reset();
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
