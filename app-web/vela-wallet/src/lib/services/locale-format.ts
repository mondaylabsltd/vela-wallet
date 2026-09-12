// Ported from src/services/locale-format.ts @ 28d25ae9 — the React subscription
// hook dropped (runes do that here), the storage seam swapped for `preferences`.
/**
 * Number / date / time formatting from the product's own presets (spec 028
 * T430 — research D47).
 *
 * **Explicit rules, never `Intl` for the output.** The reason survives the port
 * even though the platform changed: a wallet that renders a figure differently
 * depending on the browser's idea of a locale is a wallet where the same
 * balance reads two ways on two machines, and where a person cannot tell a
 * thousands separator from a decimal point in a language they are guessing at.
 * The preset is chosen by the person and is the same everywhere they open it.
 *
 * `Intl` appears exactly once, in the `auto` DETECTION below — reading the
 * platform's conventions to pick a preset is a different act from letting it
 * format money, and the result is a preset like any other.
 *
 * This includes money, which is why it is not cosmetic.
 */
import {
	preferences,
	type DateFormatKey,
	type NumberFormatKey,
	type TimeFormatKey
} from './preferences.svelte';

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

interface NumberStyle {
	group: string;
	decimal: string;
	indian: boolean;
}

const NUMBER_STYLES: Record<Exclude<NumberFormatKey, 'auto'>, NumberStyle> = {
	comma_dot: { group: ',', decimal: '.', indian: false },
	dot_comma: { group: '.', decimal: ',', indian: false },
	space_comma: { group: ' ', decimal: ',', indian: false }, // space grouping (fr-FR style)
	indian: { group: ',', decimal: '.', indian: true }
};

// ---------------------------------------------------------------------------
// System detection (best-effort) for `auto`
// ---------------------------------------------------------------------------

let autoNumber: Exclude<NumberFormatKey, 'auto'> | null = null;
let autoDate: Exclude<DateFormatKey, 'auto'> | null = null;
let autoTime: Exclude<TimeFormatKey, 'auto'> | null = null;

function detectNumber(): Exclude<NumberFormatKey, 'auto'> {
	if (autoNumber) return autoNumber;
	let key: Exclude<NumberFormatKey, 'auto'> = 'comma_dot';
	try {
		const parts = new Intl.NumberFormat(undefined, { minimumFractionDigits: 1 }).formatToParts(
			1234567.8
		);
		const group = parts.find((p) => p.type === 'group')?.value ?? ',';
		const decimal = parts.find((p) => p.type === 'decimal')?.value ?? '.';
		if (decimal === ',' && group === '.') key = 'dot_comma';
		else if (decimal === ',')
			key = 'space_comma'; // space / NBSP / narrow NBSP
		else {
			// decimal '.', group ',' → standard, unless Indian 2-3 grouping.
			const grouped = parts.filter((p) => p.type === 'integer').map((p) => p.value);
			key =
				grouped.length > 1 &&
				grouped[grouped.length - 1].length === 3 &&
				grouped.some((g) => g.length === 2)
					? 'indian'
					: 'comma_dot';
		}
	} catch {
		/* keep default */
	}
	autoNumber = key;
	return key;
}

function detectDate(): Exclude<DateFormatKey, 'auto'> {
	if (autoDate) return autoDate;
	let key: Exclude<DateFormatKey, 'auto'> = 'mdy_slash';
	try {
		const parts = new Intl.DateTimeFormat(undefined, {
			year: 'numeric',
			month: '2-digit',
			day: '2-digit'
		}).formatToParts(new Date(2026, 5, 13));
		const order = parts
			.filter((p) => ['year', 'month', 'day'].includes(p.type))
			.map((p) => p.type[0])
			.join('');
		const sep = (parts.find((p) => p.type === 'literal')?.value ?? '/').trim() || '/';
		if (order === 'ymd') key = sep === '-' ? 'iso' : 'ymd_slash';
		else if (order === 'dmy') key = sep === '.' ? 'dmy_dot' : 'dmy_slash';
		else key = 'mdy_slash';
	} catch {
		/* keep default */
	}
	autoDate = key;
	return key;
}

