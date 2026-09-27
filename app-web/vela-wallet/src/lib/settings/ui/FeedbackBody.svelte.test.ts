/**
 * The report sheet's screenshots (078 round 3), in a browser: taking them
 * from the picker and from a paste, the five-image cap, re-indexing on
 * remove, a file that is not an image, what Send hands over, and the filed /
 * fallback endings that mention them. Nothing here reaches the network — the
 * sheet only calls `onsend`; the route's send is tested in bug-report.test.ts.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { words } from '../__fixtures__/words';
import { buildMobileState } from '../fixtures';
import type { FeedbackModel, FeedbackResult } from '../model';
import { ReportDraft } from '../report-draft.svelte';
import FeedbackBody from './FeedbackBody.svelte';

const m = words();
const IDENTICON = (seed: string) => `<svg data-seed="${seed}"></svg>`;

function panel(): FeedbackModel {
	const base = buildMobileState('st1', m, IDENTICON).feedback;
	return {
		...base,
		screenshots: {
			...base.screenshots,
			remove: 'Remove screenshot {{index}}',
			view: 'View screenshot {{index}}',
			close: 'Close',
			removeFromViewer: 'Remove'
		}
	};
}

async function image(name: string, width = 320, height = 640): Promise<File> {
	const canvas = new OffscreenCanvas(width, height);
	const context = canvas.getContext('2d') as OffscreenCanvasRenderingContext2D;
	context.fillStyle = '#e8572a';
	context.fillRect(0, 0, width, height);
	const blob = await canvas.convertToBlob({ type: 'image/png' });
	return new File([blob], name, { type: 'image/png' });
}

async function drawn(
	props: {
		result?: FeedbackResult;
		sending?: boolean;
		ondone?: () => void;
		onsend?: (report: { what: string; steps: string; screenshots: string[] }) => void;
		draft?: ReportDraft;
	} = {}
) {
	const sent: { what: string; steps: string; screenshots: string[] }[] = [];
	const screen = render(FeedbackBody, {
		props: { panel: panel(), onsend: (report) => sent.push(report), ...props }
	});
	screen.container.style.width = '360px';
	await tick();
	const root = screen.container;
	const input = root.querySelector('input[type="file"]') as HTMLInputElement;
	const pick = async (files: File[]) => {
		const transfer = new DataTransfer();
		for (const file of files) transfer.items.add(file);
		input.files = transfer.files;
		input.dispatchEvent(new Event('change', { bubbles: true }));
		await tick();
	};
	const tiles = () => [...root.querySelectorAll<HTMLElement>('li.tile')];
	/** Every tile has its processed image. */
	const settled = () =>
		vi.waitFor(() => {
			for (const tile of tiles()) expect(tile.querySelector('img')).not.toBeNull();
		});
	const note = () => root.querySelector<HTMLElement>('.shots-note');
	return { screen, root, input, pick, tiles, settled, note, sent };
}

