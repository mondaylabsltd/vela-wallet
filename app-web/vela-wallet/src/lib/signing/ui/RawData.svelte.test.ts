/**
 * Technical details' raw data, drawn (`RawData`): laid out and highlighted,
 * copied as it came, a site's markup kept as text, its hidden characters as
 * markers under the sheet's caution, and a huge payload drawn on request.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import RawData from './RawData.svelte';
import { RAW_CAP } from '../raw-json';
import type { RawDataModel } from '../model';

const WORDS: RawDataModel['words'] = {
	copy: 'Copy raw data',
	copied: 'Copied',
	showAll: ['Show all (', ' lines)'],
	jsonText: '(a string, shown as JSON)',
	hidden: 'This request contains hidden or direction-changing characters.'
};

/** Built, not written: Svelte's compiler warns at a bidi control in a string literal. */
const RLO = String.fromCodePoint(0x202e);
const PDF = String.fromCodePoint(0x202c);

const model = (hex: string): RawDataModel => ({ label: 'Raw call data', hex, words: WORDS });

const CALLDATA = '0x3593564c' + '0'.repeat(63) + '6' + 'f'.repeat(64);
/** As a dApp sends it: compact, with a space a JSON.stringify would not keep. */
const REQUEST = `[{"chainId":"0x38","gas":"0x24f2b", "value":"0x2386f26fc10000","from":"0x14fb0000000000000000000000000000000000aa","to":"0xdc2600000000000000000000000000000000000b","data":"${CALLDATA}"}]`;

let host: HTMLDivElement | null = null;

function mount(hex: string, width = 320) {
	host = document.createElement('div');
	host.style.cssText = `width:${width}px;box-sizing:border-box;`;
	document.body.appendChild(host);
	return render(RawData, { target: host, props: { raw: model(hex) } });
}

const code = () => host!.querySelector<HTMLElement>('[data-testid="raw-code"]')!;
const lineTexts = () => [...code().querySelectorAll('.line')].map((line) => line.textContent);

afterEach(() => {
	host?.remove();
	host = null;
	vi.restoreAllMocks();
});

describe('the raw data, laid out', () => {
	it('one field a line, indented, its calldata in its selector and words', async () => {
		mount(REQUEST);
		await tick();
		expect(lineTexts()).toEqual([
			'[',
			'{',
			'"chainId": "0x38",',
			'"gas": "0x24f2b",',
			'"value": "0x2386f26fc10000",',
			'"from": "0x14fb0000000000000000000000000000000000aa",',
			'"to": "0xdc2600000000000000000000000000000000000b",',
			'"data": "0x3593564c',
			'0'.repeat(63) + '6',
			'f'.repeat(64) + '"',
			'}',
			']'
		]);
		const lines = code().querySelectorAll<HTMLElement>('.line');
		const indentOf = (el: HTMLElement) => parseFloat(getComputedStyle(el).paddingInlineStart);
		expect(indentOf(lines[2])).toBeGreaterThan(indentOf(lines[1]));
		expect(indentOf(lines[8])).toBeGreaterThan(indentOf(lines[7]));
		expect(lines[8].classList.contains('hex')).toBe(true);
	});

	it('each kind in its own neutral ink — the values strongest, the structure faintest', async () => {
		mount('{"k":"v","n":1,"t":true}');
		await tick();
		const ink = (selector: string) => getComputedStyle(code().querySelector(selector)!).color;
		const root = getComputedStyle(document.documentElement);
		const token = (name: string) => {
			const probe = document.createElement('span');
			probe.style.color = root.getPropertyValue(name);
			document.body.appendChild(probe);
			const color = getComputedStyle(probe).color;
			probe.remove();
			return color;
		};
		expect(ink('.string')).toBe(token('--color-fg-base'));
		expect(ink('.number')).toBe(token('--color-fg-base'));
		expect(ink('.literal')).toBe(token('--color-fg-base'));
		expect(ink('.key')).toBe(token('--color-fg-muted'));
		expect(ink('.punct')).toBe(token('--color-fg-subtle'));
		expect(ink('.string')).not.toBe(token('--color-accent-base'));
	});

	it('wraps inside a 320 px sheet: nothing runs past its edge', async () => {
		mount(REQUEST, 288);
		await tick();
		await document.fonts.ready;
		const right = host!.getBoundingClientRect().right;
		const range = document.createRange();
		range.selectNodeContents(code());
		for (const rect of range.getClientRects()) expect(rect.right).toBeLessThanOrEqual(right + 0.5);
		expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(window.innerWidth);
	});

	it('not JSON: as it came, unhighlighted', async () => {
		mount(CALLDATA);
		await tick();
		expect(lineTexts()).toEqual([CALLDATA]);
		expect(code().classList.contains('json')).toBe(false);
	});
});

