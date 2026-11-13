/**
 * "0x" stays "0x" — measured in a real browser, in the faces the app draws in.
 *
 * A face's contextual alternates (`calt`) may turn `x` after a digit into `×`
 * and `-` into `−`: Plus Jakarta Sans, the face this app once bundled, set an
 * address as "0×14fB…" and the no-P-256 hint as "RIP-7212 at 0×100" (found on
 * the iPhone, 2026-10-09). The app now draws in the system's faces
 * (docs/FONT-POLICY.md), which are not trusted to have no such rule, so
 * `app.css` still turns contextual alternates off on the body, which
 * everything inherits, and on the form controls, which inherit no font
 * settings. `tokens.test.ts` reads that declaration; this checks what it does.
 *
 * A glyph cannot be read back from the DOM, but its advance and the computed
 * style can: for every place hex is drawn — plain text, each form control,
 * every weight, beside `tabular-nums` — the string as the app draws it is as
 * wide as with `calt` forced off, and the computed setting is the body's.
 */
import { describe, expect, it } from 'vitest';
import '$lib/tokens/tokens.css';
import '../../app.css';

const WEIGHTS = ['400', '500', '600', '700'] as const;

/** Hex as the screens hold it: an address, its short form, a hash, and hex inside a sentence. */
const HEX = [
	'0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
	'0xD40086…de130b',
	'0x9f3ca71b04e82f5c55d9b21ae00734f8dd8021aed400866e00b055b20752a826',
	"This network has no P-256 verifier (RIP-7212 at 0x100), so Vela wallets can't work here.",
	'chain 0x1 · 0x100',
	// The same rule turns a hyphen after a digit into a minus sign.
	'2026-10-09'
];

type Draw = { tag?: string; style?: string };

/** The advance of `text` drawn in `tag` (as a control's value where it is one), with `style`. */
function width(text: string, { tag = 'span', style = '' }: Draw = {}): number {
	const host = document.createElement('div');
	host.style.cssText = 'position:absolute;inset-inline-start:0;top:0;white-space:nowrap;';
	const el = document.createElement(tag);
	el.setAttribute('style', style);
	let measured: HTMLElement = el;
	if (tag === 'input' || tag === 'textarea') {
		// A control's value is not its box: measure a twin that carries the
		// control's own computed font settings, which is what draws the value.
		(el as HTMLInputElement).value = text;
		host.appendChild(el);
		document.body.appendChild(host);
		const from = getComputedStyle(el);
		const twin = document.createElement('span');
		twin.textContent = text;
		twin.style.cssText = [
			`font-family:${from.fontFamily}`,
			`font-size:${from.fontSize}`,
			`font-weight:${from.fontWeight}`,
			`font-variant-ligatures:${from.fontVariantLigatures}`,
			`font-variant-numeric:${from.fontVariantNumeric}`,
			`font-feature-settings:${from.fontFeatureSettings}`,
			`letter-spacing:${from.letterSpacing}`,
			`text-transform:${from.textTransform}`,
			'white-space:nowrap'
		].join(';');
		host.appendChild(twin);
		measured = twin;
	} else if (tag === 'select') {
		const option = document.createElement('option');
		option.textContent = text;
		el.appendChild(option);
		host.appendChild(el);
		document.body.appendChild(host);
	} else {
		el.textContent = text;
		host.appendChild(el);
		document.body.appendChild(host);
	}
	const w = measured.getBoundingClientRect().width;
	host.remove();
	return w;
}

const UI = 'font-family:var(--font-ui);font-size:16px;';
const ON = 'font-variant-ligatures:normal;font-feature-settings:"calt" 1;';
const OFF = 'font-variant-ligatures:no-contextual;font-feature-settings:"calt" 0;';

describe('contextual alternates are off wherever hex is drawn', () => {
	it.each(HEX)('plain text keeps its "x" and its hyphen: %s', (text) => {
		for (const weight of WEIGHTS) {
			// As the app draws it: the body's settings, inherited.
			const style = `${UI}font-weight:${weight};`;
			const drawn = width(text, { style });
			expect(drawn, weight).toBeCloseTo(width(text, { style: style + OFF }), 1);
		}
	});

	it.each(['input', 'textarea', 'button', 'select'])(
		'a %s keeps them too — form controls inherit no font settings',
		(tag) => {
			for (const text of HEX.slice(0, 3)) {
				const style = `${UI}font-weight:500;`;
				const drawn = width(text, { tag, style });
				expect(drawn, `${tag}: ${text}`).toBeCloseTo(width(text, { tag, style: style + OFF }), 1);
			}
			// The setting itself, whatever the face: a control does not inherit
			// it, so it is the control's own rule that must carry it.
			const control = document.createElement(tag);
			document.body.appendChild(control);
			expect(getComputedStyle(control).fontVariantLigatures, tag).toBe('no-contextual');
			control.remove();
		}
	);

	it('beside `tabular-nums`, nothing brings the rule back', () => {
		for (const text of HEX) {
			const style = `${UI}font-weight:500;font-variant-numeric:tabular-nums;`;
			expect(width(text, { style }), text).toBeCloseTo(width(text, { style: style + OFF }), 1);
		}
		// No rule in the app uses the `font-variant` shorthand or sets font
		// features (either would reset the body's choice): the computed value
		// on a figure cell is still the body's.
		const cell = document.createElement('span');
		cell.style.cssText = 'font-variant-numeric:tabular-nums;';
		document.body.appendChild(cell);
		expect(getComputedStyle(cell).fontVariantLigatures).toBe('no-contextual');
		expect(getComputedStyle(cell).fontFeatureSettings).toBe('normal');
		cell.remove();
	});

	it('the mono face — where addresses are compared character by character — draws them as written', () => {
		for (const text of HEX.slice(0, 3)) {
			const mono = 'font-family:var(--font-mono);font-size:16px;';
			expect(width(text, { style: mono + ON }), text).toBeCloseTo(
				width(text, { style: mono + OFF }),
				1
			);
		}
	});
});
