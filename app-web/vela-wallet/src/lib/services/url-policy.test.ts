/**
 * The draw-time guard says what the core's URL rule says
 * (`network_admin::is_allowed_endpoint_url`). The two lists below are the
 * same lists `app_network_admin.rs` pins
 * (`the_url_rule_allows_https_and_loopback_http_only`); a case added to one
 * belongs in the other.
 */
import { describe, expect, it } from 'vitest';
import { allowedOrNull, isAllowedEndpointURL } from './url-policy';

const ALLOWED = [
	'https://rpc.example',
	'https://rpc.example/v2/key?x=1#y',
	'https://rpc.example:8545',
	'https://user:pass@node.example/rpc',
	'HTTPS://Rpc.Example',
	'https://[2001:db8::1]:443/',
	'https://例え.jp',
	'http://localhost',
	'http://localhost:8545/rpc',
	'http://127.0.0.1:3000',
	'http://[::1]:8545'
];

const REFUSED = [
	'',
	'javascript:alert(1)',
	'JavaScript:alert(1)',
	'data:text/html,<script>1</script>',
	'ftp://rpc.example',
	'ws://rpc.example',
	'wss://rpc.example',
	'notaurl',
	'rpc.example',
	'https://',
	'https:///path',
	'https://.example',
	'https://rpc.example:',
	'https://rpc.example:abc',
	'https://rpc example',
	'https://[::1]x',
	'https://rpc.example/\r\nX-Injected: 1',
	'http://rpc.example',
	'http://127.0.0.1.evil.com',
	'http://localhost.evil.com',
	'http://localhost:abc',
	'http://localhost?x'
];

describe('isAllowedEndpointURL', () => {
	it.each(ALLOWED)('allows %s', (url) => expect(isAllowedEndpointURL(url)).toBe(true));
	it.each(REFUSED)('refuses %j', (url) => expect(isAllowedEndpointURL(url)).toBe(false));
});

describe('allowedOrNull', () => {
	it('trims, and answers null for what the rule refuses or nothing at all', () => {
		expect(allowedOrNull('  https://rpc.example ')).toBe('https://rpc.example');
		expect(allowedOrNull('javascript:alert(1)')).toBeNull();
		expect(allowedOrNull('')).toBeNull();
		expect(allowedOrNull(undefined)).toBeNull();
		expect(allowedOrNull(null)).toBeNull();
	});
});