describe('FeedbackBody — screenshots', () => {
	it('starts with one add target and the "up to 5" hint, and no public line', async () => {
		const { root, tiles, note } = await drawn();
		expect(root.querySelector('.add-target')).not.toBeNull();
		expect(root.querySelector('.shots-count')?.textContent?.trim()).toBe(panel().screenshots.hint);
		expect(tiles()).toHaveLength(0);
		expect(note()).toBeNull();
	});

	it('shows picked images as tiles, the count, and says they will be public', async () => {
		const { root, pick, tiles, settled, note } = await drawn();
		await pick([await image('a.png'), await image('b.png')]);
		expect(tiles()).toHaveLength(2);
		await settled();
		expect(root.querySelector('.shots-count')?.textContent?.trim()).toBe('2 / 5');
		expect(note()?.textContent?.trim()).toBe(String(panel().screenshots.public));
		// The add tile stays while there is room.
		expect(root.querySelector('.add-tile')).not.toBeNull();
	});

	it('takes five at most — the rest are dropped, the limit is said, and the public line stays', async () => {
		const { root, pick, tiles, settled, note } = await drawn();
		const seven = await Promise.all([...'abcdefg'].map((c) => image(`${c}.png`)));
		await pick(seven);
		expect(tiles()).toHaveLength(5);
		await settled();
		// The refusal is its own line, ABOVE the public warning — never in its
		// place (v2 A1).
		const notes = [...root.querySelectorAll<HTMLElement>('.shots-note')];
		expect(notes.map((n) => n.textContent?.trim())).toEqual([
			String(panel().screenshots.limit),
			String(panel().screenshots.public)
		]);
		expect(note()?.classList.contains('refused')).toBe(true);
		expect(root.querySelector('.add-tile')).toBeNull();
	});

	it('keeps all five tiles in one row, shrinking them together on a narrow column', async () => {
		const { screen, root, pick, tiles, settled } = await drawn();
		await pick(await Promise.all([...'abcde'].map((c) => image(`${c}.png`))));
		await settled();
		screen.container.style.width = '260px';
		await tick();
		const boxes = tiles().map((t) => t.getBoundingClientRect());
		expect(new Set(boxes.map((b) => Math.round(b.top))).size).toBe(1);
		for (const box of boxes) {
			expect(box.width).toBeLessThan(72);
			expect(Math.abs(box.width - box.height)).toBeLessThan(1);
		}
		// Wide: 72 each, and still one row with the add tile gone at five.
		screen.container.style.width = '600px';
		await tick();
		for (const tile of tiles()) expect(Math.round(tile.getBoundingClientRect().width)).toBe(72);
		expect(root.querySelector('.add-tile')).toBeNull();
	});

	it('draws the remove badge opaque, and grows its tap area outward — not over the picture', async () => {
		const { screen, pick, tiles, settled } = await drawn();
		await pick(await Promise.all([...'abcde'].map((c) => image(`${c}.png`))));
		await settled();
		screen.container.style.width = '342px';
		await tick();
		for (const tile of tiles()) {
			const badge = tile.querySelector('.badge') as HTMLElement;
			// Opaque (v3 B1): no alpha in the disc.
			// (`color(srgb r g b)` from color-mix; an alpha would read "/ a" or rgba.)
			expect(getComputedStyle(badge).backgroundColor).not.toMatch(/\/|rgba/);
			// A round disc of its full size — never squeezed by the tap area.
			const disc = badge.getBoundingClientRect();
			expect(Math.round(disc.width)).toBe(22);
			expect(Math.round(disc.height)).toBe(22);
			const area = (tile.querySelector('.remove') as HTMLElement).getBoundingClientRect();
			const box = tile.getBoundingClientRect();
			expect(area.width).toBeGreaterThanOrEqual(44);
			// Over the picture only as far as the badge (22), never ~half the tile (v3 B10).
			const inside =
				Math.max(0, Math.min(area.right, box.right) - Math.max(area.left, box.left)) *
				Math.max(0, Math.min(area.bottom, box.bottom) - Math.max(area.top, box.top));
			expect(inside).toBeLessThanOrEqual(23 * 23);
		}
		// The last badge ends on the column edge (only its 4 overhang is kept
		// as padding), under the "n / 5" counter.
		const count = screen.container.querySelector('.shots-count') as HTMLElement;
		const lastBadge = tiles()[4].querySelector('.badge') as HTMLElement;
		expect(
			Math.abs(lastBadge.getBoundingClientRect().right - count.getBoundingClientRect().right)
		).toBeLessThanOrEqual(1);
		// A tap in the gap-and-corner an area reaches over lands on THAT
		// area, not on the next tile's picture painted after it.
		const next = tiles()[1].getBoundingClientRect();
		const hit = document.elementFromPoint(next.left + 3, next.top + 3);
		expect(hit?.closest('.remove')).toBe(tiles()[0].querySelector('.remove'));
	});

	it('writes the hint and the counter in fg-muted, not the failing fg-subtle', async () => {
		const { root, pick, settled } = await drawn();
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-fg-muted)';
		document.body.appendChild(probe);
		const muted = getComputedStyle(probe).color;
		probe.remove();
		const count = () => getComputedStyle(root.querySelector('.shots-count') as HTMLElement).color;
		expect(count()).toBe(muted);
		expect(getComputedStyle(root.querySelector('.drop-hint') as HTMLElement).color).toBe(muted);
		await pick([await image('a.png')]);
		await settled();
		expect(count()).toBe(muted);
	});

	it('holds the form still while sending — fields, tiles and paste', async () => {
		const { root, tiles } = await drawn({ sending: true });
		expect((root.querySelector('textarea') as HTMLTextAreaElement).inert).toBe(true);
		expect((root.querySelector('section.shots') as HTMLElement).inert).toBe(true);
		const transfer = new DataTransfer();
		transfer.items.add(await image('pasted.png'));
		window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: transfer }));
		await tick();
		expect(tiles()).toHaveLength(0);
	});

	it('re-indexes the remove labels when a tile goes', async () => {
		const { pick, tiles, settled } = await drawn();
		await pick([await image('a.png'), await image('b.png'), await image('c.png')]);
		await settled();
		const label = () => tiles().map((t) => t.querySelector('.remove')?.getAttribute('aria-label'));
		expect(label()).toEqual(['Remove screenshot 1', 'Remove screenshot 2', 'Remove screenshot 3']);
		(tiles()[0].querySelector('.remove') as HTMLButtonElement).click();
		await tick();
		await vi.waitFor(() => expect(tiles()).toHaveLength(2));
		expect(label()).toEqual(['Remove screenshot 1', 'Remove screenshot 2']);
	});

	it('refuses a file that is not an image, and one the browser cannot decode', async () => {
		const { pick, tiles, note } = await drawn();
		await pick([new File(['%PDF'], 'doc.pdf', { type: 'application/pdf' })]);
		expect(tiles()).toHaveLength(0);
		expect(note()?.textContent?.trim()).toBe(String(panel().screenshots.unsupported));
		await pick([new File(['not really'], 'photo.heic', { type: 'image/heic' })]);
		await vi.waitFor(() => expect(tiles()).toHaveLength(0));
		expect(note()?.textContent?.trim()).toBe(String(panel().screenshots.unsupported));
	});

	it('takes an image pasted anywhere while the sheet is open', async () => {
		const { tiles, settled } = await drawn();
		const transfer = new DataTransfer();
		transfer.items.add(await image('pasted.png'));
		window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: transfer }));
		await tick();
		expect(tiles()).toHaveLength(1);
		await settled();
	});

	it('hands Send the processed JPEGs, in tile order', async () => {
		const { root, pick, settled, sent } = await drawn();
		await pick([await image('wide.png', 900, 300), await image('tall.png', 300, 900)]);
		await settled();
		const field = root.querySelector('textarea') as HTMLTextAreaElement;
		field.value = 'The send froze';
		field.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		const send = [...root.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === String(panel().send)
		) as HTMLButtonElement;
		send.click();
		await tick();
		expect(sent).toHaveLength(1);
		expect(sent[0].what).toBe('The send froze');
		expect(sent[0].screenshots).toHaveLength(2);
		const sizes = await Promise.all(
			sent[0].screenshots.map(async (b64) => {
				const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
				expect([...bytes.subarray(0, 3)]).toEqual([0xff, 0xd8, 0xff]);
				const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/jpeg' }));
				return [bitmap.width, bitmap.height];
			})
		);
		expect(sizes).toEqual([
			[900, 300],
			[300, 900]
		]);
	});

	/**
	 * Holds every decode until `release()`: the tiles stay "being prepared"
	 * for as long as the test needs them to.
	 */
	function holdDecodes() {
		const original = window.createImageBitmap.bind(window);
		let release!: () => void;
		const gate = new Promise<void>((resolve) => (release = resolve));
		const spy = vi
			.spyOn(window, 'createImageBitmap')
			.mockImplementation(async (...args: Parameters<typeof createImageBitmap>) => {
				await gate;
				return original(...args);
			});
		return { release, restore: () => spy.mockRestore() };
	}

	async function typeAndSend(root: HTMLElement) {
		const field = root.querySelector('textarea') as HTMLTextAreaElement;
		field.value = 'The send froze';
		field.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		const send = [...root.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === String(panel().send)
		) as HTMLButtonElement;
		send.click();
		await tick();
	}

	it('waits for a tile still being prepared, busy, then sends it — never drops it', async () => {
		const held = holdDecodes();
		try {
			const { root, pick, tiles, sent } = await drawn();
			await pick([await image('late.png', 400, 200)]);
			expect(tiles()[0].querySelector('.processing')).not.toBeNull();
			await typeAndSend(root);
			// Not sent yet, and saying so: the spinner and the sending words.
			expect(sent).toHaveLength(0);
			const busy = root.querySelector('button[aria-busy="true"]') as HTMLButtonElement;
			expect(busy.querySelector('.busy')?.textContent?.trim()).toBe(String(panel().sending));
			held.release();
			await vi.waitFor(() => expect(sent).toHaveLength(1));
			expect(sent[0].screenshots).toHaveLength(1);
			const bytes = Uint8Array.from(atob(sent[0].screenshots[0]), (c) => c.charCodeAt(0));
			expect([...bytes.subarray(0, 3)]).toEqual([0xff, 0xd8, 0xff]);
		} finally {
			held.restore();
		}
	});

	it('drops a tile that fails while Send waits, says so, and sends the rest', async () => {
		const held = holdDecodes();
		try {
			const { root, pick, tiles, sent, note } = await drawn();
			await pick([
				await image('fine.png'),
				new File(['not really'], 'photo.heic', { type: 'image/heic' })
			]);
			expect(tiles()).toHaveLength(2);
			await typeAndSend(root);
			expect(sent).toHaveLength(0);
			held.release();
			await vi.waitFor(() => expect(sent).toHaveLength(1));
			expect(sent[0].screenshots).toHaveLength(1);
			// (The dropped tile leaves on its 150 ms scale-out.)
			await vi.waitFor(() => expect(tiles()).toHaveLength(1));
			expect(note()?.textContent?.trim()).toBe(String(panel().screenshots.unsupported));
		} finally {
			held.restore();
		}
	});

	it('is busy with a spinner and the sending words in the button, never dimmed', async () => {
		const { root } = await drawn({ sending: true });
		const field = root.querySelector('textarea') as HTMLTextAreaElement;
		field.value = 'x';
		field.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		const busy = [...root.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === String(panel().sending)
		) as HTMLButtonElement;
		// Busy, said by a spinner at full emphasis — the button takes no second
		// tap, but it is not drawn as a dead one (the founder's rule).
		expect(busy).toBeDefined();
		expect(busy.getAttribute('aria-busy')).toBe('true');
		expect(busy.querySelector('.spinner')).not.toBeNull();
		expect(getComputedStyle(busy).opacity).toBe('1');
		// The words are drawn, not hidden behind the spinner.
		const words = busy.querySelector('.busy') as HTMLElement;
		expect(words.textContent?.trim()).toBe(String(panel().sending));
		expect(getComputedStyle(words).visibility).toBe('visible');
		expect(words.getBoundingClientRect().width).toBeGreaterThan(0);
	});
});

