/**
 * §3.5, §3.6: whether this device signs for a multisig on a network — the
 * core's rule (`multisig_review::signing_gate`), read right before every
 * signature. Read-only where its genesis is refused; no signature at all for
 * a transaction whose execution changes who controls it undeclared; where
 * more than its members can act for it, each signature asks — naming what
 * was found — every time; where who can act for it could not be read,
 * nothing is signed yet. The chain is read again before EACH signature,
 * never this session's verdict: a module added mid-session is caught before
 * the next passkey. The chain's readings are stood in for.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

const seams = vi.hoisted(() => ({
	/** What the network's check found; `null`: it did not answer. */
	found: [] as unknown[] | null,
	checks: 0,
	/** The review's rule for the transaction, as the chain stands now; `null`: unread. */
	sign: { allowed: true, anyway: false } as { allowed: boolean; anyway: boolean } | null,
	reviews: 0
}));
vi.mock('./recheck', () => ({
	checkNow: async () => {
		seams.checks += 1;
		return seams.found;
	},
	findingsOn: () => seams.found ?? []
}));
vi.mock('./chain', () => ({
	readMembers: async () => ({ deployed: true, owners: [], threshold: 1, nonce: 0n })
}));
vi.mock('./proposal-review.svelte', () => ({
	gatherReview: async () => {
		seams.reviews += 1;
		if (seams.sign === null) throw new Error('the chain did not answer');
		return {};
	},
	reviewOf: () => ({ sign: seams.sign, state: { source: 'trace', verdict: { kind: 'declared' } } })
}));

const {
	authorizeSigning,
	gateOf,
	riskConfirmation,
	SigningBlockedError,
	SigningRefusedError,
	SigningUncheckedError
} = await import('./gate.svelte');

const account = {
	address: SAFE,
	genesis: { factory: '', singleton: '', saltNonce: '0x1', initializer: '0x' },
	owners: [],
	threshold: 1,
	recorded: true,
	chainIds: [100],
	addedMs: 0
};
const tx = {
	to: SAFE,
	value: '0x0',
	data: '0x',
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: '0x0000000000000000000000000000000000000000',
	refundReceiver: '0x0000000000000000000000000000000000000000',
	nonce: '0x0'
};
const risk = { from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } } as const;
const refusal = {
	from: 'genesis',
	chainId: 100,
	reason: { kind: 'module', address: MODULE }
} as const;

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

let detach: (() => void) | null = null;
beforeEach(() => {
	detach?.();
	detach = riskConfirmation.attach();
	seams.found = [];
	seams.checks = 0;
	seams.sign = { allowed: true, anyway: false };
	seams.reviews = 0;
});

/** Answers the next confirmation asked, once it is up. */
async function answerNext(signs: boolean) {
	await vi.waitFor(() => expect(riskConfirmation.request).not.toBeNull());
	const asked = riskConfirmation.request!;
	asked.answer(signs);
	return asked;
}

describe('the gate a network makes — the core rule (§3.5, §3.6)', () => {
	it('is read per network: a refusal comes before a risk; another network says nothing', () => {
		expect(gateOf([risk], 100)).toEqual({ kind: 'risk', chainId: 100, findings: [risk] });
		expect(gateOf([risk, refusal], 100)).toEqual({
			kind: 'refused',
			chainId: 100,
			findings: [refusal]
		});
		expect(gateOf([risk, refusal], 8453)).toEqual({ kind: 'open' });
	});

	it('a transaction Vela does not sign is blocked, with or without a risk; "anyway" stays open', () => {
		const blocked = { allowed: false, anyway: false };
		expect(gateOf([], 100, blocked)).toEqual({ kind: 'blocked', chainId: 100 });
		expect(gateOf([risk], 100, blocked)).toEqual({ kind: 'blocked', chainId: 100 });
		// A refused network still says so first: read-only there.
		expect(gateOf([refusal], 100, blocked).kind).toBe('refused');
		expect(gateOf([], 100, { allowed: true, anyway: true })).toEqual({ kind: 'open' });
	});
});

describe('before any signature (§3.5, §3.6)', () => {
	it('only its members act there: signs, asking nothing — the chain read for this signature', async () => {
		await authorizeSigning(account, 100, tx);
		expect(riskConfirmation.request).toBeNull();
		expect(seams.checks).toBe(1);
		expect(seams.reviews).toBe(1);
	});

	it('reads the chain again before EVERY signature: a module added since is caught', async () => {
		await authorizeSigning(account, 100, tx);
		// Mid-session, a module is enabled on chain.
		seams.found = [risk];
		const next = authorizeSigning(account, 100, tx);
		const asked = await answerNext(false);
		expect(asked.findings).toEqual([risk]);
		await expect(next).rejects.toMatchObject({ name: 'AbortError' });
		expect(seams.checks).toBe(2);
		expect(seams.reviews).toBe(2);
	});

	it('its execution changes who controls it, undeclared: blocked, no override, nothing asked', async () => {
		seams.sign = { allowed: false, anyway: false };
		seams.found = [risk];
		const blocked = await authorizeSigning(account, 100, tx).catch((e: unknown) => e);
		expect(blocked).toBeInstanceOf(SigningBlockedError);
		expect(riskConfirmation.request).toBeNull();
	});

	it('its genesis refused there: never signs, and asks nothing', async () => {
		seams.found = [refusal];
		const refused = await authorizeSigning(account, 100, tx).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(SigningRefusedError);
		expect((refused as InstanceType<typeof SigningRefusedError>).findings).toEqual([refusal]);
		expect(riskConfirmation.request).toBeNull();
	});

	it('a risk there: asks, naming what was found — and again for the next signature', async () => {
		seams.found = [risk];
		const first = authorizeSigning(account, 100, tx);
		const asked = await answerNext(true);
		expect(asked).toMatchObject({ chainId: 100, safe: SAFE, findings: [risk] });
		await expect(first).resolves.toBeUndefined();
		// Answered: the confirmation is gone (nothing left on screen).
		expect(riskConfirmation.request).toBeNull();
		// Nothing remembered: the next signature asks again.
		const second = authorizeSigning(account, 100, tx);
		await answerNext(true);
		await expect(second).resolves.toBeUndefined();
	});

	it('the risk declined: a cancel (AbortError), nothing signed', async () => {
		seams.found = [risk];
		const declined = authorizeSigning(account, 100, tx);
		await answerNext(false);
		await expect(declined).rejects.toMatchObject({ name: 'AbortError' });
		expect(riskConfirmation.request).toBeNull();
	});

	it('no screen up to ask: declined, never signed unasked', async () => {
		seams.found = [risk];
		detach?.();
		detach = null;
		await expect(authorizeSigning(account, 100, tx)).rejects.toMatchObject({ name: 'AbortError' });
	});

	it('the screen goes while it asks: declined', async () => {
		seams.found = [risk];
		const asking = authorizeSigning(account, 100, tx);
		await vi.waitFor(() => expect(riskConfirmation.request).not.toBeNull());
		detach?.();
		detach = null;
		await expect(asking).rejects.toMatchObject({ name: 'AbortError' });
	});

	it('who can act for it there could not be read: nothing signed yet', async () => {
		seams.found = null;
		await expect(authorizeSigning(account, 100, tx)).rejects.toBeInstanceOf(SigningUncheckedError);
	});

	it('the state could not be read at all: nothing signed yet', async () => {
		seams.sign = null;
		await expect(authorizeSigning(account, 100, tx)).rejects.toBeInstanceOf(SigningUncheckedError);
	});
});
