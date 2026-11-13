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
const UINT48_MAX = (1n << 48n) - 1n;

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
      // §35: what the core stamps — no lower bound, now + 30 minutes.
      validAfter: '0',
      validUntil: '1800001800',
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
      validAfter: '1700000000',
      validUntil: '1700001800',
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
      // The widest window a uint48 holds, both ends.
      validAfter: String(UINT48_MAX - 1n),
      validUntil: String(UINT48_MAX),
    },
    chainId: 8453,
  },
];

// The core's own reading of an operation (`AttestOp`): the window as numbers.
// Every uint48 is below 2^53, so a JSON number carries it exactly.
function attestJson(op) {
  return JSON.stringify({
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
    valid_after: Number(op.validAfter || 0),
    valid_until: Number(op.validUntil || 0),
  });
}
const coreHash = (op, chainId) => hex(core.attestSafeOpHash(attestJson(op), '', BigInt(chainId)));
const coreRefuses = (op, chainId) => {
  try { core.attestSafeOpHash(attestJson(op), '', BigInt(chainId)); return false; } catch { return true; }
};

for (const testCase of cases) {
  const reference = coreHash(testCase.op, testCase.chainId);
  const mine = ours.safeop.hash(testCase.op, testCase.chainId);
  check(`SafeOp · ${testCase.name}`, hex(mine) === reference, reference.slice(0, 20) + '…');
}

// The chain id must actually be in the domain, or one signature works on every
// chain — the classic replay hole.
check('SafeOp · the chain id changes the digest',
  hex(ours.safeop.hash(cases[1].op, 1)) !== hex(ours.safeop.hash(cases[1].op, 100)));

// --- §35: the validity window ------------------------------------------------
//
// The digest covers the operation's own validAfter / validUntil, exactly as
// the Safe4337Module reads them from the signature. Every window here is one
// the core hashes too, and the two must agree byte for byte.

const base = cases[1].op;
const windows = [
  ['0', '1800001800', 'the core’s stamp (now + 30 min)'],
  ['0', '1', 'the smallest expiry'],
  ['0', '1000000000', 'a second long past'],
  ['1800000000', '1800000000', 'a window that opens as it closes'],
  ['0', '4102444800', 'the year 2100'],
  ['0', String(2n ** 32n), 'past uint32'],
  ['0', String(UINT48_MAX), 'uint48 max'],
  [String(UINT48_MAX), String(UINT48_MAX), 'uint48 max, both ends'],
  ['0x0', '0x6b49d208', 'hex strings, as the core also reads them'],
  [0, 1800001800, 'JSON numbers'],
];
for (const [after, until, name] of windows) {
  const op = { ...base, validAfter: after, validUntil: until };
  const reference = coreHash({ ...op, validAfter: BigInt(after).toString(), validUntil: BigInt(until).toString() }, 1);
  check(`window · ${name} (${after}..${until}) matches vela-core`,
    hex(ours.safeop.hash(op, 1)) === reference, reference.slice(0, 20) + '…');
}

const stamped = { ...base, validAfter: '0', validUntil: '1800001800' };
check('window · validUntil is in the digest (one second later is another hash)',
  hex(ours.safeop.hash(stamped, 1)) !== hex(ours.safeop.hash({ ...stamped, validUntil: '1800001801' }, 1)));
check('window · validAfter is in the digest',
  hex(ours.safeop.hash(stamped, 1)) !== hex(ours.safeop.hash({ ...stamped, validAfter: '1' }, 1)));
check('window · the core’s stamp is what userOpValidityWindow gives at a fixed now',
  Array.from(core.userOpValidityWindow(1_800_000_000_000)).join('..') === '0..1800001800');

