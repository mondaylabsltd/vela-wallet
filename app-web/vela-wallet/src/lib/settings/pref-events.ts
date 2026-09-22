/**
 * What a person did to a preference row (spec 028 T433).
 *
 * The same shape `net-events.ts` uses, and for the same reason: the screen
 * reports the tap and the ROUTE owns the translation table, so there is one
 * place to read to know what every control does — and a component that has to
 * be handed six callbacks does not grow a seventh quietly.
 *
 * Unlike the network events, none of these reaches a core. Theme, language
 * and the three formats are shell state with no rule behind them
 * (research D48); `erase` is the exception that has a rule, and it is a
 * SHELL rule — a namespace sweep over three key-value stores, which no core has
 * a port to perform.
 */
export type SettingsPrefEvent =
	/** A segment id from the drawn control: `light` / `dark` / `auto`. */
	| { kind: 'theme'; id: string }
	/** `system`, or a shipped locale code. */
	| { kind: 'language'; id: string }
	/** The slider's stop, 0-based — `TEXT_SCALE_LEVELS` names what it means. */
	| { kind: 'text-scale'; index: number }
	/** A display-currency code — one of two rows here that reach a core (`display_currency`). */
	| { kind: 'currency'; id: string }
	/**
	 * A default transaction speed — `fast` / `standard` / `slow` (spec 068).
	 * The second row with a core behind it (`fee_tier_pref`), and the reason
	 * the comment above no longer says "the one". A per-transaction pick on
	 * the send screen is NOT this event: that choice is one-shot and never
	 * rewrites the stored default.
	 */
	| { kind: 'fee-speed'; id: string }
	/**
	 * The default "Sign with" (spec 071) — one of `SignPrefView.offered`. Like
	 * the speed, a pick on a signing sheet is NOT this event: it signs that one
	 * request and never rewrites the default.
	 */
	| { kind: 'sign-with'; id: string }
	/** The Clear Signer's page, as typed and saved — the `sign_pref` core validates it. */
	| { kind: 'signer-page'; text: string }
	/** Back to the official page. */
	| { kind: 'signer-page-reset' }
	| { kind: 'number-format'; id: string }
	| { kind: 'date-format'; id: string }
	| { kind: 'time-format'; id: string }
	/** The destructive one, confirmed. */
	| { kind: 'erase' };
