import {
  encodeAbiParameters,
  encodeFunctionData,
  getAddress,
  numberToHex,
  size,
  toFunctionSelector,
  type Hex,
} from 'viem';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const rpcCallAt = vi.hoisted(() => vi.fn());
vi.mock('./rpc', () => ({ rpcCallAt }));

import { SHARED_WEBAUTHN_OWNER } from './constants';
import { EXEC_TRANSACTION_SELECTOR, relaySafeExecution, sendGasOnlyTransaction } from './relayer';
import { hashSafeTypedData, type SafeTypedData } from './signatures';

const PRIVATE_KEY = `0x${'11'.repeat(32)}` as const;
const TARGET = '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67';
const TX_HASH = `0x${'aa'.repeat(32)}`;
const BNB_SAFE = '0x6007462A7A3409DD8E23EED2C81Cb439cD95F4d4';
const BNB_DESTINATION = getAddress('0x9641d764fc13c8b624c04430c7356c1c7c8102e2');
const ZERO_ADDRESS = '0x0000000000000000000000000000000000000000';

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
    outputs: [{ name: 'success', type: 'bool' }],
  },
] as const;

function prevalidatedSharedOwnerSignature(): Hex {
  const ownerWord = SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase().padStart(64, '0');
  return `0x${ownerWord}${'0'.repeat(64)}01`;
}

function testContractSignature(dynamicOffset: number): Hex {
  const ownerWord = SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase().padStart(64, '0');
  const offsetWord = numberToHex(dynamicOffset, { size: 32 }).slice(2);
  const lengthWord = numberToHex(1, { size: 32 }).slice(2);
  return `0x${ownerWord}${offsetWord}00${lengthWord}aa`;
}

