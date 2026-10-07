/**
 * The "Get Vela on your phone" prompt's words — names and shape only; they are
 * resolved at build time in `engine.server.ts` (`resolveAppPromptMessages`),
 * like every other screen's.
 */
export interface AppPromptMessages {
	title: string;
	body: string;
	/** Under a code, on a desktop: what to do with it. */
	scanHint: string;
	/** The close button's accessible name. */
	close: string;
	/** Product names, not prose — never translated. */
	appStore: string;
	googlePlay: string;
}

export const APP_PROMPT_KEYS = [
	'home.getApp.title',
	'home.getApp.body',
	'home.getApp.scanHint',
	'onboarding.common.close'
] as const;
