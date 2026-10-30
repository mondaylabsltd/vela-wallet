/**
 * Approve, Propose-and-approve and Reject all go through `signApproval` — and it follows where the member reviews and signs
 * (2026-10-29): the trusted signing page the member chose gets the SafeTx
 * itself; Vela's sheet keeps the member's own passkey prompt; a member whose
 * keys nothing here can reach is refused with the core's reason.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SafeTx } from './kernel';

const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
});

const onPage = vi.fn(async () => ({ owner: 'page' }));
vi.mock('./page-approval.svelte', () => ({
	approveOnPage: (...args: unknown[]) => onPage(...(args as []))
}));
const inVela = vi.fn(async () => {
	throw new Error('signed in Vela');
});
vi.mock('$lib/signing/sign-challenge', async (original) => ({
	...(await original<typeof import('$lib/signing/sign-challenge')>()),
	signChallenge: (...args: unknown[]) => inVela(...(args as []))
}));

const { signApproval } = await import('./owners');
const { VenueBlockedError } = await import('$lib/signing/sign-challenge');

const KEY = '04' + '11'.repeat(64);
const TX: SafeTx = {
	to: '0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69',
	value: '0x1',
	data: '0x',
	operation: 0,
	safeTxGas: '0x0',
	baseGas: '0x0',
	gasPrice: '0x0',
	gasToken: '0x0000000000000000000000000000000000000000',
	refundReceiver: '0x0000000000000000000000000000000000000000',
	nonce: '0x1'
};
const HASH = '0x' + '22'.repeat(32);

function account(domain: string, venue: unknown) {
	return {
		id: 'aa01',
		name: 'Alice',
		address: '0x8F1E0bC5D9aA3F6e2B7c4D1a9E8f3B2c5A6d7E01',
		public_key_hex: KEY,
		created_at_iso: '2026-10-29T00:00:00.000Z',
		keys: [{ credential_id: 'aa01', public_key_hex: KEY, name: 'Alice', transports: 'internal' }],
		sign_in_key: { credential_id: 'aa01', method: 'platform', transports: 'internal' },
		signing_domain: domain,
		signing_venue: venue
	};
}

function keep(record: unknown) {
	storage.set('vela.accounts', JSON.stringify([record]));
	return record as Parameters<typeof signApproval>[0];
}

beforeEach(() => {
	storage.clear();
	onPage.mockClear();
	inVela.mockClear();
});

describe('a member approves where it reviews and signs', () => {
	it('on its trusted signing page: the page is handed the SafeTx, no prompt in Vela', async () => {
		const owner = keep(account('getvela.app', { type: 'page', url: 'https://sign.getvela.app/' }));
		await signApproval(owner, 5042002, HASH, {
			multisig: '0x' + '44'.repeat(20),
			name: 'Team fund',
			tx: TX
		});
		expect(onPage).toHaveBeenCalledTimes(1);
		const input = (onPage.mock.calls[0] as unknown as [Record<string, unknown>])[0];
		expect(input.page).toBe('https://sign.getvela.app/');
		expect(input.tx).toEqual(TX);
		expect(input.safeTxHash).toBe(HASH);
		expect(input.multisigName).toBe('Team fund');
		expect(input.keyRoute).toMatchObject({ credential_id: 'aa01', method: 'platform' });
		expect(inVela).not.toHaveBeenCalled();
	});

	it("on a custom domain's own page: there, the only place its keys answer", async () => {
		const owner = keep(
			account('sign.example.com', { type: 'page', url: 'https://sign.example.com/' })
		);
		await signApproval(owner, 5042002, HASH, { multisig: '0x' + '44'.repeat(20), tx: TX });
		expect((onPage.mock.calls[0] as unknown as [Record<string, unknown>])[0].page).toBe(
			'https://sign.example.com/'
		);
	});

	it("in Vela: the member's own passkey prompt, as before", async () => {
		const owner = keep(account('getvela.app', { type: 'in_vela' }));
		await expect(
			signApproval(owner, 5042002, HASH, { multisig: '0x' + '44'.repeat(20), tx: TX })
		).rejects.toThrow('signed in Vela');
		expect(inVela).toHaveBeenCalledTimes(1);
		expect(onPage).not.toHaveBeenCalled();
	});

	it("nowhere, with the core's reason, when nothing here can reach the member's keys", async () => {
		const owner = keep(account('sign.example.com', { type: 'in_vela' }));
		const error = await signApproval(owner, 5042002, HASH, {
			multisig: '0x' + '44'.repeat(20),
			tx: TX
		}).catch((e: unknown) => e);
		expect(error).toBeInstanceOf(VenueBlockedError);
		expect(onPage).not.toHaveBeenCalled();
		expect(inVela).not.toHaveBeenCalled();
	});
});
