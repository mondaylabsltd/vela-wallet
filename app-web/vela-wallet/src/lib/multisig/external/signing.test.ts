/**
 * The one entry for asking an outside owner's wallet: the request is shown
 * (`externalSigning.request`, the sheet's) until the session machine says how
 * it ended — then it is gone, so the sheet goes with it.
 */
import { describe, expect, it } from 'vitest';
import { externalSigning, openOutsideSession } from './signing.svelte';

const ASK = {
	chainId: 100,
	safe: '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83',
	tx: {},
	safeTxHash: `0x${'ab'.repeat(32)}`,
	owners: ['0x70997970c51812dc3a010c7d01b50e0d17dc79c8'],
	send: null
};

describe('asking an outside wallet', () => {
	it('settles with how the ask ended, and the sheet’s request is gone', async () => {
		const asked = openOutsideSession(ASK);
		const shown = externalSigning.request!;
		expect(shown.safeTxHash).toBe(ASK.safeTxHash);
		const approval = { owner: ASK.owners[0], signature: '0x01' };
		shown.settle({ kind: 'approved', approval });
		await expect(asked).resolves.toEqual({ kind: 'approved', approval });
		expect(externalSigning.request).toBeNull();
	});

	it('an approval asked for: the approval, or an AbortError once closed', async () => {
		const approved = externalSigning.approve(ASK);
		externalSigning.request!.settle({ kind: 'approved', approval: { owner: 'x' } });
		await expect(approved).resolves.toEqual({ owner: 'x' });
		const closed = externalSigning.approve(ASK);
		externalSigning.request!.settle({ kind: 'cancelled' });
		await expect(closed).rejects.toMatchObject({ name: 'AbortError' });
		expect(externalSigning.request).toBeNull();
	});

	it('another ask supersedes the one shown: that one is cancelled', async () => {
		const first = openOutsideSession(ASK);
		const second = openOutsideSession({ ...ASK, safeTxHash: `0x${'cd'.repeat(32)}` });
		await expect(first).resolves.toEqual({ kind: 'cancelled' });
		expect(externalSigning.request!.safeTxHash).toBe(`0x${'cd'.repeat(32)}`);
		externalSigning.request!.settle({ kind: 'cancelled' });
		await second;
	});
});
