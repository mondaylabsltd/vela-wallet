/**
 * 087 F11: a dApp call that moved no coin of ours has no figure. Its row drew
 * an empty amount cell anyway (a lone " " on Android, a gap here); it draws
 * none, so the site name has the row.
 */
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ActivityRowModel } from '../model';
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

	it('keeps the amount cell for a row with one', () => {
		const screen = render(ActivityRow, {
			props: { row: row({ kind: 'sent', amount: '−0.01', unit: 'xDAI' }) }
		});
		expect(screen.container.querySelector('.amount')?.textContent).toContain('−0.01');
	});
});
