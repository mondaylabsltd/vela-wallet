import {
  bytesToHex,
  decodeFunctionData,
  decodeFunctionResult,
  encodeFunctionData,
  getAddress,
  hexToBytes,
  hexToBigInt,
  isAddress,
  isHex,
  size,
  toFunctionSelector,
  type Hex,
} from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { SHARED_WEBAUTHN_OWNER } from './constants';
import { providerError } from './errors';
import { rpcCallAt } from './rpc';
import { hashSafeTypedData, type SafeTypedData } from './signatures';

const EXEC_TRANSACTION_SIGNATURE =
  'execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)';
export const EXEC_TRANSACTION_SELECTOR = toFunctionSelector(EXEC_TRANSACTION_SIGNATURE);

const EXEC_TRANSACTION_ABI = [
  {
    type: 'function',
    name: 'execTransaction',
    stateMutability: 'payable',
    inputs: [
      { name: 'to', type: 'address' },
      { name: 'value', type: 'uint256' },
      { name: 'data', type: 'bytes' },
      { name: 'operation', type: 'uint8' },
      { name: 'safeTxGas', type: 'uint256' },
      { name: 'baseGas', type: 'uint256' },
      { name: 'gasPrice', type: 'uint256' },
      { name: 'gasToken', type: 'address' },
      { name: 'refundReceiver', type: 'address' },
      { name: 'signatures', type: 'bytes' },
    ],
    outputs: [],
  },
] as const;

const GET_OWNERS_ABI = [
  {
    type: 'function',
    name: 'getOwners',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'address[]' }],
  },
] as const;