describe('FeedbackBody — what will be sent', () => {
	it('lays each environment line out as a label and its value, split at the first ": "', async () => {
		const { root } = await drawn();
		const labels = [...root.querySelectorAll('.env dt')].map((n) => n.textContent);
		const values = [...root.querySelectorAll('.env dd')].map((n) => n.textContent);
		const lines = panel().previewLines;
		expect(labels).toHaveLength(lines.length);
		expect(labels.map((l, i) => `${l}: ${values[i]}`)).toEqual(lines.map(String));
		// No colon on screen at all (v3 B2) — the column is the separator; the
		// payload keeps "label: value".
		for (const label of labels) expect(label).not.toMatch(/[:：]\s*$/);
		// Not a code block any more (v2 A4).
		expect(getComputedStyle(root.querySelector('.env dd') as HTMLElement).fontFamily).not.toMatch(
			/mono/i
		);
	});

	it('says the consent quietly — no tinted box', async () => {
		const { root } = await drawn();
		const consent = root.querySelector('.consent') as HTMLElement;
		expect(getComputedStyle(consent).backgroundColor).toBe('rgba(0, 0, 0, 0)');
	});
});

describe('FeedbackBody — the endings', () => {
	it('filed: the centred state, the dropped line when images could not be stored, and Done', async () => {
		const ondone = vi.fn();
		const { screen, root } = await drawn({ ondone });
		// The outcome ARRIVES while the sheet is open.
		await screen.rerender({
			result: {
				filed: true,
				number: 12,
				url: 'https://github.com/x/y/issues/12',
				screenshotsDropped: 1
			}
		});
		const title = root.querySelector('.filed-title') as HTMLElement;
		expect(title.textContent?.trim()).toBe(String(panel().success.title));
		// Focused (v3 B7): focus never falls back to the body. A labelled
		// group, not a live region — the focus move is the announcement.
		const block = root.querySelector('.filed-block') as HTMLElement;
		expect(block.getAttribute('role')).toBe('group');
		expect(block.getAttribute('aria-labelledby')).toBe(title.id);
		await vi.waitFor(() => expect(document.activeElement).toBe(title));
		expect(root.querySelector('.filed-dropped')?.textContent).toBe(String(panel().success.dropped));
		// Images lost: the issue page is where they get added, so View leads.
		const view = root.querySelector('a[href="https://github.com/x/y/issues/12"]') as HTMLElement;
		expect(view.classList.contains('primary')).toBe(true);
		// Done is the plain text button (v3 B3): no pill, no border.
		const done = root.querySelector('button.done') as HTMLButtonElement;
		expect(done.textContent?.trim()).toBe(String(panel().success.done));
		expect(done.classList.contains('button')).toBe(false);
		expect(getComputedStyle(done).borderTopStyle).toBe('none');
		done.click();
		expect(ondone).toHaveBeenCalledOnce();
	});

	it('filed without trouble says nothing about screenshots, and uses no accent', async () => {
		const { root } = await drawn({
			result: {
				filed: true,
				number: 3,
				url: 'https://github.com/x/y/issues/3',
				screenshotsDropped: 0
			}
		});
		expect(root.querySelector('.filed-dropped')).toBeNull();
		expect(root.querySelector('.filed .primary')).toBeNull();
		// View outlined, Done the quiet text under it (v3 B3).
		expect(root.querySelectorAll('.filed .secondary')).toHaveLength(1);
		expect(root.querySelector('.filed button.done')).not.toBeNull();
	});

	it('fell back with screenshots: three parts, the form cannot carry them, and that is said', async () => {
		const { root } = await drawn({
			result: { filed: false, fallbackUrl: 'https://github.com/new?x', withScreenshots: true }
		});
		const parts = [...root.querySelectorAll('.fallback-text p')].map((p) => p.textContent?.trim());
		// Never "title — body" (v2 A2).
		expect(parts).toEqual([
			String(panel().fallback.title),
			String(panel().fallback.body),
			String(panel().fallback.screenshots)
		]);
		const without = await drawn({
			result: { filed: false, fallbackUrl: 'https://github.com/new?x' }
		});
		expect(without.root.querySelector('.fallback')?.textContent).not.toContain(
			String(panel().fallback.screenshots)
		);
	});

	it('reopened on a thank-you it already showed: no focus grab, no scroll', async () => {
		const scrolled = vi.spyOn(Element.prototype, 'scrollIntoView');
		try {
			const filed = await drawn({ result: { filed: true, number: 3, screenshotsDropped: 0 } });
			await tick();
			await new Promise((resolve) => setTimeout(resolve, 50));
			expect(scrolled).not.toHaveBeenCalled();
			expect(document.activeElement).not.toBe(filed.root.querySelector('.filed-title'));
		} finally {
			scrolled.mockRestore();
		}
	});

	it('reopened on a fallback (B6): the form’s road is brought into view and its title focused', async () => {
		// A sheet-sized scroller, the form taller than it: the road starts below the fold.
		const host = document.createElement('div');
		host.style.cssText = 'height: 420px; overflow-y: auto; width: 360px';
		document.body.appendChild(host);
		try {
			render(FeedbackBody, {
				target: host,
				props: {
					panel: panel(),
					onsend: () => {},
					result: { filed: false, fallbackUrl: 'https://github.com/new?x', withScreenshots: true }
				}
			});
			await tick();
			const road = host.querySelector('.fallback-road') as HTMLElement;
			await vi.waitFor(() => {
				expect(road.getBoundingClientRect().bottom).toBeLessThanOrEqual(
					host.getBoundingClientRect().bottom + 1
				);
			});
			expect(host.scrollTop).toBeGreaterThan(0);
			expect(document.activeElement).toBe(host.querySelector('.fallback-title'));
			// Its "Open GitHub form" is in view with it.
			const open = road.querySelector('a[href="https://github.com/new?x"]') as HTMLElement;
			expect(open.getBoundingClientRect().bottom).toBeLessThanOrEqual(
				host.getBoundingClientRect().bottom + 1
			);
		} finally {
			host.remove();
		}
	});

	it('fell back: its title takes focus, scrolled into view with its button', async () => {
		const scrolled = vi.spyOn(Element.prototype, 'scrollIntoView');
		try {
			const { screen, root } = await drawn();
			await screen.rerender({ result: { filed: false, fallbackUrl: 'https://github.com/new?x' } });
			const title = root.querySelector('.fallback-title') as HTMLElement;
			const block = root.querySelector('.fallback') as HTMLElement;
			// Not an alert as well as the focus move — that was read twice.
			expect(block.getAttribute('role')).toBe('group');
			expect(block.getAttribute('aria-labelledby')).toBe(title.id);
			await vi.waitFor(() => expect(document.activeElement).toBe(title));
			// The block AND the "open the form" button travel as one (v3 B6).
			const road = root.querySelector('.fallback-road') as HTMLElement;
			expect(road.querySelector('a[href="https://github.com/new?x"]')).not.toBeNull();
			expect(scrolled.mock.contexts).toContain(road);
			expect(scrolled.mock.calls.at(-1)?.[0]).toMatchObject({ block: 'nearest' });
		} finally {
			scrolled.mockRestore();
		}
	});

	it('fell back: the send button reads "try again", and the bottom GitHub link steps aside', async () => {
		const { root } = await drawn({
			result: { filed: false, fallbackUrl: 'https://github.com/new?x' }
		});
		const labels = [...root.querySelectorAll('button')].map((b) => b.textContent?.trim());
		expect(labels).toContain(String(panel().fallback.retry));
		expect(labels).not.toContain(String(panel().send));
		expect(root.querySelector('a.github')).toBeNull();
		// Before any send, the link is there.
		const fresh = await drawn();
		expect(fresh.root.querySelector('a.github')).not.toBeNull();
	});
});

