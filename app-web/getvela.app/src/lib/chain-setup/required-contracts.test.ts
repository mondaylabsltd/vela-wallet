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

interface WalletContract {
	name: string;
	address: string;
	multiKeyOnly: boolean;
}

function parseRequired(src: string): WalletContract[] {
	const block = src.match(
		/pub const REQUIRED_CONTRACTS: \[\(&str, &str, bool\); (\d+)\] = \[([\s\S]*?)\n\];/
	);
	expect(block, 'REQUIRED_CONTRACTS not found in network_admin.rs').not.toBeNull();
	const rows = [
		...block![2].matchAll(/\(\s*"([^"]+)",\s*"(0x[0-9a-fA-F]{40})",\s*(true|false),?\s*\)/g)
	].map((m) => ({ name: m[1], address: m[2], multiKeyOnly: m[3] === 'true' }));
	expect(rows).toHaveLength(Number(block![1]));
	return rows;
}

describe('the admission bar is the wallet’s, in the wallet’s order', () => {
	const src = coreSource();

	it('lists exactly the contracts network_admin.rs requires', () => {
		const wallet = parseRequired(src);
		expect(
			REQUIRED_CONTRACTS.map((c) => ({
				name: c.name,
				address: c.address,
				multiKeyOnly: c.multiKeyOnly === true
			}))
		).toEqual(wallet);
	});

	it('marks the same two contracts as multi-key-only as the wallet does', () => {
		// The wallet's third field. A chain missing these runs a one-key wallet
		// and refuses a wallet made from two to seven keys — the page must not
		// blur the two, in either direction.
		const walletOnly = parseRequired(src)
			.filter((c) => c.multiKeyOnly)
			.map((c) => c.name);
		expect(walletOnly).toEqual(['Safe Passkey Signer Factory', 'Safe Passkey Signer Singleton']);
		expect(REQUIRED_CONTRACTS.filter((c) => c.multiKeyOnly).map((c) => c.name)).toEqual(walletOnly);
	});

	it('gives every contract a deployment route, and the singleton none of its own', () => {
		// `with-factory` is not a gap in the data: Safe's factory deploys the
		// singleton in its own constructor, so a "Deploy" button for it would be
		// a button for something nobody can do.
		for (const c of REQUIRED_CONTRACTS) {
			if (c.method === 'with-factory') {
				expect(c.arrivesWith, `${c.key} must say what brings it`).toBeDefined();
				expect(REQUIRED_CONTRACTS.some((o) => o.key === c.arrivesWith)).toBe(true);
			} else {
				expect(c.arrivesWith).toBeUndefined();
			}
		}
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
