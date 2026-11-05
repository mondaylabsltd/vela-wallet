/**
 * Issue 409 — the session executor's three landing-watch operations.
 *
 * A one-key wallet is entered at the registry's 202; its pending record keeps
 * the task id, and the session confirms the landing by waiting on that task —
 * a read, never a passkey — before the record may go. These pin what this
 * shell answers; the rules are the core's (`app_session_landing_409.rs`).
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { STORAGE_KEYS } from '$lib/onboarding/core/storage';
import type { PendingPublish } from '../generated/PendingPublish';
import { executeSession, sessionFailure } from './executor';

// ob-2: the replayed publish is `publish.ts`'s mechanism (its own tests); what
// this shell owes is the arguments it hands over, and the answer it makes.
const publish = vi.hoisted(() =>
	vi.fn<(args: Record<string, unknown>) => Promise<string | null>>()
);
vi.mock('$lib/onboarding/core/publish', () => ({ publish }));
vi.mock('$lib/core/kernels', () => ({
	trustedSignerRegistryRpId: (page: string | null) => (page ? 'sign.example.org' : null)
}));

class MemoryStorage implements Storage {
	#map = new Map<string, string>();
	get length() {
		return this.#map.size;
	}
	clear() {
		this.#map.clear();
	}
	getItem(key: string) {
		return this.#map.get(key) ?? null;
	}
	key(index: number) {
		return [...this.#map.keys()][index] ?? null;
	}
	removeItem(key: string) {
		this.#map.delete(key);
	}
	setItem(key: string, value: string) {
		this.#map.set(key, value);
	}
}

const record = {
	id: 'cred-1',
	name: 'Ann',
	public_key_hex: '04ab',
	attestation_object_hex: 'a0',
	created_at_iso: '2026-10-04T10:00:00.000Z',
	authenticator_attachment: '',
	transports: '',
	members: [],
	task_id: 't1'
};

/** A registry whose task endpoint answers `body`; what it was asked is kept. */
function registry(body: object): string[] {
	const asked: string[] = [];
	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string) => {
			asked.push(new URL(url).pathname);
			return new Response(JSON.stringify(body), {
				status: 200,
				headers: { 'content-type': 'application/json' }
			});
		})
	);
	return asked;
}

const effect = (operation: Parameters<typeof executeSession>[0]['operation']) => ({
	id: 1,
	operation
});

describe('the landing watch, on the web (issue 409)', () => {
	beforeEach(() => {
		(globalThis as { localStorage?: Storage }).localStorage = new MemoryStorage();
		// Something an older build or a test left behind travels too: the core
		// skips what it cannot read, so the shell hands the outbox over whole.
		localStorage.setItem(
			STORAGE_KEYS.pendingUploads,
			JSON.stringify([{ unconfirmed: true }, record])
		);
	});

	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('hands the outbox over as stored', async () => {
		const answer = await executeSession(effect({ type: 'load_pending_uploads' }));
		expect(answer).toEqual({
			type: 'pending_uploads_loaded',
			records: [{ unconfirmed: true }, record]
		});
	});

	it('answers landed once the task is done — with nothing but a read of that task', async () => {
		const asked = registry({ id: 't1', status: 'done', txHash: '0xabc' });
		const answer = await executeSession(effect({ type: 'await_registry_landing', task_id: 't1' }));
		expect(answer).toEqual({ type: 'registry_landed' });
		expect(asked).toEqual(['/api/task/t1']);
	});

	it('answers unconfirmed when the task failed, and removes nothing', async () => {
		registry({ id: 't1', status: 'failed', error: 'execution reverted' });
		const answer = await executeSession(effect({ type: 'await_registry_landing', task_id: 't1' }));
		expect(answer).toEqual({
			type: 'registry_landing_unconfirmed',
			message: 'Register failed: execution reverted'
		});
		expect(localStorage.getItem(STORAGE_KEYS.pendingUploads)).toContain('cred-1');
	});

	it('removes the confirmed record and only that one', async () => {
		const answer = await executeSession(
			effect({ type: 'remove_pending_upload', credential_id: 'cred-1' })
		);
		expect(answer).toEqual({ type: 'pending_upload_removed' });
		expect(JSON.parse(localStorage.getItem(STORAGE_KEYS.pendingUploads) ?? '[]')).toEqual([
			{ unconfirmed: true }
		]);
	});

	it('fails toward keeping the record', () => {
		expect(sessionFailure(effect({ type: 'load_pending_uploads' })).type).toBe(
			'pending_uploads_unavailable'
		);
		expect(sessionFailure(effect({ type: 'await_registry_landing', task_id: 't1' })).type).toBe(
			'registry_landing_unconfirmed'
		);
	});
});