const GET_THRESHOLD_ABI = [
  {
    type: 'function',
    name: 'getThreshold',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const;

const NONCE_ABI = [
  {
    type: 'function',
    name: 'nonce',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const;

const GET_TRANSACTION_HASH_ABI = [
  {
    type: 'function',
    name: 'getTransactionHash',
    stateMutability: 'view',
    inputs: [
      { name: 'to', type: 'address' },
      { name: 'value', type: 'uint256' },
      { name: 'data', type: 'bytes' },
      { name: 'operation', type: 'uint8' },
      { name: 'safeTxGas', type: 'uint256' },
      { name: 'baseGas', type: 'uint256' },
      { name: 'gasPrice', type: 'uint256' },
      { name: 'gasToken', type: 'address' },
      { name: 'refundReceiver', type: 'address' },
      { name: '_nonce', type: 'uint256' },
    ],
    outputs: [{ name: '', type: 'bytes32' }],
  },
] as const;

export interface OuterTransaction {
  from?: string;
  to?: string;
  data?: Hex;
  value?: Hex | string;
}

interface RecoverySafeMetadata {
  owners: string[];
  threshold: number;
}

type DecodedExecArgs = readonly [
  `0x${string}`,
  bigint,
  Hex,
  number,
  bigint,
  bigint,
  bigint,
  `0x${string}`,
  `0x${string}`,
  Hex,
];

export type RequestPasskeySignature = (typedData: SafeTypedData, dynamicOffset: number) => Promise<Hex>;

async function assertRecoverySafe(rpcUrl: string, safeAddress: string): Promise<RecoverySafeMetadata> {
  const code = await rpcCallAt<string>(rpcUrl, 'eth_getCode', [safeAddress, 'latest']);
  if (!code || code === '0x') throw providerError(-32602, 'Execution target is not a deployed Safe.');

  const callData = encodeFunctionData({ abi: GET_OWNERS_ABI, functionName: 'getOwners' });
  const result = await rpcCallAt<Hex>(rpcUrl, 'eth_call', [{ to: safeAddress, data: callData }, 'latest']);
  const owners = decodeFunctionResult({ abi: GET_OWNERS_ABI, functionName: 'getOwners', data: result });
  if (!owners.some((owner) => owner.toLowerCase() === SHARED_WEBAUTHN_OWNER.toLowerCase())) {
    throw providerError(4100, 'The target Safe is not owned by the Vela WebAuthn shared signer.');
  }

  const thresholdCallData = encodeFunctionData({ abi: GET_THRESHOLD_ABI, functionName: 'getThreshold' });
  const thresholdResult = await rpcCallAt<Hex>(rpcUrl, 'eth_call', [
    { to: safeAddress, data: thresholdCallData },
    'latest',
  ]);
  const threshold = Number(decodeFunctionResult({
    abi: GET_THRESHOLD_ABI,
    functionName: 'getThreshold',
    data: thresholdResult,
  }));
  if (!Number.isSafeInteger(threshold) || threshold <= 0) {
    throw providerError(-32603, 'Safe returned an invalid signature threshold.');
  }
  if (threshold > owners.length) {
    throw providerError(-32603, 'Safe returned a threshold larger than its owner set.');
  }
  return { owners: [...owners], threshold };
}

function assertSafeContractSignature(
  signatures: Hex,
  owners: readonly string[],
  threshold: number,
): void {
  if (!isHex(signatures)) throw providerError(-32602, 'Safe execTransaction has invalid signatures.');
  const bytes = hexToBytes(signatures);
  const staticLength = threshold * 65;
  if (bytes.length < staticLength) {
    throw providerError(4100, 'Safe execTransaction is missing required owner signatures.');
  }

  const owner = SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase();
  const ownerSet = new Set(owners.map((entry) => entry.toLowerCase()));
  let foundSharedContract = false;
  for (let index = 0; index < threshold; index++) {
    const start = index * 65;
    // Safe's contract-signature marker is v=0. The first 32-byte word (r)
    // stores the validator address, while s stores an offset to ABI data.
    if (bytes[start + 64] !== 0) continue;
    let signer = '';
    for (let byte = start + 12; byte < start + 32; byte++) {
      signer += bytes[byte]!.toString(16).padStart(2, '0');
    }
    if (!ownerSet.has(`0x${signer}`)) {
      throw providerError(4100, 'Safe execTransaction contains a contract signature from an unknown owner.');
    }
    if (signer === owner) foundSharedContract = true;

    // For v=0 Safe interprets s as an offset to an ABI-encoded bytes value.
    // Validate the bounds here so malformed calldata cannot reach the relayer
    // broadcast path (eth_call remains the canonical cryptographic check).
    const offset = hexToBigInt(bytesToHex(bytes.slice(start + 32, start + 64)));
    if (offset < BigInt(staticLength) || offset > BigInt(bytes.length - 32)) {
      throw providerError(-32602, 'Safe contract signature points outside the signature payload.');
    }
    const dynamicStart = Number(offset);
    const dynamicLength = hexToBigInt(bytesToHex(bytes.slice(dynamicStart, dynamicStart + 32)));
    if (dynamicLength === 0n || dynamicLength > BigInt(bytes.length - dynamicStart - 32)) {
      throw providerError(-32602, 'Safe contract signature has an invalid dynamic payload.');
    }
  }
  if (!foundSharedContract) {
    throw providerError(4100, 'Safe calldata does not include the Vela WebAuthn contract signature.');
  }
}

function sharedPrevalidatedSlot(signatures: Hex, threshold: number): number | undefined {
  const bytes = hexToBytes(signatures);
  if (bytes.length < threshold * 65) return undefined;
  const sharedOwner = SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase();
  let match: number | undefined;
  for (let index = 0; index < threshold; index++) {
    const start = index * 65;
    if (bytes[start + 64] !== 1) continue;
    const rOwner = bytesToHex(bytes.slice(start + 12, start + 32)).slice(2).toLowerCase();
    const s = bytes.slice(start + 32, start + 64);
    if (rOwner !== sharedOwner || s.some((byte) => byte !== 0)) continue;
    if (match !== undefined) throw providerError(-32602, 'Safe calldata repeats the Vela prevalidated signature.');
    match = index;
  }
  return match;
}

export function replaceSharedPrevalidatedSignature(
  signatures: Hex,
  threshold: number,
  contractSignature: Hex,
): Hex {
  const slot = sharedPrevalidatedSlot(signatures, threshold);
  if (slot === undefined) throw providerError(4100, 'Safe calldata has no Vela prevalidated signature to replace.');
  const original = hexToBytes(signatures);
  const replacement = hexToBytes(contractSignature);
  if (replacement.length <= 65 || replacement[64] !== 0) {
    throw providerError(-32602, 'Generated Vela contract signature is malformed.');
  }
  const expectedOwner = SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase();
  if (bytesToHex(replacement.slice(12, 32)).slice(2).toLowerCase() !== expectedOwner) {
    throw providerError(-32602, 'Generated contract signature has the wrong signer address.');
  }
  const offset = hexToBigInt(bytesToHex(replacement.slice(32, 64)));
  if (offset !== BigInt(original.length)) {
    throw providerError(-32602, 'Generated contract signature has the wrong dynamic offset.');
  }

  const merged = new Uint8Array(original.length + replacement.length - 65);
  merged.set(original);
  merged.set(replacement.slice(0, 65), slot * 65);
  merged.set(replacement.slice(65), original.length);
  return bytesToHex(merged);
}

function safeTypedDataFromExec(
  chainId: number,
  safeAddress: string,
  args: DecodedExecArgs,
  nonce: bigint,
): SafeTypedData {
  return {
    domain: { chainId, verifyingContract: safeAddress },
    types: {
      SafeTx: [
        { name: 'to', type: 'address' },
        { name: 'value', type: 'uint256' },
        { name: 'data', type: 'bytes' },
        { name: 'operation', type: 'uint8' },
        { name: 'safeTxGas', type: 'uint256' },
        { name: 'baseGas', type: 'uint256' },
        { name: 'gasPrice', type: 'uint256' },
        { name: 'gasToken', type: 'address' },
        { name: 'refundReceiver', type: 'address' },
        { name: 'nonce', type: 'uint256' },
      ],
    },
    primaryType: 'SafeTx',
    message: {
      to: args[0],
      value: args[1],
      data: args[2],
      operation: args[3],
      safeTxGas: args[4],
      baseGas: args[5],
      gasPrice: args[6],
      gasToken: args[7],
      refundReceiver: args[8],
      nonce,
    },
  };
}

async function replaceExecutionPlaceholder(
  rpcUrl: string,
  chainId: number,
  safeAddress: string,
  args: DecodedExecArgs,
  threshold: number,
  requestPasskeySignature: RequestPasskeySignature,
): Promise<Hex> {
  const nonceCall = encodeFunctionData({ abi: NONCE_ABI, functionName: 'nonce' });
  const nonceResult = await rpcCallAt<Hex>(rpcUrl, 'eth_call', [{ to: safeAddress, data: nonceCall }, 'pending']);
  const nonce = decodeFunctionResult({ abi: NONCE_ABI, functionName: 'nonce', data: nonceResult });
  const typedData = safeTypedDataFromExec(chainId, safeAddress, args, nonce);

  // Compare our EIP-712 reconstruction with the Safe itself before asking the
  // user to sign. This prevents a version or calldata mismatch from becoming a
  // passkey authorization for a different transaction.
  const hashCall = encodeFunctionData({
    abi: GET_TRANSACTION_HASH_ABI,
    functionName: 'getTransactionHash',
    args: [args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], nonce],
  });
  const onchainHashResult = await rpcCallAt<Hex>(rpcUrl, 'eth_call', [{ to: safeAddress, data: hashCall }, 'pending']);
  const onchainHash = decodeFunctionResult({
    abi: GET_TRANSACTION_HASH_ABI,
    functionName: 'getTransactionHash',
    data: onchainHashResult,
  });
  if (hashSafeTypedData(typedData).toLowerCase() !== onchainHash.toLowerCase()) {
    throw providerError(-32603, 'Safe transaction hash reconstruction did not match the contract.');
  }

  const signatures = args[9];
  const dynamicOffset = size(signatures);
  const contractSignature = await requestPasskeySignature(typedData, dynamicOffset);
  const merged = replaceSharedPrevalidatedSignature(signatures, threshold, contractSignature);
  return encodeFunctionData({
    abi: EXEC_TRANSACTION_ABI,
    functionName: 'execTransaction',
    args: [args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], merged],
  });
}

function parseQuantity(value: string | undefined, fallback = 0n): bigint {
  if (!value || value === '0x') return fallback;
  if (typeof value !== 'string') throw providerError(-32603, 'RPC returned an invalid quantity.');
  try {
    return BigInt(value);
  } catch {
    throw providerError(-32603, `RPC returned an invalid quantity: ${value}`);
  }
}

export async function sendGasOnlyTransaction(
  rpcUrl: string,
  chainId: number,
  privateKey: Hex,
  transaction: { to: string; data: Hex },
): Promise<Hex> {
  if (!isAddress(transaction.to) || !isHex(transaction.data)) {
    throw providerError(-32602, 'Gas transaction target or calldata is invalid.');
  }
  const to = getAddress(transaction.to);
  const account = privateKeyToAccount(privateKey);
  const call = { from: account.address, to, data: transaction.data, value: '0x0' };

  // Simulate the exact Safe call before the local gas account signs anything.
  await rpcCallAt(rpcUrl, 'eth_call', [call, 'pending']);

  const [nonceHex, estimateHex, balanceHex, pendingBlock, priorityHex, gasPriceHex] = await Promise.all([
    rpcCallAt<string>(rpcUrl, 'eth_getTransactionCount', [account.address, 'pending']),
    rpcCallAt<string>(rpcUrl, 'eth_estimateGas', [call]),
    rpcCallAt<string>(rpcUrl, 'eth_getBalance', [account.address, 'pending']),
    rpcCallAt<{ baseFeePerGas?: string }>(rpcUrl, 'eth_getBlockByNumber', ['pending', false]),
    rpcCallAt<string>(rpcUrl, 'eth_maxPriorityFeePerGas', []).catch(() => '0x0'),
    rpcCallAt<string>(rpcUrl, 'eth_gasPrice', []),
  ]);

  const nonce = parseQuantity(nonceHex);
  const estimatedGas = parseQuantity(estimateHex);
  const gas = (estimatedGas * 125n + 99n) / 100n;
  const balance = parseQuantity(balanceHex);
  const gasPrice = parseQuantity(gasPriceHex);
  const baseFee = parseQuantity(pendingBlock?.baseFeePerGas, 0n);
  const suggestedPriority = parseQuantity(priorityHex, 0n);

  let serialized: Hex;
  let maximumCost: bigint;
  if (baseFee > 0n) {
    const priority = suggestedPriority > 0n ? suggestedPriority : gasPrice > baseFee ? gasPrice - baseFee : 1_000_000_000n;
    const maxFeePerGas = baseFee * 2n + priority;
    maximumCost = gas * maxFeePerGas;
    serialized = await account.signTransaction({
      chainId,
      type: 'eip1559',
      to,
      data: transaction.data,
      value: 0n,
      nonce: Number(nonce),
      gas,
      maxFeePerGas,
      maxPriorityFeePerGas: priority,
    });
  } else {
    maximumCost = gas * gasPrice;
    serialized = await account.signTransaction({
      chainId,
      type: 'legacy',
      to,
      data: transaction.data,
      value: 0n,
      nonce: Number(nonce),
      gas,
      gasPrice,
    });
  }
  if (balance < maximumCost) {
    throw providerError(
      -32000,
      `Vela gas account ${account.address} needs native gas. Balance ${balance}, required up to ${maximumCost} wei.`,
    );
  }

  const txHash = await rpcCallAt<Hex>(rpcUrl, 'eth_sendRawTransaction', [serialized]);
  if (!/^0x[0-9a-fA-F]{64}$/.test(txHash)) throw providerError(-32603, 'RPC returned an invalid transaction hash.');
  return txHash;
}

export async function relaySafeExecution(
  rpcUrl: string,
  chainId: number,
  privateKey: Hex,
  transaction: OuterTransaction,
  requestPasskeySignature?: RequestPasskeySignature,
): Promise<Hex> {
  if (!transaction.to || !isAddress(transaction.to)) throw providerError(-32602, 'Transaction has no valid Safe target.');
  if (!transaction.data || !isHex(transaction.data) || !transaction.data.toLowerCase().startsWith(EXEC_TRANSACTION_SELECTOR.toLowerCase())) {
    throw providerError(-32602, 'Vela Wallet only submits Safe execTransaction calls.');
  }
  const value = parseQuantity(transaction.value, 0n);
  if (value !== 0n) throw providerError(-32602, 'Vela Wallet refuses outer transactions with value.');

  const safeAddress = getAddress(transaction.to);
  const safe = await assertRecoverySafe(rpcUrl, safeAddress);

  let decoded: ReturnType<typeof decodeFunctionData<typeof EXEC_TRANSACTION_ABI>>;
  try {
    decoded = decodeFunctionData({ abi: EXEC_TRANSACTION_ABI, data: transaction.data });
  } catch {
    throw providerError(-32602, 'Safe execTransaction calldata is malformed.');
  }
  const args = decoded.args as DecodedExecArgs;
  let signatures = args[9];
  if (!signatures) {
    throw providerError(-32602, 'Safe execTransaction has no signatures.');
  }
  const operation = Number(decoded.args[3]);
  if (operation !== 0 && operation !== 1) {
    throw providerError(-32602, 'Safe transaction operation must be CALL or DELEGATECALL.');
  }
  const placeholder = sharedPrevalidatedSlot(signatures, safe.threshold);
  if (placeholder !== undefined) {
    if (!requestPasskeySignature) {
      throw providerError(4100, 'Safe execution requires a Vela passkey signature.');
    }
    const replacedData = await replaceExecutionPlaceholder(
      rpcUrl,
      chainId,
      safeAddress,
      args,
      safe.threshold,
      requestPasskeySignature,
    );
    transaction = { ...transaction, data: replacedData };
    const replaced = decodeFunctionData({ abi: EXEC_TRANSACTION_ABI, data: replacedData });
    signatures = replaced.args[9];
  }
  assertSafeContractSignature(signatures, safe.owners, safe.threshold);

  return sendGasOnlyTransaction(rpcUrl, chainId, privateKey, {
    to: safeAddress,
    data: transaction.data!,
  });
}

export function relayerAddress(privateKey: Hex): string {
  return privateKeyToAccount(privateKey).address;
}

export function quantityToBigInt(value: Hex): bigint {
  return hexToBigInt(value);
}
