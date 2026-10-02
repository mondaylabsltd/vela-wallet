/**
 * The packaged extension's own notices (spec 094): what every extension page
 * may have to say about the extension itself, above whatever it shows.
 * Client-safe — keys and shapes only; `engine.server.ts` resolves them at
 * build time into every `[locale]` page's layout data.
 */
export interface ExtensionMessages {
	/** A fresh install found web pages already open (S3): reload them. */
	installedNote: string;
	/** The person limited Vela's site access (S2): what that breaks. */
	accessNote: string;
	/** The one-click grant. */
	accessAllow: string;
	/** Dismisses the install note. */
	dismiss: string;
}
