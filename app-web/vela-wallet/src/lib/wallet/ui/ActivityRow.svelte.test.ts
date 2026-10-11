/**
 * 087 F11: a dApp call that moved no coin of ours has no figure. Its row drew
 * an empty amount cell anyway (a lone " " on Android, a gap here); it draws
 * none, so the site name has the row.
 */
import '$lib/tokens/tokens.css';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ActivityRowModel, ActivityTitlePart } from '../model';
import ActivityRow from './ActivityRow.svelte';

function row(partial: Partial<ActivityRowModel>): ActivityRowModel {
	return {
		kind: 'dapp',
		title: 'dApp transaction',
		subtitle: 'Pending · app.uniswap.org',
		amount: '',
		unit: '',
		positive: false,
		masked: false,
		badgeColor: '#000',
		...partial
	};
}

describe('ActivityRow', () => {
	it('draws no amount cell for a row with no figure', () => {
		const screen = render(ActivityRow, { props: { row: row({}) } });
		expect(screen.container.querySelector('.amount')).toBeNull();
		expect(screen.container.textContent).toContain('app.uniswap.org');
	});

	it('draws what came back on its own when nothing left (097 N5)', () => {
		const screen = render(ActivityRow, {
			props: { row: row({ received: { amount: '+0.3', unit: 'USDC' } }) }
		});
		const cells = screen.container.querySelectorAll('.amount');
		expect(cells).toHaveLength(1);
		expect(cells[0].classList.contains('back')).toBe(true);
		expect(cells[0].textContent).toContain('+0.3');
		expect(cells[0].textContent).toContain('USDC');
	});

	it('keeps the amount cell for a row with one', () => {
		const screen = render(ActivityRow, {
			props: { row: row({ kind: 'sent', amount: '−0.01', unit: 'xDAI' }) }
		});
		expect(screen.container.querySelector('.amount')?.textContent).toContain('−0.01');
	});
});

/**
 * Device walk of 2026-10-11, item 1. The owner's row read 「在
 * arcorbs.game3.app 合 / 约交互」 on the phone: the title was one wrapping
 * text, and a wrapping text breaks wherever the width runs out. The web drew
 * it the same way. A title now wraps only BETWEEN what was done and where; the
 * action is never broken; the site is cut in its middle if it must be.
 *
 * The parts are the live builder's (`dappTitleParts`, held to every language
 * in `live-activity.test.ts`); these are the rows a home and a History draw,
 * at a 320 and a 390 px phone's list width, in zh and en, with a long host,
 * with and without a figure beside the title.
 */
