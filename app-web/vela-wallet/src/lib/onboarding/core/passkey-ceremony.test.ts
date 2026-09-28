/**
 * The signing ceremony tells its observers when it is up and how it ended
 * (spec 079): the signing sheet's ✕ may close an approved request only once
 * the signature exists, and the core's view cannot say when that is.
 * Observers only — a ceremony's result is exactly what it was before.
 */
import { afterEach, describe, expect, it } from 'vitest';
import {
	onSignCeremony,
	setPasskeyOverride,
	sign,
	signWithAny,
	type Assertion,
	type SignCeremonyEvent
} from './passkey';

const ASSERTION: Assertion = {
	credentialId: 'aa',
	signatureHex: 'bb',
	authenticatorDataHex: 'cc',
	clientDataJSONHex: 'dd',
	authenticatorAttachment: ''
};

afterEach(() => setPasskeyOverride(null));

function listen(): { events: SignCeremonyEvent[]; stop: () => void } {
	const events: SignCeremonyEvent[] = [];
	const stop = onSignCeremony((event) => events.push(event));
	return { events, stop };
}

describe('onSignCeremony', () => {
	it('a signature: started, then signed — and the assertion is untouched', async () => {
		setPasskeyOverride({ sign: async () => ASSERTION });
		const { events, stop } = listen();
		await expect(sign('00', 'aa')).resolves.toBe(ASSERTION);
		await expect(signWithAny('00', [{ id: 'aa' }])).resolves.toBe(ASSERTION);
		expect(events).toEqual(['started', 'signed', 'started', 'signed']);
		stop();
	});

	it('a cancelled or failed prompt: started, then failed — and the error still reaches the caller', async () => {
		const cancelled = new Error('User cancelled the operation');
		setPasskeyOverride({
			sign: async () => {
				throw cancelled;
			}
		});
		const { events, stop } = listen();
		await expect(sign('00', 'aa')).rejects.toBe(cancelled);
		expect(events).toEqual(['started', 'failed']);
		stop();
	});

	it('an observer that throws never touches the ceremony, and a stopped one hears nothing', async () => {
		setPasskeyOverride({ sign: async () => ASSERTION });
		const stopBad = onSignCeremony(() => {
			throw new Error('observer fault');
		});
		const { events, stop } = listen();
		stop();
		await expect(sign('00', 'aa')).resolves.toBe(ASSERTION);
		expect(events).toEqual([]);
		stopBad();
	});
});