// 078 §C: a tile opens the screenshot large — the processed JPEG, what goes public.
describe('FeedbackBody — the viewer', () => {
	const viewer = () => document.querySelector<HTMLElement>('.viewer');
	const counter = () => viewer()?.querySelector('.counter')?.textContent?.trim();
	/** The picture on screen (phone: the current page of the strip). */
	const shown = () =>
		viewer()?.querySelector<HTMLImageElement>('.page:not([aria-hidden]) img, .picture')?.src;
	const key = async (k: string) => {
		window.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true }));
		await tick();
	};
	/** The viewer's exit plays, then it is gone. */
	const gone = () => vi.waitFor(() => expect(viewer()).toBeNull());

	async function withThree() {
		const view = await drawn();
		await view.pick([await image('a.png'), await image('b.png', 640, 320), await image('c.png')]);
		await view.settled();
		const srcs = view.tiles().map((t) => (t.querySelector('img') as HTMLImageElement).src);
		const open = async (i: number) => {
			(view.tiles()[i].querySelector('.view') as HTMLButtonElement).click();
			await tick();
		};
		return { ...view, srcs, open };
	}

	it('each ready tile is a button named "View screenshot n"', async () => {
		const { tiles } = await withThree();
		expect(tiles().map((t) => t.querySelector('.view')?.getAttribute('aria-label'))).toEqual([
			'View screenshot 1',
			'View screenshot 2',
			'View screenshot 3'
		]);
	});

	it('opens at the tile tapped, showing the processed image, named and counted', async () => {
		const { open, srcs } = await withThree();
		await open(1);
		expect(viewer()).not.toBeNull();
		expect(viewer()?.getAttribute('role')).toBe('dialog');
		expect(viewer()?.getAttribute('aria-label')).toBe('View screenshot 2');
		expect(counter()).toBe('2 / 3');
		// The tile's own blob — the re-encoded JPEG, never the picked file.
		expect(shown()).toBe(srcs[1]);
		// Focus moved in.
		expect(viewer()?.contains(document.activeElement)).toBe(true);
		await key('Escape');
		await gone();
	});

	it('pages with the arrow keys, and stops at either end', async () => {
		const { open, srcs } = await withThree();
		await open(1);
		await key('ArrowRight');
		expect(counter()).toBe('3 / 3');
		expect(shown()).toBe(srcs[2]);
		await key('ArrowRight');
		expect(counter()).toBe('3 / 3');
		await key('ArrowLeft');
		await key('ArrowLeft');
		expect(counter()).toBe('1 / 3');
		expect(viewer()?.getAttribute('aria-label')).toBe('View screenshot 1');
		await key('Escape');
		await gone();
	});

	it('pages with a swipe, and a short one springs back', async () => {
		const { open } = await withThree();
		await open(0);
		const stage = viewer()?.querySelector('.stage') as HTMLElement;
		const box = stage.getBoundingClientRect();
		const y = box.top + box.height / 2;
		// A finger's pace (~30 ms a sample): a burst would read as a flick.
		const drag = async (from: number, to: number) => {
			const at = (x: number, type: string) =>
				stage.dispatchEvent(
					new PointerEvent(type, { pointerId: 7, clientX: x, clientY: y, bubbles: true, button: 0 })
				);
			at(from, 'pointerdown');
			for (let i = 1; i <= 6; i++) {
				await new Promise((r) => setTimeout(r, 30));
				at(from + ((to - from) * i) / 6, 'pointermove');
			}
			await new Promise((r) => setTimeout(r, 120));
			at(to, 'pointerup');
			await tick();
		};
		await drag(box.left + box.width * 0.8, box.left + box.width * 0.1);
		expect(counter()).toBe('2 / 3');
		// Well under a fifth of the width, and slow: back where it was.
		await new Promise((r) => setTimeout(r, 150));
		await drag(box.left + box.width * 0.5, box.left + box.width * 0.45);
		expect(counter()).toBe('2 / 3');
		await key('Escape');
		await gone();
	});

	it('swiped down past a fifth, it closes; a short pull springs back', async () => {
		const { open } = await withThree();
		await open(0);
		const stage = viewer()?.querySelector('.stage') as HTMLElement;
		const box = stage.getBoundingClientRect();
		const x = box.left + box.width / 2;
		const pull = async (distance: number) => {
			const at = (y: number, type: string) =>
				stage.dispatchEvent(
					new PointerEvent(type, { pointerId: 9, clientX: x, clientY: y, bubbles: true, button: 0 })
				);
			const top = box.top + box.height * 0.3;
			at(top, 'pointerdown');
			for (let i = 1; i <= 8; i++) {
				at(top + (distance * i) / 8, 'pointermove');
				await new Promise((r) => setTimeout(r, 30));
			}
			at(top + distance, 'pointerup');
			await tick();
		};
		await pull(box.height * 0.08);
		await new Promise((r) => setTimeout(r, 300));
		expect(viewer()).not.toBeNull();
		await pull(box.height * 0.4);
		await gone();
	});

	it('removes from the viewer: the middle shows the next, the last the previous, the only closes', async () => {
		const { open, tiles, srcs } = await withThree();
		await open(1);
		const remove = () => (viewer()?.querySelector('.remove-text') as HTMLButtonElement).click();
		remove();
		await tick();
		await vi.waitFor(() => expect(tiles()).toHaveLength(2));
		expect(counter()).toBe('2 / 2');
		expect(shown()).toBe(srcs[2]);
		// Now the last: the previous takes its place.
		remove();
		await tick();
		await vi.waitFor(() => expect(tiles()).toHaveLength(1));
		expect(shown()).toBe(srcs[0]);
		// One image: no counter.
		expect(counter()).toBeUndefined();
		// The only one: the viewer closes, and focus lands on the add target.
		remove();
		await gone();
		await vi.waitFor(() => expect(tiles()).toHaveLength(0));
		await vi.waitFor(() =>
			expect(document.activeElement?.classList.contains('add-target')).toBe(true)
		);
	});

	it('closing returns focus to the tile that opened it — or the tile now in its place', async () => {
		const { open, tiles } = await withThree();
		await open(1);
		await key('ArrowRight');
		await key('Escape');
		await gone();
		// The OPENER, not the one paged to.
		await vi.waitFor(() => expect(document.activeElement).toBe(tiles()[1].querySelector('.view')));
		// Opened on 2 and removed there: focus goes to the tile that took its place.
		await open(1);
		(viewer()?.querySelector('.remove-text') as HTMLButtonElement).click();
		await tick();
		await key('Escape');
		await gone();
		await vi.waitFor(() => expect(tiles()).toHaveLength(2));
		await vi.waitFor(() => expect(document.activeElement).toBe(tiles()[1].querySelector('.view')));
	});

	it('keeps Tab inside itself', async () => {
		const { open } = await withThree();
		await open(0);
		const buttons = [...(viewer() as HTMLElement).querySelectorAll('button')];
		for (let i = 0; i < buttons.length + 2; i++) {
			await key('Tab');
			expect(viewer()?.contains(document.activeElement)).toBe(true);
		}
		await key('Escape');
		await gone();
	});

	it('a tile still being prepared opens nothing', async () => {
		const original = window.createImageBitmap.bind(window);
		let release!: () => void;
		const gate = new Promise<void>((resolve) => (release = resolve));
		const spy = vi
			.spyOn(window, 'createImageBitmap')
			.mockImplementation(async (...args: Parameters<typeof createImageBitmap>) => {
				await gate;
				return original(...args);
			});
		try {
			const { pick, tiles } = await drawn();
			await pick([await image('late.png')]);
			expect(tiles()[0].querySelector('.processing')).not.toBeNull();
			expect(tiles()[0].querySelector('.view')).toBeNull();
			(tiles()[0] as HTMLElement).click();
			await tick();
			expect(viewer()).toBeNull();
			release();
			await vi.waitFor(() => expect(tiles()[0].querySelector('.view')).not.toBeNull());
		} finally {
			spy.mockRestore();
		}
	});

	it('opens nothing while the report is sending', async () => {
		const { screen, tiles, pick, settled } = await drawn();
		await pick([await image('a.png')]);
		await settled();
		await screen.rerender({ sending: true });
		(tiles()[0].querySelector('.view') as HTMLButtonElement).click();
		await tick();
		expect(viewer()).toBeNull();
	});

	it('at desktop width it is a lightbox: arrows at the sides, the scrim closes it', async () => {
		await page.viewport(1600, 1000);
		try {
			const { open } = await withThree();
			await open(0);
			expect(viewer()?.classList.contains('lightbox')).toBe(true);
			// Nothing to go back to from the first; the next is named by where it goes.
			expect(viewer()?.querySelector('.nav.prev')).toBeNull();
			const next = viewer()?.querySelector('.nav.next') as HTMLButtonElement;
			expect(next.getAttribute('aria-label')).toBe('View screenshot 2');
			next.click();
			await tick();
			expect(counter()).toBe('2 / 3');
			expect(viewer()?.querySelector('.nav.prev')).not.toBeNull();
			// The picture is at most 90% × 85% of the window.
			const picture = (viewer()?.querySelector('.picture') as HTMLElement).getBoundingClientRect();
			expect(picture.width).toBeLessThanOrEqual(1600 * 0.9 + 1);
			expect(picture.height).toBeLessThanOrEqual(1000 * 0.85 + 1);
			(viewer()?.querySelector('.scrim') as HTMLElement).click();
			await gone();
		} finally {
			await page.viewport(414, 896);
		}
	});
});

