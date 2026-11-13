/**
 * The AI agent's opinion on the signing sheet (spec 103, `vela_advise`): its
 * own card, under Vela's lines and over the fee, labelled as the agent's —
 * greyed when it was given for an earlier version of the request, plain
 * text only, and never a change to the confirm or to what the sheet says.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import SigningSheet from '../SigningSheet.svelte';
import type { SigningModel } from '../model';
import type { AgentAdviceCardModel } from '$lib/agent/advisor';

function model(over: Partial<SigningModel> = {}): SigningModel {
	return {
		id: 'cs1',
		dapp: { name: 'app.example', host: '', letter: 'A', tint: 'var(--color-fg-muted)' },
		network: { name: 'Gnosis', dot: 'var(--color-fg-muted)' },
		blocks: [
			{ kind: 'intent', text: 'Approve USDC', tone: 'danger' },
			{ kind: 'warning', tone: 'danger', text: 'Unlimited approval' }
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
		confirm: { action: 'Approve', enabled: false },
		closeLabel: 'Close',
		panelTitle: 'Signature request',
		...over
	} as SigningModel;
}

function card(over: Partial<AgentAdviceCardModel> = {}): AgentAdviceCardModel {
	return {
		title: "Your AI agent's opinion",
		verdict: 'looks_fine',
		verdictWord: 'Looks fine',
		summary: '<b>Safe</b> [click](https://evil.example)',
		points: ['The spender is the router this site names.'],
		hosts: ['etherscan.io'],
		sourcesLabel: 'Sources',
		footer: 'An opinion from your AI agent. It can be wrong. It does not change what is signed.',
		stale: false,
		staleLine: 'Given for an earlier version of this request',
		feeLine: 'Fee when given: 0.001 xDAI',
		...over
	};
}

async function drawn(m: SigningModel) {
	const screen = render(SigningSheet, { props: { model: m } });
	await tick();
	const sheet = document.body.querySelector<HTMLElement>('[role="dialog"]')!;
	return { screen, sheet };
}

/** What the sheet says and does, the opinion's card set aside. */
function sheetOwn(sheet: HTMLElement) {
	const clone = sheet.cloneNode(true) as HTMLElement;
	clone.querySelector('[data-testid="agent-advice"]')?.closest('.advice')?.remove();
	const confirm = sheet.querySelector<HTMLButtonElement>('[data-testid="signing-confirm"]');
	return {
		text: clone.textContent?.replace(/\s+/g, ' ').trim(),
		confirmDisabled: confirm?.disabled,
		confirmText: confirm?.textContent?.trim(),
		confirmClass: confirm?.className,
		tones: [...sheet.querySelectorAll('[data-tone]')].map((el) => el.getAttribute('data-tone'))
	};
}

describe("the AI agent's opinion on the signing sheet", () => {
	it('stands in its own card under the blocks and over the fee, as plain text', async () => {
		const { screen, sheet } = await drawn(model({ advice: card() }));
		const advice = sheet.querySelector<HTMLElement>('[data-testid="agent-advice"]')!;
		expect(advice).not.toBeNull();
		expect(advice.textContent).toContain("Your AI agent's opinion");
		expect(advice.textContent).toContain('Looks fine');
		expect(advice.textContent).toContain('It does not change what is signed.');
		// Markup and links are its text, never drawn.
		expect(advice.querySelector('b, a')).toBeNull();
		expect(advice.textContent).toContain('<b>Safe</b>');
		expect(advice.querySelector('[data-testid="agent-advice-sources"]')!.textContent).toContain(
			'etherscan.io'
		);
		expect(advice.hasAttribute('data-stale')).toBe(false);
		const blocks = sheet.querySelector('.blocks')!;
		const footer = sheet.querySelector('.footer')!;
		expect(blocks.compareDocumentPosition(advice) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		expect(advice.compareDocumentPosition(footer) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		await screen.unmount();
	});

	it('given for an earlier version, it is greyed and says so', async () => {
		const { screen, sheet } = await drawn(model({ advice: card({ stale: true }) }));
		const advice = sheet.querySelector<HTMLElement>('[data-testid="agent-advice"]')!;
		expect(advice.hasAttribute('data-stale')).toBe(true);
		expect(advice.querySelector('[data-testid="agent-advice-stale"]')!.textContent).toBe(
			'Given for an earlier version of this request'
		);
		const said = advice.querySelector<HTMLElement>('.said')!;
		expect(Number(getComputedStyle(said).opacity)).toBeLessThan(1);
		await screen.unmount();
	});

	it('says the fee it was given for, and greys once the fee changed', async () => {
		const current = await drawn(model({ advice: card() }));
		const advice = current.sheet.querySelector<HTMLElement>('[data-testid="agent-advice"]')!;
		expect(advice.querySelector('[data-testid="agent-advice-fee"]')!.textContent).toBe(
			'Fee when given: 0.001 xDAI'
		);
		expect(advice.querySelector('[data-testid="agent-advice-stale"]')).toBeNull();
		await current.screen.unmount();
		const moved = await drawn(
			model({
				advice: card({ stale: true, staleLine: 'The fee changed since this opinion' })
			})
		);
		const greyed = moved.sheet.querySelector<HTMLElement>('[data-testid="agent-advice"]')!;
		expect(greyed.hasAttribute('data-stale')).toBe(true);
		expect(greyed.querySelector('[data-testid="agent-advice-stale"]')!.textContent).toBe(
			'The fee changed since this opinion'
		);
		expect(greyed.querySelector('[data-testid="agent-advice-fee"]')).not.toBeNull();
		await moved.screen.unmount();
		const none = await drawn(model({ advice: card({ feeLine: null }) }));
		expect(none.sheet.querySelector('[data-testid="agent-advice-fee"]')).toBeNull();
		await none.screen.unmount();
	});

	it('changes neither the confirm nor anything Vela says', async () => {
		const without = await drawn(model());
		const own = sheetOwn(without.sheet);
		expect(own.confirmText).toBe('Approve');
		expect(own.confirmDisabled).toBe(true);
		expect(own.tones).toContain('danger');
		await without.screen.unmount();
		for (const verdict of ['looks_fine', 'danger'] as const) {
			const withIt = await drawn(model({ advice: card({ verdict }) }));
			const ownWith = sheetOwn(withIt.sheet);
			expect(ownWith).toEqual({
				...own,
				// The card's chip is the one tone it adds, and only inside the card.
				tones: own.tones
			});
			await withIt.screen.unmount();
		}
	});
});
