// Cross-language check: our SafeOp digest against vela-core's.
//
// The page's copy (lib/safeop.js) is hand-transcribed from the Rust. Reading
// the two side by side proves nothing — a transposed field or a missing
// keccak still "looks right". So this runs the SHIPPED wasm build of vela-core
// and demands byte equality on the same inputs.
//
//   node samples/safeop-test.mjs
import { readFileSync, readdirSync } from 'node:fs';
import { webcrypto } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const repo = join(root, '..', '..');

const results = [];
function check(name, pass, detail) {
  results.push(pass);
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`);
}

// --- the page's implementation ----------------------------------------------

globalThis.window = globalThis;
Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });
for (const file of ['src/lib/keccak.js', 'src/lib/abi.js', 'src/lib/encode.js', 'src/lib/digest.js', 'src/lib/safeop.js']) {
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}
const ours = globalThis.VelaCS;
const hex = (bytes) => '0x' + Buffer.from(bytes).toString('hex');

// --- vela-core, as compiled for the web -------------------------------------

const core = await import(join(repo, 'rust/pkg-web/vela_core.js'));
// The committed build, whatever its content hash is this week.
const wasmDir = join(repo, 'assets/wasm');
const wasmFile = readdirSync(wasmDir).find((name) => /^vela_core_bg\..*\.wasm$/.test(name));
core.initSync({ module: readFileSync(join(wasmDir, wasmFile)) });

// --- fixtures ---------------------------------------------------------------

const SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const TO = '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1';
const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';

function executeUserOp(to, value, data) {
  return ours.encode.call('executeUserOp(address,uint256,bytes,uint8)', [to, BigInt(value), data, 0]);
}

const transfer = ours.encode.call('transfer(address,uint256)', [TO, 1000000000n]);

const cases = [
  {
    name: 'native transfer, undeployed account (initCode present)',
    op: {
      sender: SAFE,
      nonce: '0x7',
      initCode: '0x' + 'ab'.repeat(120),
      callData: executeUserOp(TO, '0x2386f26fc10000', '0x'),
      verificationGasLimit: '2000000',
      callGasLimit: '200000',
      preVerificationGas: '110000',
      maxFeePerGas: '0',
      maxPriorityFeePerGas: '0',
      paymasterAndData: '0x',
    },
    chainId: 100,
  },
  {
    name: 'ERC-20 transfer, deployed account, real fee fields',
    op: {
      sender: SAFE,
      nonce: '0x2a',
      initCode: '0x',
      callData: executeUserOp(USDC, '0x0', transfer),
      verificationGasLimit: '300000',
      callGasLimit: '250000',
      preVerificationGas: '99999',
      maxFeePerGas: '1500000007',
      maxPriorityFeePerGas: '1500000000',
      paymasterAndData: '0x',
    },
    chainId: 1,
  },
  {
    name: 'paymaster data present, large nonce',
    op: {
      sender: SAFE,
      nonce: '0xffffffffffff',
      initCode: '0x',
      callData: executeUserOp(TO, '0x1', '0xdeadbeef'),
      verificationGasLimit: '1',
      callGasLimit: '2',
      preVerificationGas: '3',
      maxFeePerGas: '4',
      maxPriorityFeePerGas: '5',
      paymasterAndData: '0xc0ffee',
    },
    chainId: 8453,
  },
];

for (const testCase of cases) {
  const op = testCase.op;
  const reference = core.attestSafeOpHash(JSON.stringify({
    sender: op.sender,
    nonce: op.nonce,
    init_code_hex: op.initCode,
    call_data_hex: op.callData,
    verification_gas_limit: op.verificationGasLimit,
    call_gas_limit: op.callGasLimit,
    pre_verification_gas: op.preVerificationGas,
    max_fee_per_gas: op.maxFeePerGas,
    max_priority_fee_per_gas: op.maxPriorityFeePerGas,
    paymaster_and_data_hex: op.paymasterAndData,
  }), '', BigInt(testCase.chainId));

  const mine = ours.safeop.hash(op, testCase.chainId);
  check(`SafeOp · ${testCase.name}`, hex(mine) === hex(reference), hex(reference).slice(0, 20) + '…');
}

// The chain id must actually be in the domain, or one signature works on every
// chain — the classic replay hole.
check('SafeOp · the chain id changes the digest',
  hex(ours.safeop.hash(cases[1].op, 1)) !== hex(ours.safeop.hash(cases[1].op, 100)));

// --- the message digest a Safe really verifies -------------------------------

const original = ours.digest.personalSignHash('hello');
check('SafeMessage · wraps the EIP-191 hash like the core does',
  hex(ours.safeop.messageHash(original, 100, SAFE)) ===
  hex(core.attestSafeMessageHash(original, 100n, SAFE)),
  hex(core.attestSafeMessageHash(original, 100n, SAFE)).slice(0, 20) + '…');

check('SafeMessage · differs from the bare EIP-191 hash',
  hex(ours.safeop.messageHash(original, 100, SAFE)) !== hex(original));

// --- reading the calldata back ----------------------------------------------

const single = ours.safeop.decodeCallData(executeUserOp(USDC, '0x0', transfer));
check('calldata · a single executeUserOp decodes back',
  !!single && single.kind === 'call' && single.calls.length === 1 &&
  ours.safeop.sameCall(single.calls[0], { to: USDC, value: '0x0', data: transfer }));

const packed = ours.encode.packMultiSend([
  { to: USDC, value: 0, data: transfer },
  { to: TO, value: 1000n, data: '0x' },
]);
const batchCallData = ours.encode.call('executeUserOp(address,uint256,bytes,uint8)', [
  ours.safeop.MULTI_SEND, 0n,
  ours.encode.call('multiSend(bytes)', [packed]),
  1,
]);
const batch = ours.safeop.decodeCallData(batchCallData);
check('calldata · a MultiSend delegatecall unpacks into its legs',
  !!batch && batch.kind === 'multiSend' && batch.calls.length === 2 &&
  ours.safeop.sameCall(batch.calls[0], { to: USDC, value: 0, data: transfer }) &&
  ours.safeop.sameCall(batch.calls[1], { to: TO, value: 1000n, data: '0x' }),
  batch ? `${batch.calls.length} legs` : 'nothing decoded');

check('calldata · foreign calldata is not mistaken for an operation',
  ours.safeop.decodeCallData(transfer) === null);

const passed = results.filter(Boolean).length;
console.log(`\n${passed}/${results.length} checks passed`);
process.exitCode = passed === results.length ? 0 : 1;
