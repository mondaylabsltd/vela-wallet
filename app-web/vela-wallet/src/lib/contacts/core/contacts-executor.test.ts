/**
 * The web contacts executor (spec 024 T030; identity + classification live
 * in 025 Phase 5): stored-shape byte compatibility, defensive coercion, the
 * forwarded identity / raw bytecode, and the fail-closed twins.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ContactEffect } from './contacts-types';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));

const identity = { value: null as { name: string; source: string } | null };
vi.mock('$lib/services/recipient-identity', () => ({
	resolveRecipientIdentity: vi.fn(async () => identity.value)
}));
const rpc = { response: { jsonrpc: '2.0', id: 1 } as Record<string, unknown> };
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: vi.fn(async () => rpc.response)
}));

import { contactOperationFailure, executeContactOperation } from './contacts-executor';

const effect = (operation: ContactEffect['operation']): ContactEffect => ({ id: 1, operation });

const ADDR = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';

beforeEach(() => kv.clear());

describe('read_store', () => {
	it('translates the stored camelCase records and the tombstone map', async () => {
		kv.set(
			'vela.contacts',
			JSON.stringify([
				{
					address: ADDR.toUpperCase(),
					name: 'Alice',
					kind: 'eoa',
					favorite: true,
					txCount: 3,
					lastUsed: 1000,
					firstSeen: 500,
					source: 'manual'
				}
			])
		);
		kv.set('vela.contacts.dismissed', JSON.stringify({ [ADDR]: 999 }));
		kv.set('vela.contactGroups', JSON.stringify([{ id: 'g1', name: '工资单', members: [ADDR] }]));

		const result = await executeContactOperation(effect({ type: 'read_store' }));
		expect(result.type).toBe('store_loaded');
		if (result.type !== 'store_loaded') return;
		// Address lowercased — the canonical key.
		expect(result.contacts[0]).toMatchObject({
			address: ADDR.toLowerCase(),
			name: 'Alice',
			favorite: true,
			tx_count: 3,
			last_used_ms: 1000,
			source: 'manual'
		});
		expect(result.tombstones).toEqual([{ address: ADDR, dismissed_at_ms: 999 }]);
		expect(result.groups[0]).toMatchObject({ id: 'g1', name: '工资单', members: [ADDR] });
	});

	it('coerces malformed rows instead of rejecting the whole book', async () => {
		kv.set(
			'vela.contacts',
			JSON.stringify([
				{ address: ADDR, kind: 'nonsense', txCount: 'three', lastUsed: null, source: 'weird' },
				{ noAddress: true },
				null,
				// set-23: not an address, and the same address twice.
				{ address: 'not-an-address', name: 'Ghost' },
				{ address: '0x1234', name: 'Short' },
				{ address: ADDR.toUpperCase().replace('0X', '0x'), name: 'Twin' }
			])
		);
		const result = await executeContactOperation(effect({ type: 'read_store' }));
		if (result.type !== 'store_loaded') throw new Error('wrong variant');
		expect(result.contacts).toHaveLength(1);
		expect(result.contacts[0].name).toBeNull();
		expect(result.contacts[0]).toMatchObject({
			kind: 'unknown',
			tx_count: 0,
			last_used_ms: 0,
			source: 'manual'
		});
	});
});

describe('writes keep the Expo stored shapes', () => {
	it('write_contacts omits null optionals, exactly as services/contacts.ts', async () => {
		await executeContactOperation(
			effect({
				type: 'write_contacts',
				contacts: [
					{
						address: ADDR,
						name: 'Alice',
						resolved_name: null,
						resolved_source: null,
						kind: 'eoa',
						favorite: false,
						note: null,
						tx_count: 0,
						last_used_ms: 1,
						first_seen_ms: 1,
						source: 'manual'
					}
				]
			})
		);
		const stored = JSON.parse(kv.get('vela.contacts')!)[0];
		expect(stored).toEqual({
			address: ADDR,
			name: 'Alice',
			kind: 'eoa',
			favorite: false,
			txCount: 0,
			lastUsed: 1,
			firstSeen: 1,
			source: 'manual'
		});
		expect('resolvedName' in stored).toBe(false);
		expect('note' in stored).toBe(false);
	});

	it('write_dismissed stores the address→ms MAP, not a list', async () => {
		await executeContactOperation(
			effect({ type: 'write_dismissed', tombstones: [{ address: ADDR, dismissed_at_ms: 42 }] })
		);
		expect(JSON.parse(kv.get('vela.contacts.dismissed')!)).toEqual({ [ADDR]: 42 });
	});
});

describe('the 025 seams answer, never skip', () => {
	it('load_send_history is truthfully empty', async () => {
		expect(await executeContactOperation(effect({ type: 'load_send_history' }))).toEqual({
			type: 'history_loaded',
			txs: []
		});
	});
	it('resolve_identity forwards the waterfall: a name with its source, or null', async () => {
		identity.value = { name: 'vitalik.eth', source: 'ENS' };
		expect(
			await executeContactOperation(effect({ type: 'resolve_identity', address: ADDR }))
		).toEqual({
			type: 'identity_resolved',
			address: ADDR,
			identity: { name: 'vitalik.eth', source: 'ENS' }
		});
		identity.value = null;
		expect(
			await executeContactOperation(effect({ type: 'resolve_identity', address: ADDR }))
		).toEqual({ type: 'identity_resolved', address: ADDR, identity: null });
	});
	it('classify_recipient hands back the raw bytecode; a non-answer is null, never a verdict', async () => {
		rpc.response = { jsonrpc: '2.0', id: 1, result: '0x6080604052' };
		expect(
			await executeContactOperation(
				effect({ type: 'classify_recipient', chain_id: 1, address: ADDR })
			)
		).toEqual({ type: 'recipient_classified', chain_id: 1, address: ADDR, code: '0x6080604052' });
		rpc.response = { jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'nope' } };
		expect(
			await executeContactOperation(
				effect({ type: 'classify_recipient', chain_id: 1, address: ADDR })
			)
		).toEqual({ type: 'recipient_classified', chain_id: 1, address: ADDR, code: null });
	});
});

describe('the failure twin', () => {
	// set-1: unreadable is not empty — a re-read answering "no contacts" would
	// have the core write the book away.
	it('read_store failure says unreadable, never an empty book', () => {
		expect(contactOperationFailure(effect({ type: 'read_store' }))).toEqual({
			type: 'store_unreadable'
		});
	});
	it('a storage fault on read rejects (so the twin answers it)', async () => {
		const storage = await import('$lib/services/storage');
		vi.mocked(storage.getItem).mockRejectedValueOnce(new Error('torn database'));
		await expect(executeContactOperation(effect({ type: 'read_store' }))).rejects.toThrow(
			'torn database'
		);
	});
	// set-22: a refused write is said, not swallowed.
	it('a write failure is write_failed', () => {
		expect(contactOperationFailure(effect({ type: 'write_contacts', contacts: [] }))).toEqual({
			type: 'write_failed'
		});
	});
	it('a write tells the other tabs', async () => {
		const sync = await import('./contacts-sync');
		const heard = vi.fn();
		const other = new BroadcastChannel('vela.contacts');
		other.onmessage = heard;
		await executeContactOperation(effect({ type: 'write_groups', groups: [] }));
		await vi.waitFor(() => expect(heard).toHaveBeenCalled());
		other.close();
		sync.resetContactsSyncForTests();
	});
	it('history failure has its own variant', () => {
		expect(contactOperationFailure(effect({ type: 'load_send_history' }))).toEqual({
			type: 'history_failed'
		});
	});
});
