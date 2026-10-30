/**
 * `/pay` — the payment link's front door (spec 039 Part B): to the locale's
 * pay page, the request untouched. The e2e drives it through the built worker.
 */
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

function hit(path: string, headers: Record<string, string> = {}): Response {
	return GET({ request: new Request(`https://wallet.getvela.app${path}`, { headers }) });
}

const LINK =
	'?to=0x52908400098527886E0F7030069857D2E4169EE7&chain=8453' +
	'&token=0x833589fcd6edb6e08f4c7c32d4f71b54bda02913&amount=1.5&sym=USDC&dec=6&net=Base';

describe('/pay', () => {
	it('goes to the negotiated locale’s pay page with the whole query', () => {
		const response = hit(`/pay${LINK}`, { 'accept-language': 'zh-CN,zh;q=0.9' });
		expect(response.status).toBe(307);
		expect(response.headers.get('location')).toBe(`/zh/pay${LINK}`);
		expect(response.headers.get('vary')).toBe('Accept-Language');
		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('falls back to English, and keeps even a query it would refuse', () => {
		// The page and the core rule on the link; the door never does.
		const response = hit('/pay?to=nope&amount=1e18');
		expect(response.headers.get('location')).toBe('/en/pay?to=nope&amount=1e18');
	});

	it('keeps percent-encoding exactly as sent', () => {
		const response = hit('/pay?to=0xabc&net=Base%20Mainnet&sym=%E2%82%AC', {
			'accept-language': 'de'
		});
		expect(response.headers.get('location')).toBe(
			'/de/pay?to=0xabc&net=Base%20Mainnet&sym=%E2%82%AC'
		);
	});
});