describe('a dApp row’s title wraps only between what was done and where', () => {
	const en = (host: string, head: string, tail: string): [string, ActivityTitlePart[]] => [
		`Contract interaction on ${host}`,
		[
			{ kind: 'action', text: 'Contract interaction ' },
			{ kind: 'place', before: 'on ', head, tail, after: '' }
		]
	];
	const zh = (host: string, head: string, tail: string): [string, ActivityTitlePart[]] => [
		`在 ${host} 合约交互`,
		[
			{ kind: 'place', before: '在 ', head, tail, after: ' ' },
			{ kind: 'action', text: '合约交互' }
		]
	];
	const HOSTS = [
		['arcorbs.game3.app', 'arcorbs', '.game3.app'],
		['a-very-long-subdomain-of.arcorbs.game3.app', 'a-very-long-subdomain-of.arcorbs', '.game3.app']
	] as const;
	/** A phone's list: the screen less its two gutters. */
	const WIDTHS = [288, 358];
	const FIGURES = [
		{ amount: '', unit: '' },
		{ amount: '−0.0003', unit: 'BNB' }
	];

	function drawn(row: ActivityRowModel, width: number) {
		const host = document.createElement('div');
		host.style.width = `${width}px`;
		document.body.appendChild(host);
		const screen = render(ActivityRow, { target: host, props: { row, onclick: () => {} } });
		return {
			host,
			title: host.querySelector<HTMLElement>('.title')!,
			done: () => {
				screen.unmount();
				host.remove();
			}
		};
	}

	/** The lines a piece of text is drawn on: the distinct tops of its boxes. */
	function linesOf(node: Node, start = 0, end = node.textContent?.length ?? 0): number {
		const range = document.createRange();
		range.setStart(node, start);
		range.setEnd(node, end);
		const tops = [...range.getClientRects()]
			.filter((box) => box.width > 0)
			.map((box) => Math.round(box.top));
		return new Set(tops).size;
	}

	/** How wide a part is when nothing makes it wrap. */
	function oneLineWidth(part: HTMLElement): number {
		const probe = part.cloneNode(true) as HTMLElement;
		probe.style.cssText = 'position:absolute;white-space:pre;visibility:hidden;width:max-content';
		part.parentElement!.appendChild(probe);
		const width = probe.getBoundingClientRect().width;
		probe.remove();
		return width;
	}

	const CASES = [en, zh].flatMap((language) =>
		HOSTS.flatMap(([host, head, tail]) =>
			WIDTHS.flatMap((width) =>
				FIGURES.map((figure) => ({ language, host, head, tail, width, figure }))
			)
		)
	);

	it('the action is whole on one line, the site is one line, and a line ends only between them', () => {
		for (const { language, host, head, tail, width, figure } of CASES) {
			const [title, titleParts] = language(host, head, tail);
			const at = `${title} @ ${width}${figure.amount ? ' beside a figure' : ''}`;
			const view = drawn(row({ title, titleParts, ...figure }), width);
			const action = view.title.querySelector<HTMLElement>('.action')!;
			const place = view.title.querySelector<HTMLElement>('.place')!;
			// Nothing is lost or added: the text is the title exactly.
			expect(view.title.textContent, at).toBe(title);
			// What was done: never broken — unless it is, ALONE, wider than
			// the whole column ("Contract interaction" beside a figure at
			// 320): then it gives between its own words, and only there.
			const column = view.host.querySelector('.text')!.getBoundingClientRect();
			const alone = oneLineWidth(action);
			const actionLines = linesOf(action.firstChild!);
			if (alone <= column.width) {
				expect(actionLines, at).toBe(1);
			} else {
				const words = action.textContent!.trim().split(/\s+/);
				expect(words.length, at).toBeGreaterThan(1);
				expect(actionLines, at).toBeLessThanOrEqual(words.length);
				for (const word of words) {
					const from = action.textContent!.indexOf(word);
					expect(linesOf(action.firstChild!, from, from + word.length), `${at} · ${word}`).toBe(1);
				}
			}
			// Where: one line too.
			const lineHeight = action.getBoundingClientRect().height / actionLines;
			expect(place.getBoundingClientRect().height, at).toBeLessThan(lineHeight * 1.5);
			// One line or two; two means one part on each.
			const tops = new Set(
				[action, place].map((part) => Math.round(part.getBoundingClientRect().top))
			);
			const titleHeight = view.title.getBoundingClientRect().height;
			expect(titleHeight, at).toBeLessThan(lineHeight * (tops.size + actionLines - 0.5));
			// Inside the row's text column: nothing runs out of it, and the
			// figure beside it keeps the row's end.
			for (const part of [action, place]) {
				expect(part.getBoundingClientRect().right, at).toBeLessThanOrEqual(column.right + 0.5);
			}
			const figures = view.host.querySelector('.figures');
			if (figures) {
				expect(figures.getBoundingClientRect().right, at).toBeLessThanOrEqual(
					view.host.getBoundingClientRect().right + 0.5
				);
			}
			view.done();
		}
	});

	it('a site too long for its line is cut in its middle: the head ends in "…", its end is whole', () => {
		const [title, titleParts] = zh(...HOSTS[1]);
		const view = drawn(row({ title, titleParts }), 288);
		const head = view.title.querySelector<HTMLElement>('.head')!;
		const [before, after] = view.title.querySelectorAll<HTMLElement>('.place .join');
		expect(head.scrollWidth).toBeGreaterThan(head.clientWidth);
		expect(getComputedStyle(head).textOverflow).toBe('ellipsis');
		// 「在 」 and the host's end keep their width, and end inside the row.
		expect(before.scrollWidth).toBeLessThanOrEqual(before.clientWidth + 1);
		expect(after.textContent).toBe('.game3.app ');
		expect(after.scrollWidth).toBeLessThanOrEqual(after.clientWidth + 1);
		expect(after.getBoundingClientRect().right).toBeLessThanOrEqual(
			view.host.getBoundingClientRect().right + 0.5
		);
		view.done();

		// One that fits is drawn whole.
		const short = zh(...HOSTS[0]);
		const fits = drawn(row({ title: short[0], titleParts: short[1] }), 358);
		const whole = fits.title.querySelector<HTMLElement>('.head')!;
		expect(whole.scrollWidth).toBeLessThanOrEqual(whole.clientWidth + 1);
		fits.done();
	});

	it('as ONE wrapping text the same title broke inside the action — what the phone showed', () => {
		const [title] = zh(...HOSTS[0]);
		// Narrow enough that 「合约交互」 does not fit after the site.
		let broke = false;
		for (const width of [200, 210, 220, 230, 240, 250, 260, 270, 280, 288]) {
			const view = drawn(row({ title }), width);
			const text = view.title.firstChild!;
			const from = title.indexOf('合约交互');
			if (linesOf(text, from, from + 4) > 1) broke = true;
			view.done();
		}
		expect(broke).toBe(true);
		// …and at those same widths the parts never do.
		const [, titleParts] = zh(...HOSTS[0]);
		for (const width of [200, 210, 220, 230, 240, 250, 260, 270, 280, 288]) {
			const view = drawn(row({ title, titleParts }), width);
			expect(linesOf(view.title.querySelector('.action')!.firstChild!), String(width)).toBe(1);
			view.done();
		}
	});

	it('a row with no place to break before is one text, as before', () => {
		const view = drawn(row({ title: 'Contract interaction' }), 288);
		expect(view.title.querySelector('.action')).toBeNull();
		expect(view.title.textContent).toBe('Contract interaction');
		view.done();
	});
});
