/**
 * The wait (issue 199). The submitted receipt said "6s elapsed — almost
 * there" six seconds into a fifteen-second wait and then sat still; what it
 * says, and how far round the ring has gone, is a function of one number —
 * seconds since the relay took the op — so that is what these pin.
 */
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import SendReceipt from './SendReceipt.svelte';
import type { SendReceiptModel } from '../model';

function submitted(elapsedS: number): SendReceiptModel {
	return {
		header: { title: 'Send XDAI', backLabel: 'Back' },
		stage: 'submitted',
		title: 'Submitted to the network',
		captions: ['Waiting for blockchain confirmation...'],
		eta: {
			submittedAtMs: Date.now() - elapsedS * 1000 - 500,
			typicalS: 15,
			typicalLine: 'Gnosis typically confirms in ~15s',
			remainingTemplate: '~{{remaining}}s remaining',
			elapsedTemplate: '{{elapsed}}s elapsed — almost there',
			slowLine: 'Taking longer than usual, please wait...'
		},
		cta: 'Close · keep running',
		ctaAccent: false
	} as SendReceiptModel;
}

const lastCaption = (el: Element) => [...el.querySelectorAll('.caption')].at(-1)?.textContent;
const drawn = (el: Element) =>
	1 - Number(el.querySelector('.arc')?.getAttribute('stroke-dashoffset'));

describe('SendReceipt — the wait', () => {
	it('counts down inside the typical time', () => {
		const { container } = render(SendReceipt, { props: { model: submitted(6) } });
		expect(lastCaption(container)).toBe('~9s remaining');
	});

	it('says "almost there" only once the typical time has passed', () => {
		const { container } = render(SendReceipt, { props: { model: submitted(20) } });
		expect(lastCaption(container)).toBe('20s elapsed — almost there');
	});

	it('admits it is slow past twice the typical time', () => {
		const { container } = render(SendReceipt, { props: { model: submitted(31) } });
		expect(lastCaption(container)).toBe('Taking longer than usual, please wait...');
	});

	it('moves the ring for minutes without ever closing it', () => {
		const at = (s: number) =>
			drawn(render(SendReceipt, { props: { model: submitted(s) } }).container);
		const [early, typical, minutes] = [at(3), at(15), at(300)];
		expect(early).toBeGreaterThan(0.1);
		expect(typical).toBeGreaterThan(early);
		expect(minutes).toBeGreaterThan(typical);
		expect(minutes).toBeLessThan(0.95);
	});

	it('closes the ring on confirmation', () => {
		const model = { ...submitted(10), stage: 'confirmed', eta: undefined } as SendReceiptModel;
		expect(drawn(render(SendReceipt, { props: { model } }).container)).toBe(1);
	});
});
