#!/usr/bin/env node
// §37 (review 1-11): the wallet's message signatures, verified by viem —
// `verifyMessage` / `verifyTypedData`, whose universal validation runs
// ERC-6492's deployless UniversalSigValidator (it simulates the account's
// own deployment, then asks it `isValidSignature`) — on the fork harness.
//
//   node scripts/onchain/erc6492/verify.mjs <vectors.json> <rpc url>
//
// <vectors.json> is what `rust/crates/vela-core/tests/erc6492_fork.rs` wrote
// (VELA_6492_OUT). viem is read from VIEM_DIR (default: the main checkout's
// app-web/getvela.app/node_modules/viem, which pins it), so this script adds
// no dependency of its own.
//
// Every case must verify; its tampered twin must not; and an UNDEPLOYED
// account's bare EIP-1271 signature must not either — the reason the
// envelope exists.
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const [vectorsPath, rpcUrl] = process.argv.slice(2);
if (!vectorsPath || !rpcUrl) {
  console.error('usage: verify.mjs <vectors.json> <rpc url>');
  process.exit(2);
}

const here = dirname(fileURLToPath(import.meta.url));
const candidates = [
  process.env.VIEM_DIR,
  join(here, '../../../app-web/getvela.app/node_modules/viem'),
  join(here, '../../../../../../app-web/getvela.app/node_modules/viem'),
].filter(Boolean);
const viemDir = candidates.find((dir) => existsSync(join(dir, 'package.json')));
if (!viemDir) {
  console.error(`viem not found (tried ${candidates.join(', ')}); set VIEM_DIR`);
  process.exit(2);
}
const viem = await import(pathToFileURL(join(resolve(viemDir), '_esm/index.js')).href);
const { createPublicClient, http, defineChain } = viem;
const version = JSON.parse(readFileSync(join(viemDir, 'package.json'), 'utf8')).version;

const vectors = JSON.parse(readFileSync(vectorsPath, 'utf8'));
const chain = defineChain({
  id: vectors.chainId,
  name: 'fork',
  nativeCurrency: { name: 'xDAI', symbol: 'xDAI', decimals: 18 },
  rpcUrls: { default: { http: [rpcUrl] } },
});
const client = createPublicClient({ chain, transport: http(rpcUrl) });

const tamper = (sig) => {
  // Flip one bit inside the passkey's signature, well before the 6492 suffix.
  const at = Math.floor(sig.length / 2);
  const nibble = (parseInt(sig[at], 16) ^ 1).toString(16);
  return sig.slice(0, at) + nibble + sig.slice(at + 1);
};

const verify = async (c, signature) => {
  if (c.kind === 'typed_data') {
    const { EIP712Domain, ...types } = c.message.types;
    void EIP712Domain;
    return client.verifyTypedData({
      address: c.address,
      domain: c.message.domain,
      types,
      primaryType: c.message.primaryType,
      message: c.message.message,
      signature,
    });
  }
  return client.verifyMessage({ address: c.address, message: c.message, signature });
};

let failed = 0;
const check = (ok, what) => {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${what}`);
  if (!ok) failed += 1;
};

console.log(`viem ${version}, chain ${vectors.chainId} at ${rpcUrl}`);
for (const c of vectors.cases) {
  const label = `${c.kind} from a ${c.deployed ? 'deployed' : 'counterfactual'} account`;
  check(
    c.deployed ? !c.signature.endsWith('6492'.repeat(16)) : c.signature.endsWith('6492'.repeat(16)),
    `${label}: ${c.deployed ? 'bare EIP-1271' : 'ERC-6492 envelope'}`,
  );
  check((await verify(c, c.signature)) === true, `${label}: verifies`);
  check((await verify(c, tamper(c.signature))) === false, `${label}: a tampered one does not`);
  if (!c.deployed) {
    check(
      (await verify(c, c.bare).catch(() => false)) === false,
      `${label}: the bare EIP-1271 signature alone does not (no code to ask)`,
    );
  }
}
console.log(failed ? `\n${failed} check(s) failed` : `\nall ${vectors.cases.length} cases verified`);
process.exit(failed ? 1 : 0);
