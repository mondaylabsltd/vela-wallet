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
	},
	loadCore: async () => {},
	get verifiedNameStep() {
		return coreVerifier;
	}
}));

/**
 * A stand-in for `vela_core::app::name_verify` (spec 081, FR-010).
 *
 * The RULE — which calls, in what order, what silence means, and the
 * comparison — is the core's and is proven there, against its own transcripts
 * (`rust/crates/vela-core/tests/app_name_verify.rs`, 15 cases). What these
 * tests pin is the half that is this shell's: that the transcript is driven to
 * its end, each `eth_call` performed through the pool, an unanswered call
 * reported as `failed` rather than as an empty body, and only a `verified`
 * verdict drawn. A second implementation of the rule would be worth nothing;
 * this is a scripted counterpart, and `null` turns the verifier off entirely.
 */
let coreVerifier: unknown = null;

const standInVerifier = (
	chainId: number,
	registry: string,
	address: string,
	name: string,
	answersJson: string
): string => {
	const answers = JSON.parse(answersJson) as {
		id: string;
		outcome: string;
		body: string | null;
	}[];
	const at = (id: string) => answers.find((a) => a.id === id);
	const done = (state: string, verified?: string) =>
		JSON.stringify({ type: 'done', forward_state: state, name: verified ?? null });
	const call = (id: string, to: string, selector: string) =>
		JSON.stringify({
			type: 'ask',
			requests: [
				{ type: 'eth_call', id, chain_id: chainId, to, data: '0x' + selector + '00'.repeat(32) }
			]
		});

	const resolverAnswer = at('resolver:0');
	if (!resolverAnswer) return call('resolver:0', registry, '0178b8bf');
	if (resolverAnswer.outcome !== 'ok' || !resolverAnswer.body) return done('unavailable');
	const resolver = '0x' + resolverAnswer.body.slice(-40);
	if (/^0x0+$/.test(resolver)) return done('mismatch');

	const addrAnswer = at('addr');
	if (!addrAnswer) return call('addr', resolver, '3b3b57de');
	if (addrAnswer.outcome !== 'ok' || (addrAnswer.body?.length ?? 0) < 66) return done('unavailable');
	const found = '0x' + (addrAnswer.body ?? '').slice(-40);
	if (/^0x0+$/.test(found)) return done('mismatch');
	return found.toLowerCase() === address.toLowerCase()
		? done('verified', name.toLowerCase())
		: done('mismatch');
};
const index = { record: null as { name: string } | null, calls: 0 };
vi.mock('$lib/services/public-key-index', () => ({
	queryWalletName: vi.fn(async () => {
		index.calls += 1;
		return index.record;
	})
}));
/** chainId → (`to:selector` → result). Anything unlisted answers `0x`. */
const chain = new Map<number, Map<string, string>>();
const rpcCalls: number[] = [];
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: vi.fn(async (_method: string, params: unknown[], chainId: number) => {
		rpcCalls.push(chainId);
		const { to, data } = params[0] as { to: string; data: string };
		const key = to.toLowerCase() + ':' + data.slice(2, 10);
		return { jsonrpc: '2.0', id: 1, result: chain.get(chainId)?.get(key) ?? '0x' };
	})
}));

import { decodeString, resolveRecipientIdentity } from './recipient-identity';

const ADDR = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
const RESOLVER = '0x' + '11'.repeat(20);
/** Somebody else entirely — the address a poisoned record really resolves to. */
const THEIRS = '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045';
const word = (hex: string) => hex.replace(/^0x/, '').padStart(64, '0');
/** ABI-encode a `string` return value. */
function abiString(s: string): string {
	const bytes = new TextEncoder().encode(s);
	let data = '';
	for (const b of bytes) data += b.toString(16).padStart(2, '0');
	return '0x' + word('20') + word(bytes.length.toString(16)) + data.padEnd(64, '0');
}
/**
 * A registry that reverse-resolves ADDR to `name`, and forward-resolves that
 * name to `forwardsTo` — the address the record must land back on to count.
 * `null` is a name with no forward record at all.
 */
