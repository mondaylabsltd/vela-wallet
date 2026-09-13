/**
 * The preferences that have no machine (spec 028 T431 — research D48).
 *
 * Theme, language, number / date / time format and avatar style are shell
 * state. There is no Rust core for them and none is invented here: a machine
 * holding four enums and no rule would be a machine in name only. Compare
 * `display_currency`, which DOES have one — because choosing a currency has a
 * rule behind it (a rate must exist, and an unpriceable currency must refuse).
 *
 * ## Why localStorage and not the IndexedDB KV
 *
 * These are read SYNCHRONOUSLY while a screen renders — every money figure and
 * every timestamp asks the number/date preset — and the theme has to be applied
 * before the first paint or the page flashes the wrong palette. That is the
 * same reason `vela.serviceEndpoints` stayed in localStorage in 024 (research
 * D3a). The keys are Expo's, byte-for-byte, so a person's phone and browser
 * would read the same record if they ever met: `vela.localePrefs` is one JSON
 * object, `vela.avatarStyle` and `vela.language` are bare strings.
 *
 * `vela.theme` is the one key Expo does not have — a phone follows the OS and
 * offers no choice, and a browser tab is a window inside someone else's
 * chrome, where "follow the OS" is a preference rather than the only option.
 */
import { browser } from '$app/environment';

/** What a person picks in 外观. `system` pins nothing and follows the OS. */
export type ThemeChoice = 'system' | 'light' | 'dark';

/** Identicon derived from the address, or the first letter of the name. */
export type AvatarStyle = 'initials' | 'identicon';

/** Grouping + decimal marks. `auto` reads the platform's conventions once. */
export type NumberFormatKey = 'auto' | 'comma_dot' | 'dot_comma' | 'space_comma' | 'indian';
/** Field order + separator. */
export type DateFormatKey = 'auto' | 'ymd_slash' | 'mdy_slash' | 'dmy_slash' | 'dmy_dot' | 'iso';
/** 12- vs 24-hour clock. */
export type TimeFormatKey = 'auto' | 'h24' | 'h12';

/** The six stops of the A ——●—— A slider, named as Expo names them. */
export type TextScaleLevel = 'compact' | 'small' | 'standard' | 'comfortable' | 'large' | 'xlarge';

/**
 * The slider's stops, in order — `src/constants/text-scale.ts` verbatim, so a
 * phone and a browser mean the same thing by "large". The factor multiplies
 * every `--text-*` token through `--text-scale` (design-tokens `textScale`
 * pins the range to 0.82–1.35).
 *
 * The same table is inlined in `app.html`, where it has to run before any
 * module loads; `preferences-store.test.ts` asserts the two agree.
 */
export const TEXT_SCALE_LEVELS: readonly { key: TextScaleLevel; factor: number }[] = [
	{ key: 'compact', factor: 0.82 },
	{ key: 'small', factor: 0.91 },
	{ key: 'standard', factor: 1 },
	{ key: 'comfortable', factor: 1.1 },
	{ key: 'large', factor: 1.22 },
	{ key: 'xlarge', factor: 1.35 }
];

const DEFAULT_TEXT_SCALE: TextScaleLevel = 'standard';

/** The Expo compatibility contract — these strings are the record format. */
export const PREF_KEYS = {
	theme: 'vela.theme',
	language: 'vela.language',
	localePrefs: 'vela.localePrefs',
	avatarStyle: 'vela.avatarStyle',
	textScale: 'vela.textScale'
} as const;

const THEMES: readonly ThemeChoice[] = ['system', 'light', 'dark'];
const AVATARS: readonly AvatarStyle[] = ['initials', 'identicon'];
const NUMBERS: readonly NumberFormatKey[] = [
	'auto',
	'comma_dot',
	'dot_comma',
	'space_comma',
	'indian'
];
const DATES: readonly DateFormatKey[] = [
	'auto',
	'ymd_slash',
	'mdy_slash',
	'dmy_slash',
	'dmy_dot',
	'iso'
];
const TIMES: readonly TimeFormatKey[] = ['auto', 'h24', 'h12'];

function read(key: string): string | null {
	if (!browser) return null;
	try {
		return localStorage.getItem(key);
	} catch {
		// Blocked storage. A preference is a convenience; the app runs without it.
		return null;
	}
}

function write(key: string, value: string): void {
	if (!browser) return;
	try {
		localStorage.setItem(key, value);
	} catch {
		/* as above — the in-memory choice still holds for this session */
	}
}

/** A stored value only wins if it is one of the values we ship. */
function oneOf<T extends string>(raw: string | null, allowed: readonly T[], fallback: T): T {
	return allowed.includes(raw as T) ? (raw as T) : fallback;
}

class Preferences {
	theme = $state<ThemeChoice>('system');
	avatarStyle = $state<AvatarStyle>('identicon');
	numberFormat = $state<NumberFormatKey>('auto');
	dateFormat = $state<DateFormatKey>('auto');
	timeFormat = $state<TimeFormatKey>('auto');
	/** `auto` follows the browser; otherwise a shipped locale code. */
	language = $state<string>('auto');
	textScale = $state<TextScaleLevel>(DEFAULT_TEXT_SCALE);

