/**
 * The usage-analytics catalog: every event the web wallet and the extension
 * may send, and every property an event may carry. One file, so what leaves a
 * person's browser can be reviewed in one read.
 *
 * Properties are COARSE by construction: each one is an enum or a chain id,
 * checked at runtime against the rules below (`PROPERTY_RULES`), so a careless
 * call site cannot send an address, an amount, a hash, a name or a URL — a
 * value that is not one of the allowed shapes is dropped before the payload is
 * built, and the payload is then refused whole if anything in it still looks
 * like an identifier (`payload.ts`).
 */
import { SUPPORTED_LOCALES, type Locale } from '$lib/i18n/locales';

/**
 * How a wallet was created or signed into — the onboarding core's own
 * `KeyMethod` names: this device's passkey, a phone's (cross-device), or a
 * security key.
 */
export const METHODS = ['platform', 'hybrid', 'security_key'] as const;
export type AnalyticsMethod = (typeof METHODS)[number];

/** Why something did not finish — a kind, never a message. */
export const REASONS = [
	'cancelled',
	'timeout',
	'network',
	'rejected',
	'unsupported',
	'not_found',
	/** The relay could not take or fund it. */
	'relay',
	/** Submitted, then never mined. */
	'dropped',
	/** A may-have-been-sent op the relay never had. */
	'not_sent',
	'error'
] as const;
export type AnalyticsReason = (typeof REASONS)[number];

/** What a site asked the wallet for. */
export const DAPP_KINDS = [
	'connect',
	'sign_message',
	'sign_typed_data',
	'send_transaction',
	'send_calls',
	'add_network',
	'other'
] as const;
export type AnalyticsDappKind = (typeof DAPP_KINDS)[number];

export const THEMES = ['system', 'light', 'dark'] as const;
export const SPEEDS = ['slow', 'standard', 'fast'] as const;
/**
 * Where the app-download prompt was: the web wallet's home, the screen that
 * says a new wallet is ready, or the extension's wallet home.
 */
export const PLACEMENTS = ['home', 'create_done', 'extension'] as const;
export const STORES = ['app_store', 'google_play'] as const;
export const PLATFORMS = ['ios', 'android', 'desktop'] as const;
/** Which build sent the event — the hosted wallet or the browser extension. */
export const SURFACES = ['web', 'extension'] as const;

export type AnalyticsPlacement = (typeof PLACEMENTS)[number];
export type AnalyticsStore = (typeof STORES)[number];
export type AnalyticsPlatform = (typeof PLATFORMS)[number];
export type AnalyticsSurface = (typeof SURFACES)[number];

/** The value type of every property an event may carry. */
export interface AnalyticsPropertyTypes {
	method: AnalyticsMethod;
	reason: AnalyticsReason;
	/** A chain id (EIP-155) — a network, never an account. */
	chain: number;
	kind: AnalyticsDappKind;
	locale: Locale;
	/** An ISO 4217 code, as the currency picker lists them. */
	currency: string;
	theme: (typeof THEMES)[number];
	speed: (typeof SPEEDS)[number];
	placement: AnalyticsPlacement;
	store: AnalyticsStore;
	platform: AnalyticsPlatform;
	surface: AnalyticsSurface;
}
export type AnalyticsProperty = keyof AnalyticsPropertyTypes;

const among =
	(values: readonly string[]) =>
	(value: unknown): boolean =>
		typeof value === 'string' && values.includes(value);

/**
 * The runtime rule for each property. A value that fails its rule is DROPPED —
 * the event still goes, without it.
 */
export const PROPERTY_RULES: Readonly<Record<AnalyticsProperty, (value: unknown) => boolean>> = {
	method: among(METHODS),
	reason: among(REASONS),
	chain: (value) =>
		typeof value === 'number' &&
		Number.isSafeInteger(value) &&
		value > 0 &&
		// EIP-2294's ceiling: a chain id above it is not one.
		value <= 4_503_599_627_370_476,
	kind: among(DAPP_KINDS),
	locale: among(SUPPORTED_LOCALES),
	currency: (value) => typeof value === 'string' && /^[A-Z]{3}$/.test(value),
	theme: among(THEMES),
	speed: among(SPEEDS),
	placement: among(PLACEMENTS),
	store: among(STORES),
	platform: among(PLATFORMS),
	surface: among(SURFACES)
};

/**
 * Every event, and the properties it may carry. `surface` and `locale` are
 * added to every event by the sender; the lists name only what a call site
 * supplies.
 */
export const EVENTS = {
	// Onboarding — a new wallet.
	wallet_create_started: ['method'],
	wallet_create_completed: ['method'],
	wallet_create_failed: ['method', 'reason'],
	// Onboarding — an existing wallet on this device.
	sign_in_started: ['method'],
	sign_in_completed: ['method'],
	sign_in_failed: ['method', 'reason'],
	// Send.
	send_opened: [],
	send_submitted: ['chain'],
	send_confirmed: ['chain'],
	send_failed: ['chain', 'reason'],
	// Receive.
	receive_opened: [],
	receive_address_copied: [],
	receive_qr_shown: [],
	// A site's requests (the extension's dApp channel).
	dapp_connect_approved: ['chain'],
	dapp_connect_rejected: ['chain'],
	dapp_request_approved: ['kind', 'chain'],
	dapp_request_rejected: ['kind', 'chain'],
	// Settings.
	setting_language_changed: ['locale'],
	setting_currency_changed: ['currency'],
	setting_theme_changed: ['theme'],
	setting_fee_speed_changed: ['speed'],
	network_added: ['chain'],
	network_removed: ['chain'],
	network_rpc_set: ['chain'],
	backup_keys_started: [],
	backup_keys_done: [],
	// Contacts.
	contact_added: [],
	contacts_imported: [],
	contacts_exported: [],
	// The app-download prompt ("Get Vela on your phone").
	store_prompt_shown: ['placement', 'platform'],
	store_click: ['store', 'placement'],
	store_prompt_dismissed: ['placement']
} as const satisfies Record<string, readonly AnalyticsProperty[]>;

export type AnalyticsEventName = keyof typeof EVENTS;

/** The properties a call site may pass with `name` — each optional, each coarse. */
export type AnalyticsEventProps<E extends AnalyticsEventName> = {
	[K in (typeof EVENTS)[E][number]]?: AnalyticsPropertyTypes[K];
};

/** Properties the sender adds to every event. */
export const AMBIENT_PROPERTIES = [
	'surface',
	'locale'
] as const satisfies readonly AnalyticsProperty[];

export function isAnalyticsEvent(name: unknown): name is AnalyticsEventName {
	return typeof name === 'string' && Object.hasOwn(EVENTS, name);
}
