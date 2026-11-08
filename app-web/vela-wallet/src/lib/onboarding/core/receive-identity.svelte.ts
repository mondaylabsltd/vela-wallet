/**
 * §37 (review 1-7): whether the stored account at an address may be received
 * at — the core's `receive_standing` — and the chain-only re-check of one
 * the key index alone vouched for (`registry_resolve::confirm_step`).
 *
 * Nothing here decides. The standing is the core's answer for the record as
 * stored; the walk's requests and its verdict are the core's; what is saved
 * is the core's `account_with_identity` of the record. This module only
 * holds the reads together for the screens: Receive (its list, its code, its
 * share card, its request, its deposit watch) and every copy of the address
 * draw the standing, and nothing of the address while it is held.
 *
 * An address no stored Vela account is at (a multisig shown, the gallery)
 * has no standing: Receive as ever.
 */
import { accountWithIdentity, receiveStanding, type ReceiveStanding } from '$lib/core/kernels';
import { whenCoreLoaded } from '$lib/core/client';
import type { Account } from '../generated/Account';
import { confirmIdentity } from './registry';
import { loadAccounts, saveAccount } from './storage';

/**
 * Before the core has loaded, or for a record it cannot read: held, with
 * nothing to say yet — never an address the core has not let through.
 */
const NOT_YET: ReceiveStanding = {
	receivable: false,
	identity: 'unconfirmed',
	title_key: null,
	body_key: null,
	check_first: false
};

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

class ReceiveIdentity {
	/** Bumped on every save and once the core is here: the standings are read again. */
	#version = $state(0);
	#loaded = $state(false);
	/** Addresses (lowercase) whose walk is out now. */
	#checking = $state<Record<string, boolean>>({});
	#running = new Map<string, Promise<void>>();

	constructor() {
		void whenCoreLoaded().then(() => {
			this.#loaded = true;
		});
	}

	#account(address: string): Account | null {
		try {
			return loadAccounts().find((a) => same(a.address, address)) ?? null;
		} catch {
			return null;
		}
	}

	/**
	 * The core's standing for the stored account at `address`; `null` when no
	 * stored Vela account is there.
	 */
	standing(address: string | null | undefined): ReceiveStanding | null {
		void this.#version;
		if (!address) return null;
		const account = this.#account(address);
		if (account === null) return null;
		if (!this.#loaded) return NOT_YET;
		try {
			return receiveStanding(account) ?? NOT_YET;
		} catch {
			return NOT_YET;
		}
	}

	/** The standing when it holds the address back (`receivable` false), else `null`. */
	held(address: string | null | undefined): ReceiveStanding | null {
		const standing = this.standing(address);
		return standing !== null && !standing.receivable ? standing : null;
	}

	/** The chain is being asked about `address` now ("Checking on-chain…"). */
	checking(address: string | null | undefined): boolean {
		return !!address && this.#checking[address.toLowerCase()] === true;
	}

	/**
	 * Ask the chain about the account at `address` when the core says to
	 * check it first (`check_first`) — Receive's open, its Check again, and
	 * the app's start. One walk per account at a time; what it found, when it
	 * differs from what is stored, is saved through the core.
	 */
	check(address: string | null | undefined): Promise<void> {
		if (!address) return Promise.resolve();
		const key = address.toLowerCase();
		const out = this.#running.get(key);
		if (out) return out;
		const walk = this.#walk(address, key).finally(() => this.#running.delete(key));
		this.#running.set(key, walk);
		return walk;
	}

	async #walk(address: string, key: string): Promise<void> {
		await whenCoreLoaded();
		const account = this.#account(address);
		if (account === null) return;
		const standing = receiveStanding(account);
		if (!standing?.check_first) return;
		this.#checking = { ...this.#checking, [key]: true };
		try {
			const publicKeyHex = account.keys?.[0]?.public_key_hex || account.public_key_hex;
			const found = await confirmIdentity(publicKeyHex, account.address);
			if (found.identity === standing.identity) return;
			// The record as it is NOW (a rename may have landed meanwhile),
			// with the identity the chain gave and nothing else changed.
			const now = this.#account(address);
			const next = now === null ? null : accountWithIdentity(now, found.identity);
			if (next !== null) saveAccount(next);
			this.#version++;
		} catch (error) {
			// Nobody answered, or the store refused: it stays as it was, and
			// Check again asks once more.
			console.warn('[receive] identity check failed:', error);
		} finally {
			const { [key]: _done, ...rest } = this.#checking;
			void _done;
			this.#checking = rest;
		}
	}

	/** Test seam: read the standings again (a record written behind this module's back). */
	refresh(): void {
		this.#version++;
	}
}

export const receiveIdentity = new ReceiveIdentity();
