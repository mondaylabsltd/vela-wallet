/**
 * A board with the display currency ON ITS WAY — gallery only.
 *
 * The core's withhold rule (`display_currency.rs`): until the display
 * currency commits, no fiat figure is drawn on any surface, and the room each
 * figure will take is kept. A live wallet is in that state for a second or
 * two after a cold start, which is not long enough to review a screen in. So
 * every board has a twin: the same drawn state, with each fiat figure
 * withheld exactly as the live builders withhold it —
 *
 * - a figure in text is the pending mark (`MONEY_PENDING`), where the figure
 *   stood: "≈ $120.00" → "≈ …", "0.0021 ETH · ≈$0.55" → "0.0021 ETH · ≈…";
 * - the hero is its skeleton, with the currency still named (it is the
 *   currency on its way);
 * - a fee row is told its money is withheld, so it holds the figure's room.
 *
 * Token amounts are not fiat and are left alone, as they are live.
 *
 * `/<locale>/gallery/<state>?fiat=withheld` draws the twin (and the gallery's
 * controls switch between the two), so the frame before the currency commits
 * and the frame after it can be laid side by side. This is a transform over
 * FIXTURE models for the gallery; product code never calls it — the live
 * surfaces go through `moneyParts`.
 */
import { MONEY_PENDING } from './live';

/** A fiat figure as the fixtures write one: a currency's glyph, then its digits. */
const FIGURE = /[$¥€£₫₩]\s?\d[\d,]*(?:\.\d+)?/g;

/** The parts of a gallery page's data that are drawn models. */
const MODEL_KEYS = ['model', 'signing', 'wallet', 'settings'] as const;

type Plain = Record<string, unknown>;

const isPlain = (value: unknown): value is Plain =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

/** The hero's model: a figure in two sizes under a label. */
const isHero = (value: Plain): boolean =>
	typeof value.label === 'string' &&
	typeof value.a11yHide === 'string' &&
	(value.state === 'normal' || value.state === 'zero-live');

/** What the hero draws only once it has a figure. */
const HERO_FIGURE_KEYS = new Set(['integer', 'decimals', 'decimalMark', 'liveText']);

/** A fee row: a coin figure and, beside it, what it costs. */
const isFeeRow = (value: Plain): boolean =>
	typeof value.valueFiat === 'string' && typeof value.openLabel === 'string';

function withheld(value: unknown): unknown {
	if (typeof value === 'string') {
		// Artwork is markup, not copy: nothing in it is a figure.
		return value.startsWith('<') ? value : value.replace(FIGURE, MONEY_PENDING);
	}
	if (Array.isArray(value)) return value.map(withheld);
	if (!isPlain(value)) return value;
	if (isHero(value)) {
		// The skeleton, whatever the balance knows; no figure, no "listening".
		const rest = Object.fromEntries(
			Object.entries(value).filter(([key]) => !HERO_FIGURE_KEYS.has(key))
		);
		return { ...withheldEntries(rest), state: 'loading' };
	}
	const out = withheldEntries(value);
	return isFeeRow(value) ? { ...out, valueFiatWithheld: true } : out;
}

function withheldEntries(value: Plain): Plain {
	return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, withheld(entry)]));
}

/** One drawn model, with every fiat figure withheld. */
export function withheldModel<T>(model: T): T {
	return withheld(model) as T;
}

/**
 * A gallery page's data with its drawn models withheld — and nothing else
 * touched: the copy bundles beside them are sentences, not figures.
 */
export function withheldBoard<T extends object>(data: T): T {
	const out: Plain = { ...(data as Plain) };
	for (const key of MODEL_KEYS) {
		if (key in out) out[key] = withheld(out[key]);
	}
	return out as T;
}
