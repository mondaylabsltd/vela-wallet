/**
 * The unreachable-networks list offers "Fix" only where there is something to
 * fix (PR 3 note 4), in a real browser.
 *
 * "Fix" opens a network's RPC editor. A network whose token list could not be
 * loaded (Tempo, its registry document away) has an RPC that works, so the
 * core gives its row no fix (`UnreachableNetwork.rpc_fixable`) and the model
 * carries no `action`. What a person sees is the point: no button on that
 * row, the button still on the row beside it, and the two rows the same
 * height — a list whose rows differ by a button's padding reads as broken.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { UnreachableModel } from '../model';
import UnreachableBody from './UnreachableBody.svelte';

const mark = (letter: string) => ({ letter, color: 'var(--color-fg-subtle)' });

const TEMPO = {
	id: '4217',
	chainId: 4217,
	mark: mark('T'),
	name: 'Tempo',
	line: 'Last seen $12.00'
};
const ETHEREUM = {
	id: '1',
	chainId: 1,
	mark: mark('E'),
	name: 'Ethereum',
	line: 'Last seen $4,500.00',
	action: 'Fix'
};

const panel = (rows: UnreachableModel['rows'], title: string): UnreachableModel => ({
	title,
	summary: "Your assets there aren't affected — we just can't read them right now.",
	rows
});

describe('the unreachable list — a row is offered "Fix" only when its RPC is what failed', () => {
	it('a token-list row draws no button at all', async () => {
		const onfix = vi.fn();
		const screen = render(UnreachableBody, {
			props: { panel: panel([TEMPO], "Can't load Tempo's token list right now"), onfix }
		});
		const rows = screen.container.querySelectorAll('[data-testid="unreachable-list"] li');
		expect(rows).toHaveLength(1);
		expect(rows[0].textContent).toContain('Tempo');
		expect(rows[0].textContent).toContain('Last seen $12.00');
		expect(screen.container.querySelectorAll('button')).toHaveLength(0);
		expect(screen.container.textContent).not.toContain('Fix');
		expect(onfix).not.toHaveBeenCalled();
	});

	it('beside a network that did not answer: that one keeps its Fix, and the rows are one height', async () => {
		const onfix = vi.fn();
		const screen = render(UnreachableBody, {
			props: { panel: panel([ETHEREUM, TEMPO], "Can't reach 2 networks right now"), onfix }
		});
		const rows = [
			...screen.container.querySelectorAll<HTMLElement>('[data-testid="unreachable-list"] li')
		];
		expect(rows).toHaveLength(2);
		expect(rows[0].querySelectorAll('button')).toHaveLength(1);
		expect(rows[1].querySelectorAll('button')).toHaveLength(0);
		// The button does not set the row's height: with it or without, the same.
		expect(rows[1].getBoundingClientRect().height).toBe(rows[0].getBoundingClientRect().height);

		rows[0].querySelector('button')!.click();
		expect(onfix).toHaveBeenCalledExactlyOnceWith(1);
	});
});
