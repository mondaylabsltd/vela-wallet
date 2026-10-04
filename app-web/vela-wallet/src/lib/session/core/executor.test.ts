/**
 * Issue 409 — the session executor's three landing-watch operations.
 *
 * A one-key wallet is entered at the registry's 202; its pending record keeps
 * the task id, and the session confirms the landing by waiting on that task —
 * a read, never a passkey — before the record may go. These pin what this
 * shell answers; the rules are the core's (`app_session_landing_409.rs`).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { STORAGE_KEYS } from '$lib/onboarding/core/storage';
import { executeSession, sessionFailure } from './executor';

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
		expect(
			sessionFailure(effect({ type: 'await_registry_landing', task_id: 't1' })).type
		).toBe('registry_landing_unconfirmed');
	});
});