function detectTime(): Exclude<TimeFormatKey, 'auto'> {
	if (autoTime) return autoTime;
	let key: Exclude<TimeFormatKey, 'auto'> = 'h24';
	try {
		const parts = new Intl.DateTimeFormat(undefined, {
			hour: 'numeric',
			minute: 'numeric'
		}).formatToParts(new Date(2026, 5, 13, 13, 45));
		key = parts.some((p) => p.type === 'dayPeriod') ? 'h12' : 'h24';
	} catch {
		/* keep default */
	}
	autoTime = key;
	return key;
}

function resolveNumber(key: NumberFormatKey): Exclude<NumberFormatKey, 'auto'> {
	return key === 'auto' ? detectNumber() : key;
}
function resolveDate(key: DateFormatKey): Exclude<DateFormatKey, 'auto'> {
	return key === 'auto' ? detectDate() : key;
}
function resolveTime(key: TimeFormatKey): Exclude<TimeFormatKey, 'auto'> {
	return key === 'auto' ? detectTime() : key;
}

/**
 * The chosen presets with `auto` already resolved to a concrete one.
 *
 * Detection reads `Intl`, which is a SHELL capability — a portable core that
 * formats figures must be handed the resolved conventions rather than the word
 * "auto".
 */
export function resolvedFormatKeys(): {
	number: Exclude<NumberFormatKey, 'auto'>;
	date: Exclude<DateFormatKey, 'auto'>;
	time: Exclude<TimeFormatKey, 'auto'>;
} {
	return {
		number: resolveNumber(preferences.numberFormat),
		date: resolveDate(preferences.dateFormat),
		time: resolveTime(preferences.timeFormat)
	};
}

/** Tests only: forget the detected `auto` values so a new stub is read. */
export function resetAutoDetectionForTests(): void {
	autoNumber = null;
	autoDate = null;
	autoTime = null;
}

// ---------------------------------------------------------------------------
// Number
// ---------------------------------------------------------------------------

const pad2 = (n: number) => (n < 10 ? '0' + n : '' + n);

function groupInteger(digits: string, group: string, indian: boolean): string {
	if (digits.length <= 3) return digits;
	if (!indian) return digits.replace(/\B(?=(\d{3})+(?!\d))/g, group);
	const head = digits.slice(0, -3);
	const tail = digits.slice(-3);
	return head.replace(/\B(?=(\d{2})+(?!\d))/g, group) + group + tail;
}

/** Group + decimal separators for the chosen (or a named) number preset. */
export function numberSeparators(key?: NumberFormatKey): { group: string; decimal: string } {
	const style = NUMBER_STYLES[resolveNumber(key ?? preferences.numberFormat)];
	return { group: style.group, decimal: style.decimal };
}

/**
 * Separators for seeding an EDITABLE numeric input: the preset's decimal mark,
 * but NO grouping — thousands separators must not jump around while typing.
 */
export function inputSeparators(key?: NumberFormatKey): { group: string; decimal: string } {
	return { group: '', decimal: numberSeparators(key).decimal };
}

/**
 * Normalise a person-typed, locale-formatted amount into a CANONICAL numeric
 * string (ASCII digits, `.` decimal, no grouping) that a parser or `BigInt` can
 * consume. Also maps Arabic-Indic digits, so a field showing "1 234,56" or
 * "١٢٣٤,٥٦" parses — and a plain "1234.56" from a foreign keyboard still does.
 */
