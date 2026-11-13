/**
 * Rules the web shares with the native shells, read through the shipped wasm:
 * the relay's spent nonce is the core's reading (send-15), and the signing
 * prompt's deadline is one clock (send-11).
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import * as wasm from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { userOpSubmitStep } from './kernels';
import { classifySign, SIGN_TIMEOUT_MS } from '$lib/onboarding/core/passkey';

beforeAll(() => {
	wasm.initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

const LOCAL = '0x' + '55'.repeat(32);

describe('a spent nonce (send-15)', () => {
	it('is the core’s own rejection, in the bundler’s words or a node’s', () => {
		for (const message of ['AA25 invalid account nonce', 'nonce too low']) {
			expect(userOpSubmitStep({ error: { code: -32500, message } }, 0, false, LOCAL)).toEqual({
				done: { type: 'not_sent', rejection: 'nonce_used' }
			});
		}
	});

	it('after a reply that may have been lost, may have been sent', () => {
		expect(
			userOpSubmitStep(
				{ error: { code: -32500, message: 'AA25 invalid account nonce' } },
				1,
				true,
				LOCAL
			)
		).toEqual({ done: { type: 'maybe_sent', user_op_hash: LOCAL } });
	});
});

describe('the signing prompt’s deadline (send-11)', () => {
	const refusal = Object.assign(new Error('x'), { name: 'NotAllowedError' });

	it('is the core’s, and the web reads it the way every shell does', () => {
		expect(wasm.passkeySignTimeoutMs()).toBe(SIGN_TIMEOUT_MS);
		for (const elapsed of [0, 3_000, 117_999, 118_000, 120_000, 130_000]) {
			const web = classifySign(refusal, 0, elapsed).timedOut;
			expect(wasm.passkeyCeremonyEnd('cancelled', elapsed) === 'timed_out', `${elapsed}`).toBe(web);
		}
		expect(wasm.passkeyCeremonyEnd('not_supported', 130_000)).toBe('not_supported');
	});
});