function serve(chainId: number, registry: string, name: string, forwardsTo: string | null = ADDR) {
	const m = new Map<string, string>();
	// `resolver(bytes32)`, for the reverse node and the forward one alike.
	m.set(registry.toLowerCase() + ':0178b8bf', '0x' + word(RESOLVER));
	// `name(bytes32)` — the claim.
	m.set(RESOLVER + ':691f3431', abiString(name));
	// `addr(bytes32)` — the forward record that decides whether it is true.
	m.set(RESOLVER + ':3b3b57de', '0x' + word(forwardsTo ?? '0x' + '0'.repeat(40)));
	chain.set(chainId, m);
}

beforeEach(() => {
	kv.clear();
	chain.clear();
	rpcCalls.length = 0;
	index.record = null;
	index.calls = 0;
	coreVerifier = standInVerifier;
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

	/**
	 * The attack this file now refuses (FR-010). `addr.reverse` is writable by
	 * the address itself, so anyone can name their own address `vitalik.eth`.
	 * Resolving that name forward lands somewhere else, so no name is drawn —
	 * and none is cached, which is what kept a poisoned name on screen for a
	 * day at a time.
	 */
	it('a reverse record that resolves elsewhere is not shown and not cached', async () => {
		serve(1, '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e', 'vitalik.eth', THEIRS);
		expect(await resolveRecipientIdentity(ADDR)).toBeNull();
		expect([...kv.keys()].some((k) => k.startsWith('recipient_id.v2:'))).toBe(false);
	});

	it('a claimed name with no forward record at all names nobody', async () => {
		serve(1, '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e', 'alice.eth', null);
		expect(await resolveRecipientIdentity(ADDR)).toBeNull();
	});

	/**
	 * Fail CLOSED. When the rule cannot be run — no core, an unreachable
	 * resolver, a timeout — the address is shown alone. Failing open would hand
	 * whoever poisons a record the ability to choose the moment: knock the
	 * forward lookup over and any name passes.
	 */
	it('a name is not shown when the check cannot be made at all', async () => {
		coreVerifier = null;
		serve(1, '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e', 'alice.eth');
		expect(await resolveRecipientIdentity(ADDR)).toBeNull();
	});

	/**
	 * The string drawn is the one the core PROVED, not the one the reverse
	 * record spelled: `Alice.ETH` and `alice.eth` are one ENS name but two
	 * different rows in a list.
	 */
	it('the name shown is the verified, normalised one', async () => {
		serve(1, '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e', 'Alice.ETH');
		expect(await resolveRecipientIdentity(ADDR)).toEqual({ name: 'alice.eth', source: 'ENS' });
	});

	/** An unanswered forward lookup is `failed`, never an empty body. */
	it('a forward lookup the pool cannot make leaves the name unshown', async () => {
		serve(1, '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e', 'alice.eth');
		// The resolver answers the reverse question and then goes quiet.
		chain.get(1)!.delete(RESOLVER + ':3b3b57de');
		expect(await resolveRecipientIdentity(ADDR)).toBeNull();
	});

	it('nothing anywhere is null and NOT cached (a later positive still gets through)', async () => {
		expect(await resolveRecipientIdentity(ADDR)).toBeNull();
		expect([...kv.keys()].some((k) => k.startsWith('recipient_id.v2:'))).toBe(false);
		index.record = { name: 'Late' };
		expect(await resolveRecipientIdentity(ADDR)).toEqual({ name: 'Late', source: 'passkey' });
	});

	it('two askers in the same tick share ONE waterfall (the book and the feed, issue 191)', async () => {
		// Casing differs on purpose: the feed hands over a checksummed address.
		const [a, b] = await Promise.all([
			resolveRecipientIdentity(ADDR),
			resolveRecipientIdentity('0x' + ADDR.slice(2).toUpperCase())
		]);
		expect(a).toBeNull();
		expect(b).toBeNull();
		expect(index.calls).toBe(1);
		expect(rpcCalls).toHaveLength(5); // one reverse lookup per name service, not ten

		// Coalescing is not caching: a miss is asked again the next time.
		await resolveRecipientIdentity(ADDR);
		expect(index.calls).toBe(2);
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
