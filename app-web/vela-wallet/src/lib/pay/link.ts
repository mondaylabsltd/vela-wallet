/**
 * A payment link — `/pay?to=…&chain=…&token=…&amount=…` (spec 039 Part B).
 *
 * The link is the receive screen's (`build_pay_link`): shared in a chat,
 * opened in a browser, landing here. Everything it says is untrusted, and
 * NOTHING in this module rules on it. Whether a link is a request this wallet
 * can honour is `payment_request`'s (`link_opened` → `pay_valid`, and
 * `pay_refusal` for why not) — the same machine iOS, Android and the desktop
 * read their `/pay` links through, so a link refused on one is refused on all.
 * What happens to the request after that is Send's: a network this wallet
 * lacks and a token nobody can describe are the send core's `lock_error`.
 *
 * This module only reads the query, words a refusal from the corpus, and puts
 * an accepted request into the send's open params. Pure; no wasm.
 */
import type { PayRefusal } from '$lib/core/generated/PayRefusal';
import type { PayRequest } from '$lib/core/generated/PayRequest';
import type { SendOpenParams } from '$lib/core/generated/SendOpenParams';
import type { RawPayQuery } from '$lib/flows/core/payment-request';

/** The query parameters a pay link carries — the builder's, in its order. */
export const PAY_PARAMS = ['to', 'chain', 'token', 'amount', 'sym', 'dec', 'net'] as const;

/**
 * The link's query as the core takes it. A parameter given twice is read as
 * its FIRST value — the one `URLSearchParams.get` and the desktop's
 * `query_pairs().find` both read — so no client sees a different request.
 */
export function payQueryOf(search: string): RawPayQuery {
	const params = new URLSearchParams(search);
	const query: RawPayQuery = {};
	for (const name of PAY_PARAMS) {
		const value = params.get(name);
		if (value !== null) query[name] = value;
	}
	return query;
}

/** The pay page's words, resolved at build time (`resolvePayMessages`). */
export interface PayMessages {
	/** "Payment request" — the page's title while it reads, and with no wallet. */
	title: string;
	invalidTitle: string;
	/** A recipient or network the core refused: the corpus's one sentence for both. */
	invalidBody: string;
	invalidToken: string;
	invalidAmount: string;
	noWalletBody: string;
	/** The recipient row's label on the no-wallet screen. */
	recipient: string;
	/** The way home from a refused link. */
	home: string;
	/** Welcome's two ways in, in Welcome's own words. */
	createWallet: string;
	alreadyHaveWallet: string;
}

export const PAY_MESSAGE_KEYS = {
	title: 'receive.pay.title',
	invalidTitle: 'receive.pay.invalidTitle',
	invalidBody: 'receive.pay.invalidBody',
	invalidToken: 'receive.pay.invalidToken',
	invalidAmount: 'receive.pay.invalidAmount',
	noWalletBody: 'receive.pay.noWalletBody',
	recipient: 'receive.pay.recipient',
	home: 'receive.pay.home',
	createWallet: 'onboarding.welcome.createWallet',
	alreadyHaveWallet: 'onboarding.welcome.alreadyHaveWallet'
} as const satisfies Record<keyof PayMessages, string>;

/** Why the core refused the link, in the person's language. */
export function refusalBody(refusal: PayRefusal | null, m: PayMessages): string {
	switch (refusal) {
		case 'token':
			return m.invalidToken;
		case 'amount':
			return m.invalidAmount;
		// Recipient, network — and a refusal with no reason, which the core
		// never sends; the general sentence is still true of it.
		default:
			return m.invalidBody;
	}
}

/**
 * An accepted request as the send opens on it: locked to the link's network,
 * token and amount — the very params a scanned `ethereum:` code with a chain
 * opens Send with (`handleScan`), so a link and a code of the same request
 * land on the same screen, as they do on iOS. Locked is not signed: the
 * person still reviews, and nothing is sent until they confirm.
 */
export function payPrefill(pay: PayRequest): Partial<SendOpenParams> {
	return {
		prefilled_recipient: pay.recipient,
		prefilled_chain_id: String(pay.chain_id),
		prefilled_token_address: pay.token_address,
		prefilled_amount_base: pay.amount_base,
		locked: true
	};
}