// iPhone, 2026-09-27: a scroll that began on a remove badge deleted the
// screenshot. The badge (and the tile) act on a tap, never on a scroll.
describe('FeedbackBody — a tap, not a scroll', () => {
	async function oneTile() {
		const view = await drawn();
		await view.pick([await image('a.png')]);
		await view.settled();
		const badge = view.tiles()[0].querySelector('.remove') as HTMLButtonElement;
		const box = badge.getBoundingClientRect();
		const press = (type: string, dx = 0, dy = 0) =>
			badge.dispatchEvent(
				new PointerEvent(type, {
					pointerId: 3,
					pointerType: 'touch',
					clientX: box.left + box.width / 2 + dx,
					clientY: box.top + box.height / 2 + dy,
					bubbles: true
				})
			);
		const click = async () => {
			badge.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
			await tick();
		};
		return { ...view, badge, press, click };
	}

	it('a touch that moves past the slop from the badge does not remove', async () => {
		const { press, click, tiles } = await oneTile();
		press('pointerdown');
		press('pointermove', 0, 30);
		press('pointerup', 0, 30);
		await click();
		expect(tiles()).toHaveLength(1);
	});

	it('a touch the browser took for a scroll (pointercancel) does not remove', async () => {
		const { press, click, tiles } = await oneTile();
		press('pointerdown');
		press('pointermove', 0, 4);
		press('pointercancel', 0, 4);
		await click();
		expect(tiles()).toHaveLength(1);
	});

	it('a still tap removes, and so does the keyboard', async () => {
		const first = await oneTile();
		first.press('pointerdown');
		first.press('pointermove', 2, 2);
		first.press('pointerup', 2, 2);
		await first.click();
		await vi.waitFor(() => expect(first.tiles()).toHaveLength(0));
		const second = await oneTile();
		// Enter / Space: a click with no pointer (detail 0).
		second.badge.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 0 }));
		await tick();
		await vi.waitFor(() => expect(second.tiles()).toHaveLength(0));
	});

	it('leaves the badge to the browser for scrolling — no touch-action of its own', async () => {
		const { badge, tiles } = await oneTile();
		expect(getComputedStyle(badge).touchAction).not.toBe('none');
		const tile = tiles()[0].querySelector('.view') as HTMLElement;
		expect(getComputedStyle(tile).touchAction).not.toBe('none');
	});

	it('a scroll that starts on a tile does not open it', async () => {
		const { tiles } = await oneTile();
		const tile = tiles()[0].querySelector('.view') as HTMLButtonElement;
		const box = tile.getBoundingClientRect();
		const at = (type: string, dy: number) =>
			tile.dispatchEvent(
				new PointerEvent(type, {
					pointerId: 4,
					pointerType: 'touch',
					clientX: box.left + 10,
					clientY: box.top + 10 + dy,
					bubbles: true
				})
			);
		at('pointerdown', 0);
		at('pointermove', 40);
		tile.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
		await tick();
		expect(document.querySelector('.viewer')).toBeNull();
	});
});

