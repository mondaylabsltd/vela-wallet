/**
 * The test signer against the 108 signatures @noble/curves made before it
 * was removed (4 keys, including 1 and n − 1; 27 hashes, including 0 and one
 * at or above n), and the rule that only tests may use it.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import vectors from './secp256k1.vectors.json';
import { sign, signEthereum } from './secp256k1';

describe('the secp256k1 test signer', () => {
	it('reproduces every recorded signature, r, s and the recovery id', async () => {
		expect(vectors.cases.length).toBe(108);
		for (const c of vectors.cases) {
			const sig = await sign(c.hash, c.key);
			const at = `${c.key.slice(0, 8)}… over ${c.hash.slice(0, 8)}…`;
			expect(sig.r.toString(16).padStart(64, '0'), at).toBe(c.r);
			expect(sig.s.toString(16).padStart(64, '0'), at).toBe(c.s);
			expect(sig.recovery, at).toBe(c.recovery);
		}
	});

	it('writes the Ethereum form, v = 27 + recovery', async () => {
		const c = vectors.cases[0];
		expect(await signEthereum(`0x${c.hash}`, c.key)).toBe(
			`0x${c.r}${c.s}${(27 + c.recovery).toString(16)}`
		);
	});

	it('refuses what is not a key or a 32-byte hash', async () => {
		const hash = vectors.cases[0].hash;
		await expect(sign(hash, '00'.repeat(32))).rejects.toThrow('not a private key');
		await expect(sign('abcd', vectors.cases[0].key)).rejects.toThrow('32-byte hash');
	});

	it('is imported by tests and nothing else', () => {
		const offenders: string[] = [];
		const walk = (dir: string) => {
			for (const name of readdirSync(dir)) {
				const path = join(dir, name);
				if (statSync(path).isDirectory()) walk(path);
				else if (
					/\.(ts|svelte)$/.test(name) &&
					!/\.test\.ts$/.test(name) &&
					!path.includes('/testing/')
				)
					if (readFileSync(path, 'utf8').includes('testing/secp256k1')) offenders.push(path);
			}
		};
		walk('src');
		expect(offenders).toEqual([]);
	});
});
