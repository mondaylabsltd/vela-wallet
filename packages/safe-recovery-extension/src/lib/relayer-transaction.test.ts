import { beforeEach, describe, expect, it, vi } from 'vitest';

const rpcCallAt = vi.hoisted(() => vi.fn());
vi.mock('./rpc', () => ({ rpcCallAt }));

import { sendGasOnlyTransaction } from './relayer';

const PRIVATE_KEY = `0x${'11'.repeat(32)}` as const;
const TARGET = '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67';
const TX_HASH = `0x${'aa'.repeat(32)}`;

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
});
