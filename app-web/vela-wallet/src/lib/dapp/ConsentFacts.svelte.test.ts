/**
 * Spec 096 F11: the request window's connect consent names the account and the
 * network a Connect shares, as every in-app browser's consent does.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import ConsentFacts from './ConsentFacts.svelte';

const ADDRESS = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';

describe('what a Connect shares', () => {
	it('the account by name and short address, then the network', async () => {
		const screen = render(ConsentFacts, {
			props: {
				accountLabel: 'Account',
				networkLabel: 'Network',
				account: {
					name: 'Parallel Multi',
					address: ADDRESS,
					short: '0x88cCA0…266894',
					identiconSvg: '<svg></svg>'
				},
				network: { name: 'BNB Chain' }
			}
		});
		await tick();
		const rows = [...document.body.querySelectorAll('[data-testid="consent-facts"] .row')];
		expect(rows.map((row) => row.querySelector('dt')?.textContent)).toEqual(['Account', 'Network']);
		expect(rows[0].textContent).toContain('Parallel Multi');
		expect(rows[0].textContent).toContain('0x88cCA0…266894');
		expect(rows[1].textContent).toContain('BNB Chain');
		await screen.unmount();
	});

	it('with nobody signed in, only the network', async () => {
		const screen = render(ConsentFacts, {
			props: {
				accountLabel: 'Account',
				networkLabel: 'Network',
				account: null,
				network: { name: 'Gnosis' }
			}
		});
		await tick();
		const rows = document.body.querySelectorAll('[data-testid="consent-facts"] .row');
		expect(rows).toHaveLength(1);
		expect(rows[0].textContent).toContain('Gnosis');
		await screen.unmount();
	});
});