describe('the copy button', () => {
	it('copies the ORIGINAL string, byte for byte, and says "Copied"', async () => {
		const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		const screen = mount(REQUEST);
		const button = screen.getByRole('button', { name: 'Copy raw data' });
		await button.click();
		expect(writeText).toHaveBeenCalledTimes(1);
		expect(writeText.mock.calls[0][0]).toBe(REQUEST);
		expect(writeText.mock.calls[0][0]).not.toBe(code().textContent);
		await expect.element(screen.getByRole('status')).toHaveTextContent('Copied');
	});

	it('copies everything of a capped payload, hidden characters and all', async () => {
		const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		const huge = JSON.stringify([{ to: `a${RLO}b`, data: '0x3593564c' + 'ab'.repeat(100_000) }]);
		const screen = mount(huge);
		await screen.getByRole('button', { name: 'Copy raw data' }).click();
		expect(writeText.mock.calls[0][0]).toBe(huge);
	});

	it('a refused clipboard says nothing was copied', async () => {
		vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('denied'));
		const screen = mount(REQUEST);
		await screen.getByRole('button', { name: 'Copy raw data' }).click();
		await tick();
		expect(host!.querySelector('.copied')).toBeNull();
		expect(screen.getByRole('status').element().textContent).toBe('');
	});
});

describe('a site’s hostile text', () => {
	it('markup is text: no element, no handler, the characters on screen', async () => {
		const raw = JSON.stringify([
			{ '<script>window.__pwned=1</script>': '"><img src=x onerror="window.__pwned=2">' }
		]);
		mount(raw);
		await tick();
		expect(code().querySelector('script, img')).toBeNull();
		expect(code().textContent).toContain('<script>window.__pwned=1</script>');
		expect(code().textContent).toContain('<img src=x onerror=');
		expect((window as { __pwned?: number }).__pwned).toBeUndefined();
	});

	it('markup that is not JSON is text too', async () => {
		mount('<img src=x onerror="window.__pwned=3"><b>bold</b>');
		await tick();
		expect(code().querySelector('img, b')).toBeNull();
		expect((window as { __pwned?: number }).__pwned).toBeUndefined();
	});

	it('a right-to-left override is its marker, under the hidden-character caution', async () => {
		const screen = mount(JSON.stringify({ to: `Bob ${RLO}gpj.exe${PDF} ok` }));
		await tick();
		expect(code().textContent).not.toContain(RLO);
		expect(code().textContent).not.toContain(PDF);
		expect(code().textContent).toContain('Bob ⟨U+202E⟩gpj.exe⟨U+202C⟩ ok');
		expect(code().querySelectorAll('.hidden')).toHaveLength(2);
		await expect.element(screen.getByText(WORDS.hidden)).toBeVisible();
	});

	it('no caution when there is nothing hidden', async () => {
		const screen = mount(REQUEST);
		await tick();
		expect(screen.getByText(WORDS.hidden).elements()).toHaveLength(0);
	});
});

describe('a huge payload', () => {
	const huge = JSON.stringify([{ to: '0x01', data: '0x3593564c' + 'ab'.repeat(100_000) }]);

	it('draws its first lines, then everything on "Show all"', async () => {
		const screen = mount(huge);
		await tick();
		expect(lineTexts().length).toBeLessThanOrEqual(RAW_CAP.lines);
		const showAll = screen.getByRole('button', { name: /^Show all \(\d+ lines\)$/ });
		await expect.element(showAll).toBeVisible();
		await showAll.click();
		await tick();
		expect(lineTexts().length).toBeGreaterThan(3000);
		expect(screen.getByRole('button', { name: /^Show all/ }).elements()).toHaveLength(0);
	});

	it('a short one has no "Show all"', async () => {
		const screen = mount(REQUEST);
		await tick();
		expect(screen.getByRole('button', { name: /^Show all/ }).elements()).toHaveLength(0);
	});
});