// 078: the route hands in the app-resident draft, so closing the sheet never
// loses the report — not the words, not the images, not a send that waits.
describe('FeedbackBody — the draft outlives the sheet', () => {
	it('reopened, the sheet shows what was typed and attached', async () => {
		const draft = new ReportDraft();
		const first = await drawn({ draft });
		await first.pick([await image('a.png')]);
		await first.settled();
		const field = first.root.querySelector('textarea') as HTMLTextAreaElement;
		field.value = 'Balance shows 0';
		field.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		first.screen.unmount();
		const again = await drawn({ draft });
		expect((again.root.querySelector('textarea') as HTMLTextAreaElement).value).toBe(
			'Balance shows 0'
		);
		expect(again.tiles()).toHaveLength(1);
		expect(again.tiles()[0].querySelector('img')).not.toBeNull();
	});

	it('a send waiting on a tile still goes when the sheet is closed meanwhile', async () => {
		const original = window.createImageBitmap.bind(window);
		let release!: () => void;
		const gate = new Promise<void>((resolve) => (release = resolve));
		const spy = vi
			.spyOn(window, 'createImageBitmap')
			.mockImplementation(async (...args: Parameters<typeof createImageBitmap>) => {
				await gate;
				return original(...args);
			});
		try {
			const draft = new ReportDraft();
			const sent: { screenshots: string[] }[] = [];
			const view = await drawn({ draft, onsend: (report) => sent.push(report) });
			await view.pick([await image('late.png')]);
			const field = view.root.querySelector('textarea') as HTMLTextAreaElement;
			field.value = 'The send froze';
			field.dispatchEvent(new Event('input', { bubbles: true }));
			await tick();
			const send = [...view.root.querySelectorAll('button')].find(
				(b) => b.textContent?.trim() === String(panel().send)
			) as HTMLButtonElement;
			send.click();
			await tick();
			expect(draft.waiting).toBe(true);
			view.screen.unmount();
			release();
			await vi.waitFor(() => expect(sent).toHaveLength(1));
			expect(sent[0].screenshots).toHaveLength(1);
			expect(draft.waiting).toBe(false);
		} finally {
			spy.mockRestore();
		}
	});

	it('a sheet with no draft of its own discards it when it goes', async () => {
		const view = await drawn();
		await view.pick([await image('a.png')]);
		await view.settled();
		const url = (view.tiles()[0].querySelector('img') as HTMLImageElement).src;
		view.screen.unmount();
		// The blob was revoked with the sheet.
		await expect(fetch(url)).rejects.toThrow();
	});
});