describe('the restore read (settings set-21)', () => {
	let storage: MemoryStorage;
	beforeEach(() => {
		storage = new MemoryStorage();
		vi.stubGlobal('localStorage', storage);
	});
	afterEach(() => vi.unstubAllGlobals());

	it('a damaged account list is a failed read — accounts_unavailable — not an empty one', async () => {
		storage.setItem(STORAGE_KEYS.accounts, '{not json');
		const effect = { id: 1, operation: { type: 'load_accounts' as const } };
		await expect(executeSession(effect)).rejects.toThrow();
		expect(sessionFailure(effect)).toEqual({ type: 'accounts_unavailable' });
		expect(storage.getItem(STORAGE_KEYS.accounts)).toBe('{not json');
	});

	it('nothing stored is still an empty list', async () => {
		const effect = { id: 1, operation: { type: 'load_accounts' as const } };
		expect(await executeSession(effect)).toEqual({ type: 'accounts_loaded', accounts: [] });
	});
});

describe('the replayed publish, on the web (ob-2)', () => {
	const pending: PendingPublish = {
		metadata_hex: 'a1b2',
		members: [
			{
				credential_id: 'cred-1',
				public_key_hex: '04ab',
				attestation_hex: '',
				authenticator_attachment: 'platform',
				transports: 'internal',
				proof: {
					authenticatorData: 'aa',
					clientDataJSON: 'bb',
					challengeIndex: 9,
					typeIndex: 1,
					r: '11',
					s: '22'
				}
			}
		],
		group_seed_hex: '5eed',
		group_public_key_hex: '04feed'
	};

	beforeEach(() => {
		publish.mockReset();
	});

	it('sends the record’s own publish — its proofs, its group key — and waits for the landing', async () => {
		publish.mockResolvedValue(null);
		const answer = await executeSession(
			effect({ type: 'publish_pending_upload', publish: pending })
		);
		expect(answer).toEqual({ type: 'registry_landed' });
		expect(publish).toHaveBeenCalledOnce();
		expect(publish.mock.calls[0][0]).toMatchObject({
			metadataHex: 'a1b2',
			members: pending.members,
			seedHex: '5eed',
			groupPublicKeyHex: '04feed',
			page: null,
			// Never answered at the 202: only a landing removes the record.
			answerWhenAccepted: false
		});
	});

	it('files a custom-domain wallet under its page', async () => {
		publish.mockResolvedValue(null);
		await executeSession(
			effect({
				type: 'publish_pending_upload',
				publish: { ...pending, page: 'https://sign.example.org/' }
			})
		);
		expect(publish.mock.calls[0][0]).toMatchObject({
			rpId: 'sign.example.org',
			page: 'https://sign.example.org/'
		});
	});

	it('a publish that did not land is unconfirmed — the record stays', async () => {
		publish.mockRejectedValue(new Error('Register failed: 500'));
		const answer = await executeSession(
			effect({ type: 'publish_pending_upload', publish: pending })
		);
		expect(answer).toEqual({
			type: 'registry_landing_unconfirmed',
			message: 'Register failed: 500'
		});
		expect(sessionFailure(effect({ type: 'publish_pending_upload', publish: pending })).type).toBe(
			'registry_landing_unconfirmed'
		);
	});
});
