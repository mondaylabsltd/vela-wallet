/**
 * Issue 466: "Report this" on both relay stops is a button the route answers
 * with the in-app report — no longer a bare link to an empty GitHub form.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { RelayerModel, RelayUnreachableModel } from '../model';
import RelayerBody from './RelayerBody.svelte';
import RelayUnreachableBody from './RelayUnreachableBody.svelte';

const MARK = { letter: 'E', color: 'var(--color-fg-subtle)' };

const RELAYER: RelayerModel = {
	title: 'Relayer out of gas',
	lead: 'Vela’s operator runs this network’s relayer, and it is out of gas.',
	mark: MARK,
	name: 'Ethereum',
	amountHint: 'About 0.0001 ETH',
	qrCaption: 'Treasury',
	addressDisplay: '0x3e59…fe3c',
	copyLabel: 'Copy',
	callout: { tone: 'warning', text: 'Non-refundable.' },
	primary: 'Retry',
	report: { label: 'Report this', selfFundLabel: 'Fund it yourself' }
};

const UNREACHABLE: RelayUnreachableModel = {
	title: 'Relay can’t reach this network',
	lead: 'Vela’s operator runs this network’s relay.',
	mark: MARK,
	name: 'Ethereum',
	report: { label: 'Report this' },
	primary: 'Retry'
};

describe('a relay stop’s Report this', () => {
	it('on the treasury sheet is a button that asks the route for the report', () => {
		const onreport = vi.fn();
		const screen = render(RelayerBody, { props: { panel: RELAYER, onreport } });
		expect(screen.container.querySelector('a.report')).toBeNull();
		const button = screen.container.querySelector<HTMLButtonElement>('button.report');
		expect(button?.textContent?.trim()).toBe('Report this');
		button!.click();
		expect(onreport).toHaveBeenCalledTimes(1);
	});

	it('on the can’t-reach sheet too', () => {
		const onreport = vi.fn();
		const screen = render(RelayUnreachableBody, { props: { panel: UNREACHABLE, onreport } });
		expect(screen.container.querySelector('a[href*="issues/new"]')).toBeNull();
		screen.container.querySelector<HTMLButtonElement>('button.report')!.click();
		expect(onreport).toHaveBeenCalledTimes(1);
	});

	it('is not drawn where there is nobody to tell', () => {
		const screen = render(RelayUnreachableBody, {
			props: { panel: { ...UNREACHABLE, report: undefined } }
		});
		expect(screen.container.querySelector('.report')).toBeNull();
	});
});