export function parseLocaleNumber(text: string, key?: NumberFormatKey): string {
	const { group, decimal } = numberSeparators(key);
	let s = String(text ?? '').trim();
	s = s
		.replace(/[٠-٩]/g, (d) => String(d.charCodeAt(0) - 0x0660))
		.replace(/[۰-۹]/g, (d) => String(d.charCodeAt(0) - 0x06f0));
	if (/\s/.test(group))
		s = s.replace(/\s/g, ''); // space grouping: every whitespace kind
	else if (group) s = s.split(group).join('');
	if (decimal !== '.') s = s.split(decimal).join('.');
	return s;
}

/**
 * Group an INTEGER DIGIT STRING with the chosen preset's grouping (Indian 2-3
 * included) WITHOUT routing through a JS `number`. This is the bigint-safe
 * entry point: a uint256 base-unit string must never become a `number`, because
 * precision loss on a money surface is not a rounding error, it is a wrong
 * figure shown to someone about to sign.
 */
export function groupDigits(digits: string, key?: NumberFormatKey): string {
	const style = NUMBER_STYLES[resolveNumber(key ?? preferences.numberFormat)];
	return groupInteger(digits, style.group, style.indian);
}

interface NumberOpts {
	minimumFractionDigits?: number;
	maximumFractionDigits?: number;
	key?: NumberFormatKey;
}

/** Format a number with the chosen (or a named) preset. */
export function formatNumber(value: number, opts: NumberOpts = {}): string {
	if (!isFinite(value)) return '0';
	const style = NUMBER_STYLES[resolveNumber(opts.key ?? preferences.numberFormat)];
	const maxFrac = opts.maximumFractionDigits ?? 2;
	const minFrac = Math.min(opts.minimumFractionDigits ?? 0, maxFrac);

	const sign = value < 0 ? '-' : '';
	const fixed = Math.abs(value).toFixed(maxFrac);
	const split = fixed.split('.');
	const intPart = split[0];
	let fracPart = split[1] ?? '';
	while (fracPart.length > minFrac && fracPart.endsWith('0')) fracPart = fracPart.slice(0, -1);

	const grouped = groupInteger(intPart, style.group, style.indian);
	return sign + grouped + (fracPart ? style.decimal + fracPart : '');
}

// Compact tiers use the Latin suffixes a wallet reads everywhere. Locale
// myriads (CJK 万/億) are deliberately not used: K/M/B/T are what a price feed
// shows, and a figure a person compares against one has to match it.
const COMPACT_TIERS = [
	{ v: 1e12, s: 'T' },
	{ v: 1e9, s: 'B' },
	{ v: 1e6, s: 'M' },
	{ v: 1e3, s: 'K' }
] as const;

/**
 * Compact form for large magnitudes: 1234567.89 → "1.23M". The legibility floor
 * for hero amounts — once shrinking a full number would make it unreadable, the
 * caller abbreviates instead of going tiny.
 */
export function formatCompact(value: number, key?: NumberFormatKey): string {
	if (!isFinite(value)) return '0';
	const sign = value < 0 ? '-' : '';
	const abs = Math.abs(value);
	for (const tier of COMPACT_TIERS) {
		if (abs >= tier.v) {
			const scaled = abs / tier.v;
			const frac = scaled < 10 ? 2 : scaled < 100 ? 1 : 0;
			return sign + formatNumber(scaled, { maximumFractionDigits: frac, key }) + tier.s;
		}
	}
	return formatNumber(value, { maximumFractionDigits: abs < 1 ? 4 : 2, key });
}

/**
 * A token amount with magnitude-appropriate precision:
 * ≥1000 → 2 decimals · ≥1 → 4 · <1 → 6, so dust still reads as non-zero.
 */
