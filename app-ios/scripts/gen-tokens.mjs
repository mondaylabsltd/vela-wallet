#!/usr/bin/env node
/**
 * iOS design-token pipeline (spec 009-ios-onboarding-swiftui, T002).
 *
 * Source  <-  docs/design-tokens.json                              Penpot DTCG export, THE value authority
 * Output  ->  app-ios/VelaWallet/VelaWallet/DesignSystem/Tokens.swift
 *
 * The output is COMMITTED; `--check` regenerates in memory, byte-compares with
 * the file on disk, and exits 1 on drift — same guarantee as the web pipeline
 * (app-web/vela-wallet/scripts/gen-tokens.mjs, same source file).
 *
 * iOS additions (tokens the export lacks) live in IOS_ADDITIONS below with the
 * docs/design-system.md rule that licenses each; nothing else may invent a value.
 */

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const SOURCE = join(APP_ROOT, '..', 'docs', 'design-tokens.json');
const OUT_SWIFT = join(APP_ROOT, 'VelaWallet', 'VelaWallet', 'DesignSystem', 'Tokens.swift');

/**
 * The ONLY place introducing values the DTCG export lacks; each entry cites
 * the docs/design-system.md rule (or web precedent) that licenses it.
 */
const IOS_ADDITIONS = {
	// docs/design-system.md §Layout & sizing: "Controls: use sizing.control.sm (36px),
	// sizing.control.md (44px), or sizing.control.lg (52px)."
	control: { sm: 36, md: 44, lg: 52 },
	// CTA label on accent.base, white in BOTH modes (fg.inverse flips);
	// web precedent: WEB_ADDITIONS `color-onAccent` in app-web gen-tokens.mjs.
	onAccent: '#FFFFFF',
	// Fallbacks ONLY if the export drops core.size.hitTarget / core.size.hitSlop
	// (docs/design-system.md: prefer 44px or larger for touch targets). The current
	// export carries both, so these are unused.
	hitTarget: 44,
	hitSlop: 8
};

function flatten(setObj, prefix = '') {
	const out = [];
	for (const [key, value] of Object.entries(setObj)) {
		if (key.startsWith('$')) continue;
		const path = prefix ? `${prefix}.${key}` : key;
		if (value && typeof value === 'object' && '$value' in value) {
			out.push({ path, type: value.$type, value: value.$value });
		} else if (value && typeof value === 'object') {
			out.push(...flatten(value, path));
		}
	}
	return out;
}

function loadTokens(sourcePath = SOURCE) {
	const doc = JSON.parse(readFileSync(sourcePath, 'utf8'));
	for (const set of ['core', 'color-light', 'color-dark']) {
		if (!doc[set]) throw new Error(`token source missing set "${set}"`);
	}
	const core = flatten(doc.core);
	const light = flatten(doc['color-light']);
	const dark = flatten(doc['color-dark']);
	const lightPaths = light.map((t) => t.path).join('\n');
	const darkPaths = dark.map((t) => t.path).join('\n');
	if (lightPaths !== darkPaths) {
		throw new Error('color-light and color-dark define different token paths');
	}
	return { core, light, dark };
}

/** "#RRGGBB", "#RRGGBBAA" or "rgba(r,g,b,a)" -> 0xAARRGGBB (unsigned). */
function parseColor(path, value) {
	const v = String(value).trim();
	let m = v.match(/^#([0-9a-fA-F]{6})([0-9a-fA-F]{2})?$/);
	if (m) {
		const rgb = parseInt(m[1], 16);
		const a = m[2] ? parseInt(m[2], 16) : 0xff;
		return a * 0x1000000 + rgb;
	}
	m = v.match(/^rgba\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*([\d.]+)\s*\)$/);
	if (m) {
		const a = Math.round(Number(m[4]) * 255);
		return a * 0x1000000 + Number(m[1]) * 0x10000 + Number(m[2]) * 0x100 + Number(m[3]);
	}
	throw new Error(`unparseable color: ${path} = ${value}`);
}