describe('local gas-only transaction sender', () => {
  beforeEach(() => {
    rpcCallAt.mockReset();
  });

  it('simulates, estimates, signs and broadcasts an EIP-1559 deployment call', async () => {
    rpcCallAt.mockImplementation(async (...args: unknown[]) => {
      const method = args[1];
      switch (method) {
        case 'eth_call': return '0x';
        case 'eth_getTransactionCount': return '0x2';
        case 'eth_estimateGas': return '0x186a0';
        case 'eth_getBalance': return '0xde0b6b3a7640000';
        case 'eth_getBlockByNumber': return { baseFeePerGas: '0x3b9aca00' };
        case 'eth_maxPriorityFeePerGas': return '0x59682f00';
        case 'eth_gasPrice': return '0x77359400';
        case 'eth_sendRawTransaction': return TX_HASH;
        default: throw new Error(`Unexpected RPC call ${JSON.stringify(args)}`);
      }
    });

    await expect(sendGasOnlyTransaction('https://rpc.example', 100, PRIVATE_KEY, {
      to: TARGET,
      data: '0x12345678',
    })).resolves.toBe(TX_HASH);

    const methods = rpcCallAt.mock.calls.map((call) => call[1]);
    expect(methods[0]).toBe('eth_call');
    expect(methods).toContain('eth_estimateGas');
    expect(methods.at(-1)).toBe('eth_sendRawTransaction');
    expect(rpcCallAt.mock.calls.at(-1)?.[2]?.[0]).toMatch(/^0x[0-9a-f]+$/i);
  });

  it('never broadcasts when the local gas account cannot cover the maximum cost', async () => {
    rpcCallAt.mockImplementation(async (...args: unknown[]) => {
      const method = args[1];
      switch (method) {
        case 'eth_call': return '0x';
        case 'eth_getTransactionCount': return '0x0';
        case 'eth_estimateGas': return '0x186a0';
        case 'eth_getBalance': return '0x0';
        case 'eth_getBlockByNumber': return {};
        case 'eth_maxPriorityFeePerGas': return '0x0';
        case 'eth_gasPrice': return '0x3b9aca00';
        default: throw new Error(`Unexpected RPC call ${JSON.stringify(args)}`);
      }
    });

    await expect(sendGasOnlyTransaction('https://rpc.example', 100, PRIVATE_KEY, {
      to: TARGET,
      data: '0x12345678',
    })).rejects.toThrow(/needs native gas/);
    expect(rpcCallAt.mock.calls.some((call) => call[1] === 'eth_sendRawTransaction')).toBe(false);
  });

  it('replaces the shared-owner placeholder and broadcasts a BNB Safe execution', async () => {
    const placeholder = prevalidatedSharedOwnerSignature();
    const calldata = encodeFunctionData({
      abi: EXEC_TRANSACTION_ABI,
      functionName: 'execTransaction',
      args: [
        BNB_DESTINATION,
        0n,
        '0x1234',
        0,
        0n,
        0n,
        0n,
        ZERO_ADDRESS,
        ZERO_ADDRESS,
        placeholder,
      ],
    });
    const typedData: SafeTypedData = {
      domain: { chainId: 56, verifyingContract: BNB_SAFE },
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
        to: BNB_DESTINATION,
        value: 0n,
        data: '0x1234',
        operation: 0,
        safeTxGas: 0n,
        baseGas: 0n,
        gasPrice: 0n,
        gasToken: ZERO_ADDRESS,
        refundReceiver: ZERO_ADDRESS,
        nonce: 0n,
      },
    };
    const safeTxHash = hashSafeTypedData(typedData);

    rpcCallAt.mockImplementation(async (...args: unknown[]) => {
      const method = args[1];
      if (method === 'eth_getCode') return '0x6000';
      if (method === 'eth_call') {
        const call = (args[2] as Array<{ data: Hex }>)[0]!;
        const selector = call.data.slice(0, 10).toLowerCase();
        if (selector === toFunctionSelector('getOwners()').toLowerCase()) {
          return encodeAbiParameters([{ type: 'address[]' }], [[SHARED_WEBAUTHN_OWNER]]);
        }
        if (selector === toFunctionSelector('getThreshold()').toLowerCase()) {
          return encodeAbiParameters([{ type: 'uint256' }], [1n]);
        }
        if (selector === toFunctionSelector('nonce()').toLowerCase()) {
          return encodeAbiParameters([{ type: 'uint256' }], [0n]);
        }
        if (selector === toFunctionSelector(
          'getTransactionHash(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)',
        ).toLowerCase()) {
          return encodeAbiParameters([{ type: 'bytes32' }], [safeTxHash]);
        }
        if (selector === EXEC_TRANSACTION_SELECTOR.toLowerCase()) return '0x';
        throw new Error(`Unexpected eth_call selector ${selector}`);
      }
      switch (method) {
        case 'eth_getTransactionCount': return '0x0';
        case 'eth_estimateGas': return '0x30d40';
        case 'eth_getBalance': return '0xde0b6b3a7640000';
        case 'eth_getBlockByNumber': return { baseFeePerGas: '0x0' };
        case 'eth_maxPriorityFeePerGas': return '0x0';
        case 'eth_gasPrice': return '0x3b9aca00';
        case 'eth_sendRawTransaction': return TX_HASH;
        default: throw new Error(`Unexpected RPC call ${JSON.stringify(args)}`);
      }
    });

    const requestPasskeySignature = vi.fn(async (_request: SafeTypedData, dynamicOffset: number) => {
      expect(_request).toEqual(typedData);
      expect(dynamicOffset).toBe(size(placeholder));
      return testContractSignature(dynamicOffset);
    });

    await expect(relaySafeExecution(
      'https://bsc-rpc.example',
      56,
      PRIVATE_KEY,
      {
        from: SHARED_WEBAUTHN_OWNER,
        to: BNB_SAFE,
        data: calldata,
        value: '0x0',
      },
      requestPasskeySignature,
    )).resolves.toBe(TX_HASH);

    expect(requestPasskeySignature).toHaveBeenCalledOnce();
    expect(rpcCallAt.mock.calls.at(-1)?.[1]).toBe('eth_sendRawTransaction');
  });
});