// The core's request to the page (`trustedSignerSignatureRequest`, the very
// JSON every shell hands over): the page reads ITS window, and the page's
// digest of that request is the core's SafeOp hash.
const requestOf = (op) => JSON.parse(core.trustedSignerSignatureRequest(JSON.stringify({
  method: '', chainId: 1, account: SAFE, userOp: op, calls: [{ to: USDC, value: '0x0', data: transfer }],
})));
for (const [after, until] of [['0', '1800001800'], ['0', String(UINT48_MAX)], ['1700000000', '1700001800']]) {
  const request = requestOf({ ...base, validAfter: after, validUntil: until });
  const carried = request.context.operation.userOp;
  const digest = ours.digest.of(request.intent, request.context);
  check(`request · the core sends ${after}..${until} as decimal strings, and the page signs the core’s hash`,
    carried.validAfter === after && carried.validUntil === until && !!digest.hash &&
    hex(digest.hash) === coreHash({ ...base, validAfter: after, validUntil: until }, 1),
    digest.refuse || (digest.hash && hex(digest.hash).slice(0, 20) + '…'));
}

// No expiry: the core will not sign it, and the page will not either.
const txIntent = { method: 'eth_sendTransaction', params: [{ to: USDC, value: '0x0', data: transfer }] };
const digestOf = (userOp) => ours.digest.of(txIntent, { chainId: 1, operation: { userOp } });
const { validAfter: _a, validUntil: _u, ...unstamped } = base;
check('no expiry · an operation without validUntil is refused refuse.noExpiry (the core refuses it too)',
  digestOf(unstamped).refuse === 'refuse.noExpiry' && !digestOf(unstamped).hash && coreRefuses(unstamped, 1),
  JSON.stringify(digestOf(unstamped)));
check('no expiry · validUntil "0" is refused refuse.noExpiry',
  digestOf({ ...base, validAfter: '0', validUntil: '0' }).refuse === 'refuse.noExpiry');
check('no expiry · validUntil null, validAfter set, is refused refuse.noExpiry',
  digestOf({ ...base, validAfter: '5', validUntil: null }).refuse === 'refuse.noExpiry');
const requestUnstamped = requestOf({ ...base, validAfter: '0', validUntil: '0' });
check('no expiry · the core’s own request for an unstamped operation is refused by the page',
  ours.digest.of(requestUnstamped.intent, requestUnstamped.context).refuse === 'refuse.noExpiry');

// A window that does not read is no window: never guessed, never signed.
const hostile = [
  [String(UINT48_MAX + 1n), 'validUntil one past uint48 max'],
  ['18446744073709551616', 'validUntil past u64'],
  ['-1', 'a negative validUntil'],
  [-1, 'a negative JSON number'],
  ['soon', 'a word'],
  ['1800001800s', 'digits with a suffix'],
  [' 1800001800', 'leading space (BigInt would read it)'],
  ['1e9', 'exponent notation'],
  [1.5, 'a fraction'],
  [true, 'true (BigInt would read it as 1)'],
  [[1800001800], 'an array (BigInt would read it)'],
  [{}, 'an object'],
  ['', 'an empty string (the core does not read it)'],
  ['0x', '“0x” then nothing… read as zero → no expiry'],
];
for (const [until, name] of hostile) {
  const result = digestOf({ ...base, validAfter: '0', validUntil: until });
  const expect = name.includes('no expiry') ? 'refuse.noExpiry' : 'refuse.badWindow';
  check(`hostile window · ${name} → ${expect}, no digest`,
    result.refuse === expect && !result.hash, JSON.stringify(result));
}
check('hostile window · validAfter past uint48 max → refuse.badWindow',
  digestOf({ ...base, validAfter: String(UINT48_MAX + 1n), validUntil: '1800001800' }).refuse === 'refuse.badWindow');
check('hostile window · a window that opens after it closes → refuse.badWindow (the core refuses it too)',
  digestOf({ ...base, validAfter: '1800001801', validUntil: '1800001800' }).refuse === 'refuse.badWindow' &&
  coreRefuses({ ...base, validAfter: '1800001801', validUntil: '1800001800' }, 1));
check('hostile window · the core refuses validUntil past uint48 too',
  coreRefuses({ ...base, validAfter: '0', validUntil: String(UINT48_MAX + 1n) }, 1));
let thrown = false;
try { ours.safeop.hash({ ...base, validUntil: '-1' }, 1); } catch { thrown = true; }
check('hostile window · safeop.hash itself throws rather than hash a window it cannot read', thrown);

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
