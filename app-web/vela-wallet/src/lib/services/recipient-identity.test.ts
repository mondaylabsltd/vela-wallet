/**
 * The identity waterfall (spec 025 Phase 5): order, positive-only caching,
 * and the answers that must be null without touching the network.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
// The kernel is Rust-tested; here any deterministic 32 bytes will do.
vi.mock('$lib/core/client', () => ({
	keccak256: (data: Uint8Array) => {
		const out = new Uint8Array(32);
		for (let i = 0; i < data.length; i++) out[i % 32] ^= data[i];
		out[0] |= 1;
		return out;
	}
}));
const index = { record: null as { name: string } | null, calls: 0 };
vi.mock('$lib/services/public-key-index', () => ({
	queryByWalletRef: vi.fn(async () => {
		index.calls += 1;
		return index.record;
	})
}));
/** chainId → (to → result). Anything unlisted answers `0x`. */
const chain = new Map<number, Map<string, string>>();
const rpcCalls: number[] = [];
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: vi.fn(async (_method: string, params: unknown[], chainId: number) => {
		rpcCalls.push(chainId);
		const to = (params[0] as { to: string }).to.toLowerCase();
		return { jsonrpc: '2.0', id: 1, result: chain.get(chainId)?.get(to) ?? '0x' };
	})
}));

import { decodeString, resolveRecipientIdentity } from './recipient-identity';

const ADDR = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
const RESOLVER = '0x' + '11'.repeat(20);
const word = (hex: string) => hex.replace(/^0x/, '').padStart(64, '0');
/** ABI-encode a `string` return value. */
function abiString(s: string): string {
	const bytes = new TextEncoder().encode(s);
	let data = '';
	for (const b of bytes) data += b.toString(16).padStart(2, '0');
	return '0x' + word('20') + word(bytes.length.toString(16)) + data.padEnd(64, '0');
}
function serve(chainId: number, registry: string, name: string) {
	const m = new Map<string, string>();
	m.set(registry.toLowerCase(), '0x' + word(RESOLVER));
	m.set(RESOLVER, abiString(name));
	chain.set(chainId, m);
}

beforeEach(() => {
	kv.clear();
	chain.clear();
	rpcCalls.length = 0;
	index.record = null;
	index.calls = 0;
});

describe('resolveRecipientIdentity', () => {
	it('malformed and zero addresses are null with zero network', async () => {
		expect(await resolveRecipientIdentity('not-an-address')).toBeNull();
		expect(await resolveRecipientIdentity('0x' + '0'.repeat(40))).toBeNull();
		expect(index.calls).toBe(0);
		expect(rpcCalls).toEqual([]);
	});

	it('the passkey index wins and the hit is cached — the second ask costs nothing', async () => {
		index.record = { name: 'Alice' };
		expect(await resolveRecipientIdentity(ADDR)).toEqual({ name: 'Alice', source: 'passkey' });
		expect(rpcCalls).toEqual([]);
		index.record = null;
		expect(await resolveRecipientIdentity(ADDR)).toEqual({ name: 'Alice', source: 'passkey' });
		expect(index.calls).toBe(1);
	});

	it('name services answer in priority order (.bnb before ENS) and decode the string', async () => {
		serve(1, '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e', 'alice.eth');
		serve(56, '0x08CEd32a7f3eeC915Ba84415e9C07a7286977956', 'alice.bnb');
		expect(await resolveRecipientIdentity(ADDR)).toEqual({ name: 'alice.bnb', source: '.bnb' });
	});

	it('ENS alone resolves with its own label', async () => {
		serve(1, '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e', 'alice.eth');
		expect(await resolveRecipientIdentity(ADDR)).toEqual({ name: 'alice.eth', source: 'ENS' });
	});

	it('nothing anywhere is null and NOT cached (a later positive still gets through)', async () => {
		expect(await resolveRecipientIdentity(ADDR)).toBeNull();
		expect([...kv.keys()].some((k) => k.startsWith('recipient_id:'))).toBe(false);
		index.record = { name: 'Late' };
		expect(await resolveRecipientIdentity(ADDR)).toEqual({ name: 'Late', source: 'passkey' });
	});
});

describe('decodeString', () => {
	it('decodes offset + length + padded bytes; rejects short/empty payloads', () => {
		expect(decodeString(abiString('vitalik.eth'))).toBe('vitalik.eth');
		expect(decodeString('0x' + word('20') + word('0'))).toBeNull();
		expect(decodeString('0x1234')).toBeNull();
	});

	it('a multibyte name survives whole', () => {
		expect(decodeString(abiString('小明.eth'))).toBe('小明.eth');
		expect(decodeString(abiString('jxjjx\u{1f600}'))).toBe('jxjjx\u{1f600}');
	});

	/**
	 * Issue 200. The lenient decoder turned every unreadable byte into U+FFFD,
	 * and that name was cached, adopted as a contact's resolved name and drawn
	 * in the picker — "jxjjx????" where a person's name belonged. A name beside
	 * somebody's money is the real one or nothing, the rule the desktop shell's
	 * `decode_name` already states and tests.
	 */
	it('refuses non-UTF-8 bytes rather than naming someone with replacement characters', () => {
		const bytes = (hex: string) =>
			'0x' + word('20') + word((hex.length / 2).toString(16)) + hex.padEnd(64, '0');
		// A truncated 4-byte sequence (an emoji cut in half) and a lone
		// continuation byte: both decode leniently to U+FFFD.
		expect(decodeString(bytes('6a786a6a78f09f'))).toBeNull();
		expect(decodeString(bytes('ff'))).toBeNull();
		expect(decodeString(bytes('e4bda0ff'))).toBeNull();
	});

	it('a name that is only whitespace is no name', () => {
		expect(decodeString(abiString('   '))).toBeNull();
		expect(decodeString(abiString('  vitalik.eth '))).toBe('vitalik.eth');
	});
});
