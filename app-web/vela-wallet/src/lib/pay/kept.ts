/**
 * A payment link waiting for the wallet (spec 039 Part B).
 *
 * `/pay` keeps the link's query here and goes to the wallet, which takes it
 * once it knows whose wallet it is and opens Send on it. One door for both
 * cases the page meets: a wallet already on this device (taken at once), and
 * none yet — kept while the person creates one or signs in, and taken when
 * the wallet first opens, so the request is not lost to the trip through
 * Welcome (the multisig link's r4b-9 rule, for payments).
 *
 * The RAW query is kept, never a ruling on it: the wallet asks the core
 * again, so what opens Send is always the core's reading of the link.
 *
 * An hour, not the multisig link's day. A kept multisig link opens a page to
 * look at; a kept payment opens a Send — a request found on a device the next
 * morning is a surprise, and an hour is enough to create a wallet or sign in.
 * Taken once. Nothing secret: a link anyone could have read in the chat.
 */
import type { RawPayQuery } from '$lib/flows/core/payment-request';
import { PAY_PARAMS } from './link';

export const KEPT_PAY_KEY = 'vela.payKeptLink';
export const KEPT_PAY_MS = 60 * 60_000;

/** Keep `query` for the wallet. Whether it was kept (storage can refuse). */
export function keepPay(query: RawPayQuery, now = Date.now()): boolean {
	try {
		localStorage.setItem(KEPT_PAY_KEY, JSON.stringify({ query, at: now }));
		return true;
	} catch {
		return false;
	}
}

/** The kept query, once: `null` when none is, it waited too long, or it is not one. */
export function takeKeptPay(now = Date.now()): RawPayQuery | null {
	let kept: { query?: unknown; at?: unknown } | null;
	try {
		kept = JSON.parse(localStorage.getItem(KEPT_PAY_KEY) ?? 'null') as typeof kept;
		localStorage.removeItem(KEPT_PAY_KEY);
	} catch {
		return null;
	}
	if (!kept || typeof kept.at !== 'number' || typeof kept.query !== 'object' || !kept.query) {
		return null;
	}
	// A clock set back by more than a minute reads as "not this visit's".
	if (now - kept.at > KEPT_PAY_MS || now < kept.at - 60_000) return null;
	const stored = kept.query as Record<string, unknown>;
	const query: RawPayQuery = {};
	for (const name of PAY_PARAMS) {
		const value = stored[name];
		if (typeof value === 'string') query[name] = value;
	}
	return query;
}