/** Core shadow string "x y blur spread rgba(...)" -> TokenShadow parts. */
function parseShadow(path, value) {
	const m = String(value).match(/^(-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (.+)$/);
	if (!m) throw new Error(`unparseable shadow: ${path} = ${value}`);
	const [x, y, blur, spread] = [m[1], m[2], m[3], m[4]].map(Number);
	if (spread !== 0) {
		throw new Error(`shadow spread unsupported in SwiftUI: ${path} = ${value}`);
	}
	return { x, y, radius: blur, argb: parseColor(path, m[5]) };
}

const hex = (argb) => `0x${argb.toString(16).toUpperCase().padStart(8, '0')}`;
const num = (v) => String(Number(v));

/** TokenPalette field -> export path within a color set. onAccent is appended. */
const PALETTE_FIELDS = [
	['bgBase', 'color.bg.base'],
	['bgRaised', 'color.bg.raised'],
	['bgSunken', 'color.bg.sunken'],
	['fgBase', 'color.fg.base'],
	['fgMuted', 'color.fg.muted'],
	['fgSubtle', 'color.fg.subtle'],
	['fgInverse', 'color.fg.inverse'],
	['accentBase', 'color.accent.base'],
	['accentSoft', 'color.accent.soft'],
	['successBase', 'color.success.base'],
	['successSoft', 'color.success.soft'],
	['warningBase', 'color.warning.base'],
	['warningSoft', 'color.warning.soft'],
	['errorBase', 'color.error.base'],
	['errorSoft', 'color.error.soft'],
	['infoBase', 'color.info.base'],
	['infoSoft', 'color.info.soft'],
	['borderBase', 'color.border.base'],
	['borderStrong', 'color.border.strong']
];

export function generateSwift(tokens = loadTokens()) {
	const { core, light, dark } = tokens;
	const coreMap = new Map(core.map((t) => [t.path, t]));
	const need = (path) => {
		const t = coreMap.get(path);
		if (!t) throw new Error(`token source missing core.${path}`);
		return t.value;
	};

	const paletteLiteral = (list, setName) => {
		const map = new Map(list.map((t) => [t.path, t]));
		const lines = PALETTE_FIELDS.map(([field, path]) => {
			const t = map.get(path);
			if (!t) throw new Error(`token source missing ${setName}.${path}`);
			return `        ${field}: TokenColor(argb: ${hex(parseColor(path, t.value))}), // ${t.value}`;
		});
		lines.push(
			`        onAccent: TokenColor(argb: ${hex(parseColor('onAccent', IOS_ADDITIONS.onAccent))}) // IOS_ADDITIONS: ${IOS_ADDITIONS.onAccent} both modes`
		);
		return `TokenPalette(\n${lines.join('\n')}\n    )`;
	};

	// Scale members named after their value (space "xl"=16 -> s16), so call
	// sites read as the design spec does ("space 16", "text 17").
	const scale = (prefix, letter) =>
		core
			.filter((t) => t.path.startsWith(`${prefix}.`))
			.map((t) => `        static let ${letter}${num(t.value)}: CGFloat = ${num(t.value)}`)
			.join('\n');

	const radii = core
		.filter((t) => t.path.startsWith('radius.') && t.path !== 'radius.full')
		.map((t) => `        static let r${num(t.value)}: CGFloat = ${num(t.value)}`)
		.join('\n');

	const weights = core
		.filter((t) => t.path.startsWith('weight.'))
		.map((t) => `        static let ${t.path.slice('weight.'.length)}: Int = ${num(t.value)}`)
		.join('\n');

	const leadings = core
		.filter((t) => t.path.startsWith('leading.'))
		.map((t) => `        static let ${t.path.slice('leading.'.length)}: CGFloat = ${num(t.value)}`)
		.join('\n');

	const opacities = core
		.filter((t) => t.path.startsWith('opacity.'))
		.map((t) => `        static let ${t.path.slice('opacity.'.length)}: Double = ${num(t.value)}`)
		.join('\n');

	// Durations ms -> seconds; "normal" is exposed as `base` (web-TS precedent).
	const motion = [
		['fast', 'motion.duration.fast'],
		['base', 'motion.duration.normal'],
		['slow', 'motion.duration.slow']
	]
		.map(
			([name, path]) =>
				`        static let ${name}: TimeInterval = ${num(Number(need(path)) / 1000)} // ${num(need(path))}ms`
		)
		.join('\n');

	const shadows = core
		.filter((t) => t.path.startsWith('shadow.'))
		.map((t) => {
			const s = parseShadow(t.path, t.value);
			return `        static let ${t.path.slice('shadow.'.length)} = TokenShadow(color: TokenColor(argb: ${hex(s.argb)}), radius: ${num(s.radius)}, x: ${num(s.x)}, y: ${num(s.y)})`;
		})
		.join('\n');

	// hitTarget/hitSlop come from core.size when the export carries them;
	// IOS_ADDITIONS is the licensed fallback otherwise.
	const sizeOr = (path, fallback) =>
		coreMap.has(path)
			? [num(coreMap.get(path).value), `core ${path}`]
			: [num(fallback), 'IOS_ADDITIONS fallback'];
	const [hitTarget, hitTargetSrc] = sizeOr('size.hitTarget', IOS_ADDITIONS.hitTarget);
	const [hitSlop, hitSlopSrc] = sizeOr('size.hitSlop', IOS_ADDITIONS.hitSlop);

	const controls = Object.entries(IOS_ADDITIONS.control)
		.map(([name, value]) => `        static let ${name}: CGFloat = ${num(value)}`)
		.join('\n');

	return `// GENERATED — do not edit. Regenerate: node app-ios/scripts/gen-tokens.mjs
// Source: docs/design-tokens.json (Penpot DTCG export — the design value authority).
// IOS_ADDITIONS entries are the only values the export lacks; each cites its
// docs/design-system.md license inside app-ios/scripts/gen-tokens.mjs.

import SwiftUI

/// A design-token color stored as 0xAARRGGBB.
struct TokenColor {
    let argb: UInt32

    var alpha: Double { Double((argb >> 24) & 0xFF) / 255.0 }
    var red: Double { Double((argb >> 16) & 0xFF) / 255.0 }
    var green: Double { Double((argb >> 8) & 0xFF) / 255.0 }
    var blue: Double { Double(argb & 0xFF) / 255.0 }

    var color: Color { Color(.sRGB, red: red, green: green, blue: blue, opacity: alpha) }
}

/// A design-token shadow. Export strings are "x y blur spread color" with
/// spread always 0 (SwiftUI has no spread); blur maps to \`radius\`.
struct TokenShadow {
    let color: TokenColor
    let radius: CGFloat
    let x: CGFloat
    let y: CGFloat
}

/// One appearance mode's semantic colors (sets color-light / color-dark).
struct TokenPalette {
    let bgBase: TokenColor
    let bgRaised: TokenColor
    let bgSunken: TokenColor
    let fgBase: TokenColor
    let fgMuted: TokenColor
    let fgSubtle: TokenColor
    let fgInverse: TokenColor
    let accentBase: TokenColor
    let accentSoft: TokenColor
    let successBase: TokenColor
    let successSoft: TokenColor
    let warningBase: TokenColor
    let warningSoft: TokenColor
    let errorBase: TokenColor
    let errorSoft: TokenColor
    let infoBase: TokenColor
    let infoSoft: TokenColor
    let borderBase: TokenColor
    let borderStrong: TokenColor
    /// IOS_ADDITIONS: label on accent surfaces, white in BOTH modes.
    let onAccent: TokenColor
}

enum Tokens {
    /// Set color-light.
    static let light = ${paletteLiteral(light, 'color-light')}

    /// Set color-dark.
    static let dark = ${paletteLiteral(dark, 'color-dark')}

    /// core space (member = point value: space "xl" 16 -> s16).
    enum Space {
${scale('space', 's')}
    }

    /// core text sizes (member = point value: text "xl" 17 -> t17).
    enum TextSize {
${scale('text', 't')}
    }

    /// core weight, as raw font-weight numbers.
    enum Weight {
${weights}
    }

    /// core radius (member = point value) + full.
    enum Radius {
${radii}
        static let full: CGFloat = ${num(need('radius.full'))}
    }

    /// core leading — line-height multipliers.
    enum Leading {
${leadings}
    }

    /// core opacity.
    enum Opacity {
${opacities}
    }

    /// core motion.duration, ms -> seconds ("normal" exposed as \`base\`).
    enum Motion {
${motion}
    }

    /// core layout + hit sizes.
    enum Layout {
        static let screenPaddingX: CGFloat = ${num(need('layout.screenPaddingX'))}
        static let maxContentWidth: CGFloat = ${num(need('layout.maxContentWidth'))}
        static let frameW: CGFloat = ${num(need('layout.frameW'))}
        static let frameH: CGFloat = ${num(need('layout.frameH'))}
        static let hitTarget: CGFloat = ${hitTarget} // ${hitTargetSrc}
        static let hitSlop: CGFloat = ${hitSlop} // ${hitSlopSrc}
    }

    /// core border widths.
    enum BorderWidth {
        static let hairline: CGFloat = ${num(need('border.hairline'))}
        static let emphasis: CGFloat = ${num(need('border.emphasis'))}
    }

    /// core shadow.
    enum Shadow {
${shadows}
    }

    /// IOS_ADDITIONS: docs/design-system.md sizing.control.sm/md/lg.
    enum Control {
${controls}
    }

    /// core letterSpacing.
    enum LetterSpacing {
        static let sectionLabel: CGFloat = ${num(need('letterSpacing.sectionLabel'))}
    }
}
`;
}

function main() {
	const check = process.argv.includes('--check');
	const swift = generateSwift();
	if (check) {
		let onDisk = '';
		try {
			onDisk = readFileSync(OUT_SWIFT, 'utf8');
		} catch {
			// missing file counts as drift
		}
		if (onDisk !== swift) {
			const oldLines = onDisk.split('\n');
			const newLines = swift.split('\n');
			let i = 0;
			while (i < Math.min(oldLines.length, newLines.length) && oldLines[i] === newLines[i]) i++;
			console.error(
				`Tokens.swift drifts from docs/design-tokens.json — run \`node app-ios/scripts/gen-tokens.mjs\`:\n` +
					`  ${OUT_SWIFT}\n` +
					`  first differing line ${i + 1}:\n` +
					`    on disk:   ${oldLines[i] ?? '<EOF>'}\n` +
					`    regenerated: ${newLines[i] ?? '<EOF>'}`
			);
			process.exit(1);
		}
		console.log('tokens in sync');
		return;
	}
	mkdirSync(dirname(OUT_SWIFT), { recursive: true });
	writeFileSync(OUT_SWIFT, swift);
	console.log(`wrote ${OUT_SWIFT}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
	main();
}
