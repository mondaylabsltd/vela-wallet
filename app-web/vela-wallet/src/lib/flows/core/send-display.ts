/**
 * The display currency, as the send machine is told it (`SendDisplayContext`).
 *
 * The machine's context "arrives straight from `display_currency`'s committed
 * pair" — and before there IS a committed pair, the view on the wire is the
 * USD/1 placeholder, which is not the person's currency (the core's withhold
 * rule: no fiat figure before the currency commits, on any surface). Handed to
 * the send machine as it was, a Send opened in a wallet's first seconds could
 * be flipped to typing dollars at rate 1 — a fiat figure in a currency nobody
 * chose — and, since nothing told the machine when the real pair landed, it
 * stayed on dollars for as long as that send was open.
 *
 * So, one rule in one place:
 *
 * - **committed**: the pair as it is (a `null` rate included — the person's
 *   choice that could not be priced, which the machine already refuses to
 *   convert at);
 * - **not yet**: the code on its way (`pending`) when there is one, with NO
 *   rate. `rate: null` is the state the machine keeps for exactly this — the
 *   fiat-denominated input is unavailable, the ⇄ toggle will not enter it —
 *   and token-denominated sending is untouched, so Send stays usable while
 *   the currency is on its way.
 *
 * The page sends the result at `open`, and again as `display_changed` whenever
 * it changes, so the pair landing opens the toggle in the right money.
 */
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { SendDisplayContext } from '$lib/core/generated/SendDisplayContext';

/** Every currency the wallet lists is entered to two places here, as before. */
const FIAT_DECIMALS = 2;

export function sendDisplayContext(currency: CurrencyView): SendDisplayContext {
	if (!currency.committed) {
		return { code: currency.pending ?? currency.code, rate: null, fiat_decimals: FIAT_DECIMALS };
	}
	return { code: currency.code, rate: currency.rate, fiat_decimals: FIAT_DECIMALS };
}

/**
 * The currency a figure typed by the person is counted in, where a surface
 * needs a code before the pair is committed (the batch importer prices its
 * own sheet): the committed code, else the stored choice on its way, else the
 * placeholder's — which is then what the core itself will commit.
 */
export function chosenCurrencyCode(currency: CurrencyView): string {
	return currency.committed ? currency.code : (currency.pending ?? currency.code);
}
