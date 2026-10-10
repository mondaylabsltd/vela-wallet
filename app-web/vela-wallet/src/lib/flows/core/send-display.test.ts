/**
 * The display currency, as the send machine is told it (0.8, `send_form`).
 *
 * While `CurrencyView.committed` is false the view is the USD/1 placeholder —
 * not the person's currency — and no fiat figure may be drawn or TYPED in it.
 * The machine was handed that placeholder as a real pair and never told when
 * the real one landed. This pins the rule the page now sends, and — against
 * the REAL send machine — what it does with it.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { SendCore } from '$lib/core/client';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { chosenCurrencyCode, sendDisplayContext } from './send-display';
import { sendTokenId } from './send-types';

/** The three views the display-currency machine can emit before and after a commit. */
const FIRST: CurrencyView = { code: 'USD', rate: 1, committed: false, pending: null };
const ON_ITS_WAY: CurrencyView = { code: 'USD', rate: 1, committed: false, pending: 'CNY' };
const CNY: CurrencyView = { code: 'CNY', rate: 7.1, committed: true, pending: null };
const USD: CurrencyView = { code: 'USD', rate: 1, committed: true, pending: null };
const UNPRICED: CurrencyView = { code: 'VND', rate: null, committed: true, pending: null };

describe('sendDisplayContext', () => {
	it('not committed: no rate — the placeholder’s "1" is never handed over as a price', () => {
		expect(sendDisplayContext(FIRST)).toEqual({ code: 'USD', rate: null, fiat_decimals: 2 });
		// The stored choice on its way names the code; still nothing to divide by.
		expect(sendDisplayContext(ON_ITS_WAY)).toEqual({ code: 'CNY', rate: null, fiat_decimals: 2 });
	});

	it('committed: the pair as it is — a real 1 for USD, and "cannot price" kept as that', () => {
		expect(sendDisplayContext(CNY)).toEqual({ code: 'CNY', rate: 7.1, fiat_decimals: 2 });
		expect(sendDisplayContext(USD)).toEqual({ code: 'USD', rate: 1, fiat_decimals: 2 });
		expect(sendDisplayContext(UNPRICED)).toEqual({ code: 'VND', rate: null, fiat_decimals: 2 });
	});

	it('the code a typed figure is counted in is the person’s, never the placeholder’s', () => {
		expect(chosenCurrencyCode(ON_ITS_WAY)).toBe('CNY');
		expect(chosenCurrencyCode(CNY)).toBe('CNY');
		// Nothing stored: USD is what the core will commit.
		expect(chosenCurrencyCode(FIRST)).toBe('USD');
	});
});

/**
 * The REAL send machine, opened while the currency is on its way: fiat entry
 * is shut (nothing to divide by), token entry works, and the pair landing —
 * said as `display_changed`, which the web never sent — opens the toggle in
 * the person's money.
 */
describe('the send machine, opened before the currency commits', () => {
	const ACCOUNT = '0x' + 'aa'.repeat(20);
	const ETH: SendToken = {
		network: 'eth-mainnet',
		chain_id: 1,
		symbol: 'ETH',
		balance: '1.5',
		decimals: 18,
		token_address: null,
		price_usd: 3000,
		logo_urls: [],
		spam: false
	};

	type Out = { view: SendView; effects: { id: number; operation: { type: string } }[] };

	function opened(currency: CurrencyView) {
		const core = new SendCore();
		let view: SendView | null = null;
		const run = (out: Out) => {
			view = out.view;
			const queue = [...out.effects];
			for (let guard = 0; queue.length > 0 && guard < 50; guard += 1) {
				const effect = queue.shift()!;
				// Only the boot's token read is answered: the form, one coin held.
				if (effect.operation.type !== 'fetch_tokens') continue;
				const next = JSON.parse(
					core.resolve_effect(
						BigInt(effect.id),
						JSON.stringify({ type: 'tokens_loaded', tokens: [ETH], chains: [] })
					)
				) as Out;
				view = next.view;
				queue.push(...next.effects);
			}
		};
		const dispatch = (event: SendEvent) =>
			run(JSON.parse(core.dispatch(JSON.stringify(event))) as Out);
		dispatch({
			type: 'open',
			account: { id: 'cred', address: ACCOUNT, name: null },
			params: {
				preselected_symbol: null,
				preselected_network: null,
				prefilled_recipient: null,
				prefilled_chain_id: null,
				prefilled_token_address: null,
				prefilled_amount_base: null,
				locked: false,
				preselected_multi: null
			},
			display: sendDisplayContext(currency)
		});
		return { core, dispatch, view: () => view as unknown as SendView };
	}

	it('the ⇄ toggle will not enter fiat while there is no rate, and enters it once the pair lands', () => {
		const send = opened(ON_ITS_WAY);
		const token = send.view().tokens[0];
		expect(token).toBeDefined();
		send.dispatch({ type: 'select_token', token_id: sendTokenId(token) });
		send.dispatch({ type: 'set_amount', amount: '0.1' });
		expect(send.view().amount_fiat_code).toBeNull();

		// Pressed with the currency still on its way: nothing happens — no "$",
		// no "¥", no fiat figure of any kind.
		send.dispatch({ type: 'toggle_fiat_input' });
		expect(send.view().amount_fiat_code).toBeNull();
		expect(send.view().amount).toBe('0.1');

		// The pair lands, and the page says so.
		send.dispatch({ type: 'display_changed', display: sendDisplayContext(CNY) });
		// The token figure typed meanwhile stands: it was never in a currency.
		expect(send.view().amount).toBe('0.1');
		send.dispatch({ type: 'toggle_fiat_input' });
		expect(send.view().amount_fiat_code).toBe('CNY');
		send.core.free();
	});

	it('handed the placeholder as a pair (what the page used to do), it types dollars nobody chose', () => {
		// The defect, kept as a witness: USD at rate 1 is a valid pair, so the
		// machine cannot know it is not the person's.
		const send = opened(FIRST);
		const token = send.view().tokens[0];
		send.dispatch({ type: 'select_token', token_id: sendTokenId(token) });
		send.dispatch({
			type: 'display_changed',
			display: { code: 'USD', rate: 1, fiat_decimals: 2 }
		});
		send.dispatch({ type: 'toggle_fiat_input' });
		expect(send.view().amount_fiat_code).toBe('USD');
		send.core.free();
	});
});
