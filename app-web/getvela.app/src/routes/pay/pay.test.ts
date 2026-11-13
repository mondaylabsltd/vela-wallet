/**
 * getvela.app/pay forwards to the web wallet's `/pay` with the link intact
 * (spec 039 Part B) — no longer to the wallet's front door, which dropped
 * the request.
 */
import { isRedirect } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

function forward(path: string): { status: number; location: string } {
	try {
		GET({ url: new URL(`https://getvela.app${path}`) } as Parameters<typeof GET>[0]);
	} catch (thrown) {
		if (isRedirect(thrown)) return { status: thrown.status, location: thrown.location };
		throw thrown;
	}
	throw new Error('no redirect');
}

describe('getvela.app/pay', () => {
	it('forwards to the wallet’s pay page with the whole query', () => {
		const link =
			'?to=0x52908400098527886E0F7030069857D2E4169EE7&chain=8453' +
			'&token=0x833589fcd6edb6e08f4c7c32d4f71b54bda02913&amount=1.5&sym=USDC&dec=6&net=Base%20Mainnet';
		expect(forward(`/pay${link}`)).toEqual({
			status: 307,
			location: `https://wallet.getvela.app/pay${link}`
		});
	});

	it('forwards a bare /pay as a bare /pay — the wallet says what is missing', () => {
		expect(forward('/pay').location).toBe('https://wallet.getvela.app/pay');
	});
});
