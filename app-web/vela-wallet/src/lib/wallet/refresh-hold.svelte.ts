/**
 * The hero refresh control's minimum spin (issue 462).
 *
 * The core holds `BalanceView.refreshing` for exactly as long as the read the
 * person asked for is out, and a read answered from warm connections can be
 * out for less than a frame — a press that "did nothing". The core's doc
 * gives this hold to the shell, and every shell holds the same 650 ms: from
 * the press, the glyph turns at least that long, and for as long as the
 * core's round is out after it.
 */

/** How long a press turns the glyph, however fast the read answers. */
export const REFRESH_MIN_SPIN_MS = 650;

/**
 * How often "Updated <ago>" is worded again while the home is on screen —
 * the core's compact time moves at 45 s and then by the minute, so a quarter
 * minute is never a whole step late.
 */
export const REFRESH_AGE_TICK_MS = 15_000;

export class RefreshHold {
	/** A press's hold is still running. */
	held = $state(false);

	#timer: ReturnType<typeof setTimeout> | null = null;

	/** The control was pressed: hold the spin for {@link REFRESH_MIN_SPIN_MS}. */
	press(): void {
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.held = true;
		this.#timer = setTimeout(() => {
			this.#timer = null;
			this.held = false;
		}, REFRESH_MIN_SPIN_MS);
	}

	/** The page went away: no timer outlives it. */
	dispose(): void {
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.#timer = null;
		this.held = false;
	}
}