// 078 design review, second pass.
describe('FeedbackBody — review fixes', () => {
	const viewer = () => document.querySelector<HTMLElement>('.viewer');
	const gone = () => vi.waitFor(() => expect(viewer()).toBeNull());

	async function withTwo() {
		const view = await drawn();
		await view.pick([await image('a.png'), await image('b.png')]);
		await view.settled();
		(view.tiles()[0].querySelector('.view') as HTMLButtonElement).click();
		await tick();
		return view;
	}

	it('a swipe down hides the ✕, the counter and Remove at once, and a spring back brings them back', async () => {
		await withTwo();
		const stage = viewer()?.querySelector('.stage') as HTMLElement;
		const bars = () => [...(viewer() as HTMLElement).querySelectorAll('.bar')];
		const box = stage.getBoundingClientRect();
		const x = box.left + box.width / 2;
		const top = box.top + box.height * 0.3;
		const at = (y: number, type: string) =>
			stage.dispatchEvent(
				new PointerEvent(type, { pointerId: 5, clientX: x, clientY: y, bubbles: true, button: 0 })
			);
		// Past the slop (8), short of a flick's minimum (16): a pull that
		// decides "dismiss" and can only spring back.
		at(top, 'pointerdown');
		at(top + 12, 'pointermove');
		await tick();
		for (const bar of bars()) expect(bar.classList.contains('hidden')).toBe(true);
		await new Promise((r) => setTimeout(r, 200));
		at(top + 12, 'pointerup');
		await tick();
		for (const bar of bars()) expect(bar.classList.contains('hidden')).toBe(false);
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await gone();
	});

	it('closed by a tap, focus returns quietly; closed from the keyboard, with its ring', async () => {
		const focus = vi.spyOn(HTMLElement.prototype, 'focus');
		try {
			const { tiles } = await withTwo();
			const tile = tiles()[0].querySelector('.view') as HTMLElement;
			// A pointer's click on ✕ (detail 1).
			(viewer()?.querySelector('.close') as HTMLElement).dispatchEvent(
				new MouseEvent('click', { bubbles: true, detail: 1 })
			);
			await gone();
			await vi.waitFor(() => expect(document.activeElement).toBe(tile));
			const quiet = focus.mock.calls.findLast((_, i) => focus.mock.contexts[i] === tile);
			expect(quiet?.[0]).toMatchObject({ focusVisible: false });
			// Escape.
			tile.click();
			await tick();
			window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
			await gone();
			await vi.waitFor(() => {
				const loud = focus.mock.calls.findLast((_, i) => focus.mock.contexts[i] === tile);
				expect(loud?.[0]).toMatchObject({ focusVisible: true });
			});
		} finally {
			focus.mockRestore();
		}
	});

	it('the fallback’s body is muted and its screenshots line smaller, under a title in the body colour', async () => {
		const { root } = await drawn({
			result: { filed: false, fallbackUrl: 'https://github.com/new?x', withScreenshots: true }
		});
		const probe = (color: string) => {
			const el = document.createElement('span');
			el.style.color = color;
			document.body.appendChild(el);
			const value = getComputedStyle(el).color;
			el.remove();
			return value;
		};
		const [title, body, shots] = [...root.querySelectorAll<HTMLElement>('.fallback-text p')];
		expect(getComputedStyle(title).color).toBe(probe('var(--color-fg-base)'));
		expect(getComputedStyle(body).color).toBe(probe('var(--color-fg-muted)'));
		expect(getComputedStyle(shots).color).toBe(probe('var(--color-fg-muted)'));
		expect(parseFloat(getComputedStyle(shots).fontSize)).toBeLessThan(
			parseFloat(getComputedStyle(body).fontSize)
		);
	});

	it('the thank-you sits at the 2:3 optical centre of the pane it is seen in', async () => {
		const host = document.createElement('div');
		host.style.cssText = 'height: 900px; overflow-y: auto; width: 560px';
		document.body.appendChild(host);
		try {
			render(FeedbackBody, {
				target: host,
				props: { panel: panel(), result: { filed: true, number: 3, screenshotsDropped: 0 } }
			});
			await tick();
			const block = host.querySelector('.filed-block') as HTMLElement;
			const frame = host.getBoundingClientRect();
			const box = block.getBoundingClientRect();
			const free = frame.height - box.height;
			const above = box.top - frame.top;
			// Two parts of the free room above, three below.
			expect(above / free).toBeGreaterThan(0.35);
			expect(above / free).toBeLessThan(0.45);
		} finally {
			host.remove();
		}
	});
});
