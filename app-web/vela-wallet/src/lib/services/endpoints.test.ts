/**
 * set-8: a stored service endpoint is used only when it is an address the
 * wallet may use. Settings now refuses the rest when they are typed; these
 * readers refuse the ones already in storage.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const stored: { record: Record<string, string> } = { record: {} };
vi.mock('$lib/onboarding/core/storage', () => ({
	loadServiceEndpoints: vi.fn(() => stored.record)
}));

import {
	DEFAULT_SERVICE_ENDPOINTS,
	getBundlerServiceURL,
	getEthereumDataURL,
	getFiatRatesURL,
	getPasskeyIndexURL
} from './endpoints';

beforeEach(() => {
	stored.record = {};
});

describe('service endpoint readers', () => {
	it('use what the person set when it is an https address', () => {
		stored.record = {
			ethereumDataURL: 'https://data.example',
			passkeyIndexURL: 'http://localhost:8787',
			bundlerServiceURL: 'https://relay.example/',
			fiatRatesURL: 'https://fx.example/rates?base=USD'
		};
		expect(getEthereumDataURL()).toBe('https://data.example');
		expect(getPasskeyIndexURL()).toBe('http://localhost:8787');
		expect(getBundlerServiceURL()).toBe('https://relay.example/');
		expect(getFiatRatesURL()).toBe('https://fx.example/rates?base=USD');
	});

	it('fall back to the default for an address the URL rule refuses', () => {
		stored.record = {
			ethereumDataURL: 'javascript:alert(1)',
			passkeyIndexURL: 'http://index.example',
			bundlerServiceURL: 'notaurl',
			fiatRatesURL: 'ftp://fx.example'
		};
		expect(getEthereumDataURL()).toBe(DEFAULT_SERVICE_ENDPOINTS.ethereumDataURL);
		expect(getPasskeyIndexURL()).toBe(DEFAULT_SERVICE_ENDPOINTS.passkeyIndexURL);
		expect(getBundlerServiceURL()).toBe(DEFAULT_SERVICE_ENDPOINTS.bundlerServiceURL);
		expect(getFiatRatesURL()).toBe(DEFAULT_SERVICE_ENDPOINTS.fiatRatesURL);
	});

	it('read blank as the default', () => {
		stored.record = { ethereumDataURL: '  ' };
		expect(getEthereumDataURL()).toBe(DEFAULT_SERVICE_ENDPOINTS.ethereumDataURL);
	});
});
