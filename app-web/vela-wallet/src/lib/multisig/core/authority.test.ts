/**
 * Who can act for a multisig (§3.5), read with the wallet's own RPC and
 * decided by the shipped core: the chain's code for a genesis not deployed
 * yet (Safe's own, as Gnosis holds it — the core's test vectors), and a
 * deployed Safe's modules, guards, handler and singleton.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const VECTORS = '../../rust/crates/vela-core/tests/vectors';
const code = (name: string) => readFileSync(`${VECTORS}/safe-code/${name}.hex`, 'utf8').trim();
/** Safe v1.4.1 on Gnosis, by address. */
const SAFE_CODE: Record<string, string> = {
	'0x4e1dcf7ad4e460cfd30791ccc4f9c8a4f820ec67': 'safe-proxy-factory-1.4.1',
	'0x41675c099f32341bf84bfc5382af534df5c7461a': 'safe-1.4.1',
	'0x29fcb43b46531bca003ddc8fcb67ffe91900c762': 'safe-l2-1.4.1',
	'0xfd0732dc9e303f09fcef3a7388ad10a83459ec99': 'compatibility-fallback-handler-1.4.1',
	'0xbd89a1ce4dde368ffab0ec35506eece0b1ffdc54': 'safe-to-l2-setup-1.4.1'
};
const hostile = JSON.parse(readFileSync(`${VECTORS}/multisig/hostile-geneses.json`, 'utf8')) as {
	factory: string;
	singleton: string;
	attacker: string;
	geneses: { name: string; saltNonce: string; initializer: string; address?: string }[];
};

const ZERO_WORD = `0x${'0'.repeat(64)}`;
const word = (a: string) => `0x${a.slice(2).toLowerCase().padStart(64, '0')}`;

const seams = vi.hoisted(() => ({
	/** The chain does not answer `eth_getCode`. */
	codeDown: false,
	/** A deployed Safe's storage, by slot; its `getModulesPaginated` answer. */
	storage: {} as Record<string, string>,
	modules: '',
	asked: [] as string[]
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (method: string, params: unknown[]) => {
		seams.asked.push(method);
		if (method === 'eth_getCode') {
			if (seams.codeDown) return { error: { message: 'down' } };
			const name = SAFE_CODE[String(params[0]).toLowerCase()];
			return { result: name ? code(name) : '0x' };
		}
		if (method === 'eth_getStorageAt') {
			return { result: seams.storage[String(params[1])] ?? ZERO_WORD };
		}
		if (method === 'eth_call') return { result: seams.modules };
		throw new Error(`unexpected ${method}`);
	}
}));

let kernel: typeof import('./kernel');
let authority: typeof import('./authority');

beforeAll(async () => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	kernel = await import('./kernel');
	authority = await import('./authority');
});

beforeEach(() => {
	seams.codeDown = false;
	seams.storage = {};
	seams.modules = '';
	seams.asked = [];
});

function hostileGenesis(name: string) {
	const row = hostile.geneses.find((g) => g.name === name)!;
	const genesis = {
		factory: hostile.factory,
		singleton: hostile.singleton,
		saltNonce: row.saltNonce,
		initializer: row.initializer
	};
	return { genesis, address: kernel.genesisAddress(genesis) };
}

describe('a multisig not deployed on a chain yet', () => {
	it("is Verified where Safe's own code is: its genesis's members, nothing else", async () => {
		const ms = kernel.multisigGenesis([hostile.attacker, hostile.factory], 2, 'Team', '0x7');
		expect(await authority.counterfactualOn(100, ms.address, ms.genesis)).toEqual({
			kind: 'verified',
			owners: ms.owners,
			threshold: 2
		});
	});

	it('is refused for a module its setup enables — the Safe deployed on Arc with one', async () => {
		const { genesis, address } = hostileGenesis('HOOK_MODULE');
		expect(address).toBe('0x2F53cC750594bB97c21Bf3c18A4837bb5Ad911f6');
		expect(await authority.counterfactualOn(5042002, address, genesis)).toEqual({
			kind: 'refused',
			reasons: [{ kind: 'module', address: hostile.attacker }]
		});
		// Nothing on the chain could change that: nothing is read for it.
		expect(seams.asked).toEqual([]);
	});

	it('is not Verified while the chain does not answer', async () => {
		seams.codeDown = true;
		const { genesis, address } = hostileGenesis('CLEAN');
		const verdict = await authority.counterfactualOn(100, address, genesis);
		expect(verdict.kind).toBe('unknown');
	});
});

describe('a deployed Safe', () => {
	it('with nothing besides its members: no risk, its handler and singleton checked', async () => {
		const reads = kernel.deployedReads();
		seams.modules = `0x${(0x40).toString(16).padStart(64, '0')}${word('0x0000000000000000000000000000000000000001').slice(2)}${'0'.repeat(64)}`;
		seams.storage = {
			[reads.handlerSlot]: word('0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99'),
			[reads.singletonSlot]: word('0x29fcB43b46531BcA003ddC8FCB67FFE91900C762')
		};
		expect(await authority.deployedOn(100, hostile.attacker)).toEqual({
			risks: [],
			needCode: [],
			unread: false
		});
		expect(seams.asked.filter((m) => m === 'eth_getCode')).toHaveLength(2);
	});

	it('with a module enabled: said, with its address', async () => {
		const reads = kernel.deployedReads();
		seams.modules = `0x${(0x40).toString(16).padStart(64, '0')}${word('0x0000000000000000000000000000000000000001').slice(2)}${(1).toString(16).padStart(64, '0')}${word(hostile.attacker).slice(2)}`;
		seams.storage = {
			[reads.handlerSlot]: word('0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99'),
			[reads.singletonSlot]: word('0x29fcB43b46531BcA003ddC8FCB67FFE91900C762')
		};
		const check = await authority.deployedOn(5042002, '0x2F53cC750594bB97c21Bf3c18A4837bb5Ad911f6');
		expect(check.risks).toEqual([{ kind: 'module', address: hostile.attacker }]);
	});
});
