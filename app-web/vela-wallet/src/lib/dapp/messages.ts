/**
 * The request window's message manifest (spec 027 T322).
 *
 * Client-safe: names keys and shapes only — resolution happens in
 * `engine.server.ts` at build time, exactly like `wallet/messages.ts` and
 * `explore/messages.ts`.
 *
 * Every key here already exists in the corpus. The words a person reads when a
 * site asks for their address were written for the in-app browser's connect
 * sheet (spec 022), and they say the right thing wherever the request came
 * from — which is the point of keeping copy in one corpus rather than one per
 * surface.
 */
export interface RequestMessages {
	/** Template — 'Connect to {{host}}'. */
	title: string;
	body: string;
	connect: string;
	cancel: string;
	preparing: string;
	/**
	 * Spec 077: the landing a signed transaction shows, in the SEND receipt's
	 * own words. Borrowed rather than written again so the two surfaces cannot
	 * drift into saying different things about the same moment — which is what
	 * the owner asked for ("UI 要保持一致性").
	 */
	receipt: {
		confirming: string;
		confirmingHint: string;
		submitted: string;
		confirmed: string;
		failed: string;
		failedHint: string;
		opHashLabel: string;
		txHashLabel: string;
		explorer: string;
		done: string;
	};
}
