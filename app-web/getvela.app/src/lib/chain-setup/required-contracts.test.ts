import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
	P256_PRECOMPILE,
	REQUIRED_CONTRACTS,
	VALID_P256_CALL,
	ARACHNID_PROXY,
	SAFE_SINGLETON_FACTORY
} from './required-contracts';

/**
 * The page's admission bar IS the wallet's. This reads the Rust source off disk
 * and parses the literals out of it — the same discipline `locales.test.ts`
 * applies to the wallet's locale list — so the two cannot drift silently. A
 * contract added to the wallet's checker without landing here fails this test;
 * a contract listed here that the wallet does not require fails it too.
 */
function coreSource(): string {
	return readFileSync(
		fileURLToPath(
			new URL('../../../../../rust/crates/vela-core/src/app/network_admin.rs', import.meta.url)
		),
		'utf8'
	);
}

function parseRequired(src: string): { name: string; address: string }[] {
	const block = src.match(
		/pub const REQUIRED_CONTRACTS: \[\(&str, &str\); (\d+)\] = \[([\s\S]*?)\n\];/
	);
	expect(block, 'REQUIRED_CONTRACTS not found in network_admin.rs').not.toBeNull();
	const pairs = [...block![2].matchAll(/\(\s*"([^"]+)",\s*"(0x[0-9a-fA-F]{40})",?\s*\)/g)].map(
		(m) => ({ name: m[1], address: m[2] })
	);
	expect(pairs).toHaveLength(Number(block![1]));
	return pairs;
}

describe('the admission bar is the wallet’s, in the wallet’s order', () => {
	const src = coreSource();

	it('lists exactly the contracts network_admin.rs requires', () => {
		const wallet = parseRequired(src);
		expect(REQUIRED_CONTRACTS.map((c) => ({ name: c.name, address: c.address }))).toEqual(wallet);
	});

	it('probes the precompile the wallet probes, with the wallet’s vector', () => {
		const addr = src.match(/pub const P256_PRECOMPILE: &str = "(0x[0-9a-fA-F]{40})";/);
		expect(addr?.[1]).toBe(P256_PRECOMPILE);
		const vec = src.match(/pub const VALID_P256_CALL: &str = "0x\\\s*([\s\S]*?)";/);
		expect(vec, 'VALID_P256_CALL not found').not.toBeNull();
		const walletVector = '0x' + vec![1].replace(/\\\s*/g, '').replace(/\s+/g, '');
		expect(VALID_P256_CALL).toBe(walletVector);
	});

	it('names the two factories the CREATE2 contracts deploy through', () => {
		expect(REQUIRED_CONTRACTS.find((c) => c.key === 'arachnidProxy')?.address).toBe(ARACHNID_PROXY);
		expect(REQUIRED_CONTRACTS.find((c) => c.key === 'safeSingletonFactory')?.address).toBe(
			SAFE_SINGLETON_FACTORY
		);
		for (const c of REQUIRED_CONTRACTS) {
			if (c.method === 'create2') expect(c.factory, `${c.key} has no factory`).toBeDefined();
			else expect(c.factory).toBeUndefined();
		}
	});
});