	#booted = false;

	/** The slider's stop for the current level — what the A ——●—— A shows. */
	get textScaleIndex(): number {
		return TEXT_SCALE_LEVELS.findIndex((level) => level.key === this.textScale);
	}

	/**
	 * Read what is stored. Idempotent, synchronous, and safe to call from every
	 * route's `onMount` — the second call is a no-op rather than a second read
	 * that could land after a person has already chosen something.
	 */
	boot(): void {
		if (this.#booted || !browser) return;
		this.#booted = true;
		this.theme = oneOf(read(PREF_KEYS.theme), THEMES, 'system');
		this.avatarStyle = oneOf(read(PREF_KEYS.avatarStyle), AVATARS, 'identicon');
		this.language = read(PREF_KEYS.language) ?? 'auto';
		this.textScale = oneOf(
			read(PREF_KEYS.textScale),
			TEXT_SCALE_LEVELS.map((level) => level.key),
			DEFAULT_TEXT_SCALE
		);
		const raw = read(PREF_KEYS.localePrefs);
		if (raw !== null) {
			try {
				const stored = JSON.parse(raw) as Record<string, unknown>;
				this.numberFormat = oneOf(stored.numberFormat as string, NUMBERS, 'auto');
				this.dateFormat = oneOf(stored.dateFormat as string, DATES, 'auto');
				this.timeFormat = oneOf(stored.timeFormat as string, TIMES, 'auto');
			} catch {
				/* A torn record reads as the defaults, which is what it means. */
			}
		}
		this.applyTheme();
		this.applyTextScale();
	}

	/**
	 * Put the chosen size on the document as `--text-scale`, the multiplier
	 * every `--text-*` use already carries (`calc(var(--text-lg) * var(--text-scale, 1))`).
	 * `standard` REMOVES the property rather than writing `1`, so the fallback
	 * stays the one source of the default — and a stylesheet that never met this
	 * store reads exactly as it did before.
	 */
	applyTextScale(): void {
		if (!browser) return;
		const factor = TEXT_SCALE_LEVELS.find((level) => level.key === this.textScale)?.factor ?? 1;
		const root = document.documentElement.style;
		if (factor === 1) root.removeProperty('--text-scale');
		else root.setProperty('--text-scale', String(factor));
	}

	/**
	 * Put the chosen palette on the document, which is where `isDarkTheme()`
	 * already looks (spec 012 FR-009). `system` REMOVES the attribute rather
	 * than writing a resolved value: a pinned "dark" would stop following an OS
	 * that changes at sunset, which is the whole meaning of the choice.
	 */
	applyTheme(): void {
		if (!browser) return;
		if (this.theme === 'system') delete document.documentElement.dataset.theme;
		else document.documentElement.dataset.theme = this.theme;
	}

	setTheme(value: ThemeChoice): void {
		this.theme = value;
		write(PREF_KEYS.theme, value);
		this.applyTheme();
	}

	setAvatarStyle(value: AvatarStyle): void {
		this.avatarStyle = value;
		write(PREF_KEYS.avatarStyle, value);
	}

	setLanguage(value: string): void {
		this.language = value;
		write(PREF_KEYS.language, value);
	}

	setTextScale(value: TextScaleLevel): void {
		this.textScale = value;
		write(PREF_KEYS.textScale, value);
		this.applyTextScale();
	}

	/** The slider speaks in stops; a stop off the end is ignored, not clamped. */
	setTextScaleIndex(index: number): void {
		const level = TEXT_SCALE_LEVELS[index];
		if (level !== undefined) this.setTextScale(level.key);
	}

	setNumberFormat(value: NumberFormatKey): void {
		this.numberFormat = value;
		this.#saveLocalePrefs();
	}

	setDateFormat(value: DateFormatKey): void {
		this.dateFormat = value;
		this.#saveLocalePrefs();
	}

	setTimeFormat(value: TimeFormatKey): void {
		this.timeFormat = value;
		this.#saveLocalePrefs();
	}

	/** One record, three fields — the shape Expo writes and reads. */
	#saveLocalePrefs(): void {
		write(
			PREF_KEYS.localePrefs,
			JSON.stringify({
				numberFormat: this.numberFormat,
				dateFormat: this.dateFormat,
				timeFormat: this.timeFormat
			})
		);
	}

	/** Tests only: forget what was read so the next `boot()` reads again. */
	resetForTests(): void {
		this.#booted = false;
		this.theme = 'system';
		this.avatarStyle = 'identicon';
		this.numberFormat = 'auto';
		this.dateFormat = 'auto';
		this.timeFormat = 'auto';
		this.language = 'auto';
		this.textScale = DEFAULT_TEXT_SCALE;
	}
}

export const preferences = new Preferences();
