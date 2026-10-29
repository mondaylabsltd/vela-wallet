/**
 * §3.5: whether this device signs for a multisig on a network. Read-only
 * where its genesis is refused; where more than its members can act for it,
 * each signature asks — naming what was found — every time; where who can
 * act for it could not be read, nothing is signed yet. The network's check
 * is stood in for (`recheck.test.ts` drives it).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

const seams = vi.hoisted(() => ({
	/** What the network's check found; `null`: it did not answer. */
	found: [] as unknown[] | null,
	checks: 0
}));
vi.mock('./recheck', () => ({
	checkOn: async () => {
		seams.checks += 1;
		return seams.found;
	},
	findingsOn: () => seams.found ?? []
}));

const { authorizeSigning, gateOf, riskConfirmation, SigningRefusedError, SigningUncheckedError } =
	await import('./gate.svelte');

const account = {
	address: SAFE,
	genesis: { factory: '', singleton: '', saltNonce: '0x1', initializer: '0x' },
	owners: [],
	threshold: 1,
	recorded: true,
	chainIds: [100],
	addedMs: 0
};
const risk = { from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } } as const;
const refusal = {
	from: 'genesis',
	chainId: 100,
	reason: { kind: 'module', address: MODULE }
} as const;

let detach: (() => void) | null = null;
beforeEach(() => {
	detach?.();
	detach = riskConfirmation.attach();
	seams.found = [];
	seams.checks = 0;
});

/** Answers the next confirmation asked, once it is up. */
async function answerNext(signs: boolean) {
	await vi.waitFor(() => expect(riskConfirmation.request).not.toBeNull());
	const asked = riskConfirmation.request!;
	asked.answer(signs);
	return asked;
}

describe('the gate a network makes (§3.5)', () => {
	it('is read per network: a refusal comes before a risk; another network says nothing', () => {
		expect(gateOf([risk], 100)).toEqual({ kind: 'risk', chainId: 100, findings: [risk] });
		expect(gateOf([risk, refusal], 100)).toEqual({
			kind: 'refused',
			chainId: 100,
			findings: [refusal]
		});
		expect(gateOf([risk, refusal], 8453)).toEqual({ kind: 'open' });
	});
});

describe('before any signature (§3.5)', () => {
	it('only its members act there: signs, asking nothing', async () => {
		await authorizeSigning(account, 100);
		expect(riskConfirmation.request).toBeNull();
		expect(seams.checks).toBe(1);
	});

	it('its genesis refused there: never signs, and asks nothing', async () => {
		seams.found = [refusal];
		const refused = await authorizeSigning(account, 100).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(SigningRefusedError);
		expect((refused as InstanceType<typeof SigningRefusedError>).findings).toEqual([refusal]);
		expect(riskConfirmation.request).toBeNull();
	});

	it('a risk there: asks, naming what was found — and again for the next signature', async () => {
		seams.found = [risk];
		const first = authorizeSigning(account, 100);
		const asked = await answerNext(true);
		expect(asked).toMatchObject({ chainId: 100, safe: SAFE, findings: [risk] });
		await expect(first).resolves.toBeUndefined();
		// Answered: the confirmation is gone (nothing left on screen).
		expect(riskConfirmation.request).toBeNull();
		// Nothing remembered: the next signature asks again.
		const second = authorizeSigning(account, 100);
		await answerNext(true);
		await expect(second).resolves.toBeUndefined();
	});

	it('the risk declined: a cancel (AbortError), nothing signed', async () => {
		seams.found = [risk];
		const declined = authorizeSigning(account, 100);
		await answerNext(false);
		await expect(declined).rejects.toMatchObject({ name: 'AbortError' });
		expect(riskConfirmation.request).toBeNull();
	});

	it('no screen up to ask: declined, never signed unasked', async () => {
		seams.found = [risk];
		detach?.();
		detach = null;
		await expect(authorizeSigning(account, 100)).rejects.toMatchObject({ name: 'AbortError' });
	});

	it('the screen goes while it asks: declined', async () => {
		seams.found = [risk];
		const asking = authorizeSigning(account, 100);
		await vi.waitFor(() => expect(riskConfirmation.request).not.toBeNull());
		detach?.();
		detach = null;
		await expect(asking).rejects.toMatchObject({ name: 'AbortError' });
	});

	it('who can act for it there could not be read: nothing signed yet', async () => {
		seams.found = null;
		await expect(authorizeSigning(account, 100)).rejects.toBeInstanceOf(SigningUncheckedError);
	});
});
