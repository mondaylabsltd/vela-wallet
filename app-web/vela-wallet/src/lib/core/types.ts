/**
 * Shared types for wiring wallet-state cores (spec 024).
 *
 * The generic half of what every machine's factory takes: a view sink and a
 * fault reporter. Machines with extra construction inputs extend this the way
 * the Expo client's `wallet-state-core/types.ts` does (its `SessionOptions`
 * is this shape, ported); onboarding keeps its own richer options type in
 * `#lib/onboarding/core/sessions.ts` because its deps travel with the screen.
 */
export type SessionOptions<View> = {
	/** Called with every view the core produces, including the first. */
	onView: (view: View) => void;
	/** A core-level fault (malformed event, bad JSON) — never a user-facing error. */
	onError?: (error: unknown) => void;
};
