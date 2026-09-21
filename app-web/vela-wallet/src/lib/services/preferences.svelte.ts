/**
 * The display preferences (spec 028 T431 — research D48; spec 072).
 *
 * Theme, language, number / date / time format, text size and avatar style.
 * Choosing one has no rule behind it — compare `display_currency`, whose
 * machine refuses a currency no rate can price — so there is no machine here.
 * What IS a rule is the record format: four shells wrote these five
 * preferences four ways (Android's `system` language and its text size nested
 * in `vela.localePrefs`, the desktop's `vela.formats`), and a record written
 * by one then meant something else to the others. How a stored record reads,
 * and which older spellings are rewritten, is the core's (`vela_core::prefs`,
 * `prefsRead` / `prefsMigrations`) — this store hands it the entries and
 * writes back what it says.
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
 * ## Why two reads
 *
 * The core is 4 MB of wasm that arrives after the first render, and a boot
 * that waited for it would render every figure in the wrong format first. So
 * `boot()` reads the shared spelling synchronously, and once the core is up
 * the store is migrated and read again through it — which is when an older
 * shell's spelling starts to count.
 */
import { browser } from '$app/environment';
import { loadCore, prefsMigrations, prefsRead } from '$lib/core/client';

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

function remove(key: string): void {
	if (!browser) return;
	try {
		localStorage.removeItem(key);
	} catch {
		/* as above */
	}
}

/** A stored value only wins if it is one of the values we ship. */
function oneOf<T extends string>(raw: string | null, allowed: readonly T[], fallback: T): T {
	return allowed.includes(raw as T) ? (raw as T) : fallback;
}

/** The five preferences, read — `prefsRead`'s record, less the factor the table already holds. */
interface PrefsRecord {
	theme: ThemeChoice;
	language: string;
	avatarStyle: AvatarStyle;
	textScale: TextScaleLevel;
	numberFormat: NumberFormatKey;
	dateFormat: DateFormatKey;
	timeFormat: TimeFormatKey;
}

/**
 * The first read, before the core is up: the shared spelling only, anything
 * else the default. The core's read replaces it moments later.
 */
function readSharedSpelling(): PrefsRecord {
	const record: PrefsRecord = {
		theme: oneOf(read(PREF_KEYS.theme), THEMES, 'system'),
		language: read(PREF_KEYS.language) ?? 'auto',
		avatarStyle: oneOf(read(PREF_KEYS.avatarStyle), AVATARS, 'identicon'),
		textScale: oneOf(
			read(PREF_KEYS.textScale),
			TEXT_SCALE_LEVELS.map((level) => level.key),
			DEFAULT_TEXT_SCALE
		),
		numberFormat: 'auto',
		dateFormat: 'auto',
		timeFormat: 'auto'
	};
	const raw = read(PREF_KEYS.localePrefs);
	if (raw !== null) {
		try {
			const stored = JSON.parse(raw) as Record<string, unknown>;
			record.numberFormat = oneOf(stored.numberFormat as string, NUMBERS, 'auto');
			record.dateFormat = oneOf(stored.dateFormat as string, DATES, 'auto');
			record.timeFormat = oneOf(stored.timeFormat as string, TIMES, 'auto');
		} catch {
			/* A torn record reads as the defaults, which is what it means. */
		}
	}
	return record;
}

/**
 * The whole store as the core takes it, `{key: rawValue}`. The core picks out
 * the keys it knows — including the older spellings this file never names.
 */
function storedEntries(): string {
	const entries: Record<string, string> = {};
	try {
		for (let i = 0; i < localStorage.length; i += 1) {
			const key = localStorage.key(i);
			const value = key === null ? null : localStorage.getItem(key);
			if (key !== null && value !== null) entries[key] = value;
		}
	} catch {
		/* Blocked storage holds nothing, which reads as the defaults. */
	}
	return JSON.stringify(entries);
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
	/** Set by every setter: a choice made this visit is newer than any stored read. */
	#chosen = false;
	#ready: Promise<void> = Promise.resolve();

	/** The slider's stop for the current level — what the A ——●—— A shows. */
	get textScaleIndex(): number {
		return TEXT_SCALE_LEVELS.findIndex((level) => level.key === this.textScale);
	}

	/** Settles once the core has migrated the store and read it (or could not load). */
	get ready(): Promise<void> {
		return this.#ready;
	}

	/**
	 * Read what is stored. Idempotent, synchronous, and safe to call from every
	 * route's `onMount` — the second call is a no-op rather than a second read
	 * that could land after a person has already chosen something.
	 *
	 * Every route that calls this loads the core anyway; the migration rides
	 * on that same load rather than starting one of its own.
	 */
	boot(): void {
		if (this.#booted || !browser) return;
		this.#booted = true;
		this.#adopt(readSharedSpelling());
		this.#ready = loadCore().then(
			() => this.#readThroughCore(),
			() => {
				/* No core, no older spellings: the first read stands. */
			}
		);
	}

	/**
	 * Rewrite what an older shell wrote into the shared spelling — once: the
	 * migrations are empty for a store that already agrees — then read the
	 * store again the way every shell reads it. A person who chose something
	 * while the core was loading is not second-guessed by a read that began
	 * before they did; the rewrite still lands for the next visit.
	 */
	#readThroughCore(): void {
		const writes = JSON.parse(prefsMigrations(storedEntries())) as {
			key: string;
			value: string | null;
		}[];
		for (const { key, value } of writes) {
			if (value === null) remove(key);
			else write(key, value);
		}
		if (this.#chosen) return;
		this.#adopt(JSON.parse(prefsRead(storedEntries())) as PrefsRecord);
	}

	#adopt(record: PrefsRecord): void {
		this.theme = record.theme;
		this.avatarStyle = record.avatarStyle;
		this.language = record.language;
		this.textScale = record.textScale;
		this.numberFormat = record.numberFormat;
		this.dateFormat = record.dateFormat;
		this.timeFormat = record.timeFormat;
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
		this.#chosen = true;
		write(PREF_KEYS.theme, value);
		this.applyTheme();
	}

	setAvatarStyle(value: AvatarStyle): void {
		this.avatarStyle = value;
		this.#chosen = true;
		write(PREF_KEYS.avatarStyle, value);
	}

	setLanguage(value: string): void {
		this.language = value;
		this.#chosen = true;
		write(PREF_KEYS.language, value);
	}

	setTextScale(value: TextScaleLevel): void {
		this.textScale = value;
		this.#chosen = true;
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
		this.#chosen = true;
		this.#saveLocalePrefs();
	}

	setDateFormat(value: DateFormatKey): void {
		this.dateFormat = value;
		this.#chosen = true;
		this.#saveLocalePrefs();
	}

	setTimeFormat(value: TimeFormatKey): void {
		this.timeFormat = value;
		this.#chosen = true;
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
		this.#chosen = false;
		this.#ready = Promise.resolve();
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
