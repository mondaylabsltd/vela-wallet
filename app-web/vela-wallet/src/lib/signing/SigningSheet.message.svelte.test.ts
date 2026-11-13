/**
 * A message on the signing sheet, in a browser (clear-signing audit cs-10,
 * cs-11).
 *
 * cs-10: "Click to sign in." over sixty empty lines and a transfer — the sheet
 * showed the top and a live confirm. A message that runs past the sheet's
 * body now says it is unread (`onmessageunread`) until its last line has been
 * in sight, and the core's gate holds the confirm meanwhile; one that fits
 * says nothing at all.
 *
 * cs-11: a hidden character's ⟨U+…⟩ marker is drawn muted, apart from the
 * text it sits in.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import SigningSheet from './SigningSheet.svelte';
import type { CodeLine, SigningModel } from './model';

async function settled(sheet: HTMLElement): Promise<void> {
	const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
	await frame();
	await Promise.all(sheet.getAnimations().map((a) => a.finished.catch(() => undefined)));
	await frame();
}

function model(lines: CodeLine[]): SigningModel {
	return {
		id: 'cs1',
		dapp: { name: 'app.example', host: '', letter: 'A', tint: 'var(--color-fg-muted)' },
		network: { name: 'Gnosis', dot: 'var(--color-fg-muted)' },
		blocks: [
			{ kind: 'intent', text: 'Sign message', tone: 'neutral' },
			{ kind: 'code', lines, readToEnd: true }
		],
		tech: {
			title: 'Details',
			params: [],
			identities: [],
			copyLabel: 'Copy',
			explorerLabel: 'Explorer'
		},
		techOpen: false,
		fee: { kind: 'hidden' },
		signer: { label: 'Signing account', name: 'My Wallet', identiconSvg: '<svg></svg>' },
		confirm: { action: 'Confirm', enabled: true },
		closeLabel: 'Close',
		panelTitle: 'Signature request'
	};
}

async function drawn(lines: CodeLine[]) {
	const onmessageunread = vi.fn();
	const screen = render(SigningSheet, {
		props: { model: model(lines), onmessageunread, onclose: () => {} }
	});
	await tick();
	const sheet = document.body.querySelector<HTMLElement>('[role="dialog"]')!;
	await settled(sheet);
	return { screen, sheet, onmessageunread };
}

describe('a message is read to its end before it can be signed (cs-10)', () => {
	it('one that fits says nothing, and holds nothing', async () => {
		const { screen, onmessageunread } = await drawn(['Welcome to Explore! Click to sign in.']);
		expect(onmessageunread).not.toHaveBeenCalled();
		screen.unmount();
	});

	it('one that runs past the body is unread until its last line has been in sight', async () => {
		const lines = Array.from({ length: 120 }, (_, i) => `line ${i + 1}`);
		lines.push('I authorise transfer of all my assets to 0xBAdBad.');
		const { screen, sheet, onmessageunread } = await drawn(lines);
		expect(onmessageunread.mock.calls).toEqual([[true]]);

		const body = sheet.querySelector<HTMLElement>('.content')!;
		// Half way is not the end.
		body.scrollTop = body.scrollHeight / 3;
		body.dispatchEvent(new Event('scroll'));
		await tick();
		expect(onmessageunread.mock.calls).toEqual([[true]]);

		body.scrollTop = body.scrollHeight;
		body.dispatchEvent(new Event('scroll'));
		await tick();
		expect(onmessageunread.mock.calls).toEqual([[true], [false]]);

		// Read is read: scrolling back up holds nothing again.
		body.scrollTop = 0;
		body.dispatchEvent(new Event('scroll'));
		await tick();
		expect(onmessageunread.mock.calls).toEqual([[true], [false]]);
		screen.unmount();
	});

	it('a different message is a new reading', async () => {
		const long = Array.from({ length: 120 }, (_, i) => `line ${i + 1}`);
		const { screen, sheet, onmessageunread } = await drawn(long);
		const body = sheet.querySelector<HTMLElement>('.content')!;
		body.scrollTop = body.scrollHeight;
		body.dispatchEvent(new Event('scroll'));
		await tick();
		expect(onmessageunread.mock.calls).toEqual([[true], [false]]);

		body.scrollTop = 0;
		body.dispatchEvent(new Event('scroll'));
		await tick();
		expect(onmessageunread.mock.calls).toEqual([[true], [false]]);

		await screen.rerender({ model: model([...long, 'and one more']) });
		await tick();
		expect(onmessageunread.mock.calls).toEqual([[true], [false], [true]]);
		screen.unmount();
	});
});

describe('a hidden character is drawn as a muted marker (cs-11)', () => {
	it('stands apart from the text it sits in', async () => {
		const { screen, sheet } = await drawn([
			'Approve transfer to',
			[{ text: 'Bob ' }, { text: '⟨U+202E⟩', muted: true }, { text: 'gpj.exe' }]
		]);
		const code = sheet.querySelector<HTMLElement>('pre')!;
		// Line by line, and nothing between them but the break.
		expect(code.textContent).toBe('Approve transfer to\nBob ⟨U+202E⟩gpj.exe');
		const marker = code.querySelector<HTMLElement>('.muted')!;
		expect(marker.textContent).toBe('⟨U+202E⟩');
		expect(getComputedStyle(marker).color).not.toBe(getComputedStyle(code).color);
		screen.unmount();
	});
});
