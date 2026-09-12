/**
 * What a person did to a preference row (spec 028 T433).
 *
 * The same shape `net-events.ts` uses, and for the same reason: the screen
 * reports the tap and the ROUTE owns the translation table, so there is one
 * place to read to know what every control does — and a component that has to
 * be handed six callbacks does not grow a seventh quietly.
 *
 * Unlike the network events, none of these reaches a core. Theme, language,
 * the three formats and the avatar style are shell state with no rule behind
 * them (research D48); `erase` is the exception that has a rule, and it is a
 * SHELL rule — a namespace sweep over three key-value stores, which no core has
 * a port to perform.
 */
export type SettingsPrefEvent =
	/** A segment id from the drawn control: `light` / `dark` / `auto`. */
	| { kind: 'theme'; id: string }
	/** `initials` or `identicon`. */
	| { kind: 'avatar'; id: string }
	/** `system`, or a shipped locale code. */
	| { kind: 'language'; id: string }
	/** The slider's stop, 0-based — `TEXT_SCALE_LEVELS` names what it means. */
	| { kind: 'text-scale'; index: number }
	/** A display-currency code — the one row here that DOES reach a core (`display_currency`). */
	| { kind: 'currency'; id: string }
	| { kind: 'number-format'; id: string }
	| { kind: 'date-format'; id: string }
	| { kind: 'time-format'; id: string }
	/** The destructive one, confirmed. */
	| { kind: 'erase' };
