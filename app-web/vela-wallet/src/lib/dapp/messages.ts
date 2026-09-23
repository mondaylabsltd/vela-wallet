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
}
