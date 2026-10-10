/**
 * The send confirm in a browser (PR 2 polish).
 *
 * - A submit the relay turned back because the account's previous
 *   transaction still holds the nonce is "Not sent yet": its title over the
 *   sentence, said politely and in the quiet ink — never the error's red —
 *   with Try again.
 * - The failed fee line is the control its words and the footer promise:
 *   "Tap to retry" over "Couldn't work out the fee. Tap it to retry" asks
 *   again; "Pay with another coin" opens the coins; a dash with nothing
 *   behind it is a fact. The web's line used to read "—", unpressable, under
 *   a footer asking for the tap.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import SendConfirm from './SendConfirm.svelte';
import type { FactRowModel, SendConfirmModel } from '../model';

const FEE_LABEL = 'Est. Fee';

function model(over: Partial<SendConfirmModel> = {}, fee: Partial<FactRowModel> = {}) {
	return {
		header: { title: 'Send ETH', backLabel: 'Back' },
		amount: '0.1',
		amountUnit: 'ETH',
		subline: '≈ $300.00',
		facts: [
			{ label: 'From', value: 'My Wallet' },
			{ label: 'Network', value: 'Ethereum' },
			{ label: FEE_LABEL, value: '0.00021 ETH', ...fee }
		],
		cta: 'Confirm send',
		...over
	} satisfies SendConfirmModel;
}

const colourOf = (variable: string): string => {
	const probe = document.createElement('span');
	probe.style.color = `var(${variable})`;
	document.body.appendChild(probe);
	const colour = getComputedStyle(probe).color;
	probe.remove();
	return colour;
};

async function drawn(m: SendConfirmModel) {
	const calls: string[] = [];
	const screen = render(SendConfirm, {
		props: {
			model: m,
			ctaDisabled: true,
			onretry: () => calls.push('retry'),
			onfeeretry: () => calls.push('fee-retry'),
			onfeecoins: () => calls.push('fee-coins')
		}
	});
	await tick();
	return { screen, calls };
}

describe('SendConfirm — not sent yet (PR 2 polish)', () => {
	it('is its own calm notice: the title over the sentence, politely, never in red — with Try again', async () => {
		const { screen, calls } = await drawn(
			model({
				error: {
					title: 'Not sent yet',
					text: 'Your previous transaction on this network is still being processed. Try again once it’s done.',
					retry: 'Try Again',
					calm: true
				}
			})
		);
		const notice = screen.container.querySelector(
			'[data-testid="send-confirm-error"]'
		) as HTMLElement;
		expect(notice.getAttribute('role')).toBe('status');
		const [title, body] = [...notice.querySelectorAll('p')];
		expect(title.textContent).toBe('Not sent yet');
		expect(body.textContent).toContain('still being processed');
		const red = colourOf('--color-error-base');
		expect(getComputedStyle(title).color).not.toBe(red);
		expect(getComputedStyle(body).color).not.toBe(red);
		expect(getComputedStyle(body).color).toBe(colourOf('--color-fg-muted'));
		(notice.querySelector('button') as HTMLButtonElement).click();
		await tick();
		expect(calls).toEqual(['retry']);
	});

	it('every other failed submit keeps its alert, in the error colour', async () => {
		const { screen } = await drawn(
			model({ error: { text: 'Something went wrong. Your funds are safe.', retry: 'Try Again' } })
		);
		const notice = screen.container.querySelector(
			'[data-testid="send-confirm-error"]'
		) as HTMLElement;
		expect(notice.getAttribute('role')).toBe('alert');
		expect(getComputedStyle(notice.querySelector('p') as HTMLElement).color).toBe(
			colourOf('--color-error-base')
		);
	});
});

describe('SendConfirm — the failed fee line does what it says (PR 2 polish)', () => {
	const feeLine = (screen: ReturnType<typeof render>) =>
		[...screen.container.querySelectorAll<HTMLElement>('.fact')].find((row) =>
			row.textContent?.includes(FEE_LABEL)
		) as HTMLElement;

	it('"Tap to retry" is a control, and asks the fee again', async () => {
		const { screen, calls } = await drawn(
			model(
				{ held: 'Couldn’t work out the fee. Tap it to retry' },
				{ value: 'Tap to retry', tap: { does: 'retry', label: 'Refresh fee', busy: false } }
			)
		);
		const line = feeLine(screen);
		expect(line.tagName.toLowerCase()).toBe('button');
		expect(line.getAttribute('aria-label')).toBe('Refresh fee');
		expect(line.textContent).toContain('Tap to retry');
		line.click();
		await tick();
		expect(calls).toEqual(['fee-retry']);
	});

	it('while a re-ask is out the line stays the same control, and asks nothing', async () => {
		const { screen, calls } = await drawn(
			model({}, { value: '—', tap: { does: 'retry', label: 'Refresh fee', busy: true } })
		);
		const line = feeLine(screen);
		expect(line.tagName.toLowerCase()).toBe('button');
		expect(line.getAttribute('aria-busy')).toBe('true');
		line.click();
		await tick();
		expect(calls).toEqual([]);
	});

	it('"Pay with another coin" opens the fee coins, behind its chevron', async () => {
		const { screen, calls } = await drawn(
			model(
				{ held: 'This would fail if sent as it is.' },
				{
					value: 'Pay with another coin',
					tap: { does: 'choose_coin', label: 'Fee token', busy: false }
				}
			)
		);
		const line = feeLine(screen);
		expect(line.getAttribute('aria-label')).toBe('Fee token');
		expect(line.querySelector('.chevron svg')).not.toBeNull();
		line.click();
		await tick();
		expect(calls).toEqual(['fee-coins']);
	});

	it('a dash with nothing behind it — and every settled fee — is a fact, never pressable', async () => {
		for (const fee of [{ value: '—' }, {}]) {
			const { screen, calls } = await drawn(model({}, fee));
			const line = feeLine(screen);
			expect(line.tagName.toLowerCase()).toBe('div');
			expect(screen.container.querySelectorAll('.facts button')).toHaveLength(0);
			line.click();
			await tick();
			expect(calls).toEqual([]);
			screen.unmount();
		}
	});
});
