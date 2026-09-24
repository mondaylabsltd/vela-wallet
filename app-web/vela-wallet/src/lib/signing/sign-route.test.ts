/**
 * The routing mirror, against the cases the core's own suite pins
 * (`rust/crates/vela-core/src/wallet_keys.rs`, `sign_route` tests).
 *
 * Every assertion below is one of those, transcribed: if the core's rule
 * changes and this file still passes, the mirror has drifted and the web will
 * open the wrong page for somebody's key.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { signRoute, type DeviceKey } from './sign-route';

const held = (credential_id: string, transports: string): DeviceKey => ({
	credential_id,
	transports
});
const behind = (credential_id: string, signer_origin: string): DeviceKey => ({
	credential_id,
	transports: '',
	signer_origin
});

describe('the passkey routes, as before', () => {
	const keys = [
		held('apple', 'hybrid,internal'),
		held('chrome', 'internal'),
		held('yubikey', 'nfc,usb')
	];

	it('pins the key of the chosen kind', () => {
		// The YubiKey, not the first key: a security key cannot answer for a
		// credential it does not hold.
		expect(signRoute(keys, 'security_key')).toMatchObject({
			credentialId: 'yubikey',
			transports: 'usb,nfc,ble'
		});
		expect(signRoute(keys, 'platform')).toMatchObject({ credentialId: 'apple' });
		// Nobody registered as hybrid-only: the first key, made reachable over a QR.
		expect(signRoute(keys, 'hybrid')).toMatchObject({
			credentialId: 'apple',
			transports: 'hybrid,internal'
		});
	});

	it('auto, an unknown name and a wallet with no credential change nothing', () => {
		expect(signRoute(keys, 'auto')).toBeNull();
		expect(signRoute(keys, 'telepathy')).toBeNull();
		expect(signRoute([held('', 'internal')], 'platform')).toBeNull();
		expect(signRoute([], 'trusted_signer')).toBeNull();
	});
});

describe('a key that lives behind a page', () => {
	it('is signed there by `auto`', () => {
		const route = signRoute(
			[behind('cs', 'https://sign.getvela.app'), held('apple', 'internal')],
			'auto'
		);
		expect(route).toEqual({
			credentialId: 'cs',
			transports: '',
			method: 'trusted_signer',
			signerOrigin: 'https://sign.getvela.app'
		});
	});

	it('is preferred when the Trusted Signer is chosen by name, wherever it stands', () => {
		expect(
			signRoute([held('apple', 'internal'), behind('cs', 'https://me.example')], 'trusted_signer')
		).toMatchObject({ credentialId: 'cs', signerOrigin: 'https://me.example' });
		// With none, the first key is pinned for the Settings page to reach.
		expect(signRoute([held('apple', 'internal')], 'trusted_signer')).toMatchObject({
			method: 'trusted_signer',
			signerOrigin: ''
		});
	});

	it('is reachable only through its page when the page is not ours', () => {
		expect(signRoute([behind('mine', 'https://me.example')], 'platform')).toMatchObject({
			method: 'trusted_signer',
			signerOrigin: 'https://me.example'
		});
		// With another key that a platform sheet CAN reach, that one is pinned.
		expect(
			signRoute([behind('mine', 'https://me.example'), held('yubikey', 'usb,nfc')], 'platform')
		).toMatchObject({ method: 'platform', credentialId: 'yubikey' });
		// A key made on the official page is a getvela.app passkey: still ours.
		expect(signRoute([behind('cs', 'https://sign.getvela.app')], 'platform')).toMatchObject({
			method: 'platform',
			credentialId: 'cs'
		});
	});
});