export function formatTokenAmount(value: number, opts: { compact?: boolean } = {}): string {
	if (!isFinite(value) || value === 0) return '0';
	const abs = Math.abs(value);
	if (opts.compact && abs >= 1e6) return formatCompact(value);
	if (abs >= 1000)
		return formatNumber(value, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
	if (abs >= 1) return formatNumber(value, { maximumFractionDigits: 4 });
	if (!opts.compact) return formatNumber(value, { maximumFractionDigits: 6 });
	// The glanceable form caps at 4 — but not when that would round a tiny
	// amount down to "0", which would print a stray "+0" beside a real transfer.
	const capped = formatNumber(value, { maximumFractionDigits: 4 });
	return capped === '0' ? formatNumber(value, { maximumFractionDigits: 6 }) : capped;
}

// ---------------------------------------------------------------------------
// Date & time
// ---------------------------------------------------------------------------

/** Format a date (no time) with the chosen (or a named) preset. */
export function formatDate(input: Date | number, key?: DateFormatKey): string {
	const d = typeof input === 'number' ? new Date(input) : input;
	const y = d.getFullYear();
	const m = pad2(d.getMonth() + 1);
	const day = pad2(d.getDate());
	switch (resolveDate(key ?? preferences.dateFormat)) {
		case 'ymd_slash':
			return `${y}/${m}/${day}`;
		case 'iso':
			return `${y}-${m}-${day}`;
		case 'dmy_slash':
			return `${day}/${m}/${y}`;
		case 'dmy_dot':
			return `${day}.${m}.${y}`;
		case 'mdy_slash':
		default:
			return `${m}/${day}/${y}`;
	}
}

/** Format a time with the chosen (or a named) preset. */
export function formatTime(input: Date | number, key?: TimeFormatKey): string {
	const d = typeof input === 'number' ? new Date(input) : input;
	const h = d.getHours();
	const min = pad2(d.getMinutes());
	if (resolveTime(key ?? preferences.timeFormat) === 'h12') {
		const h12 = h % 12 === 0 ? 12 : h % 12;
		return `${h12}:${min} ${h < 12 ? 'AM' : 'PM'}`;
	}
	return `${pad2(h)}:${min}`;
}

/** Combined, e.g. "2026/06/13, 13:45" or "06/13/2026, 1:45 PM". */
export function formatDateTime(input: Date | number): string {
	const d = typeof input === 'number' ? new Date(input) : input;
	return `${formatDate(d)}, ${formatTime(d)}`;
}

// ---------------------------------------------------------------------------
// Option metadata for the pickers — the label IS a live example
// ---------------------------------------------------------------------------

/** 2026-06-13 13:45 — the one sample that distinguishes every order. */
const SAMPLE = new Date(2026, 5, 13, 13, 45);

/** `noteKey` is a semantic token, not display text: the UI translates it. */
export interface FormatOption<K> {
	key: K;
	example: string;
	noteKey?: 'system' | 'indian' | 'h24' | 'h12';
}

export function numberFormatOptions(): FormatOption<NumberFormatKey>[] {
	const keys: NumberFormatKey[] = ['auto', 'comma_dot', 'dot_comma', 'space_comma', 'indian'];
	return keys.map((key) => ({
		key,
		example: formatNumber(1234567.89, {
			minimumFractionDigits: 2,
			maximumFractionDigits: 2,
			key
		}),
		noteKey: key === 'auto' ? 'system' : key === 'indian' ? 'indian' : undefined
	}));
}

export function dateFormatOptions(): FormatOption<DateFormatKey>[] {
	const keys: DateFormatKey[] = ['auto', 'ymd_slash', 'mdy_slash', 'dmy_slash', 'dmy_dot', 'iso'];
	return keys.map((key) => ({
		key,
		example: formatDate(SAMPLE, key),
		noteKey: key === 'auto' ? 'system' : undefined
	}));
}

export function timeFormatOptions(): FormatOption<TimeFormatKey>[] {
	const keys: TimeFormatKey[] = ['auto', 'h24', 'h12'];
	return keys.map((key) => ({
		key,
		example: formatTime(SAMPLE, key),
		noteKey: key === 'auto' ? 'system' : key === 'h24' ? 'h24' : 'h12'
	}));
}
