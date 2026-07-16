import { afterEach, describe, expect, it, vi } from 'vitest';
import { rpcCallAt } from './rpc';

describe('RPC error normalization', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('does not misreport a remote 4200 as an unsupported injected-provider method', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      error: { code: 4200, message: 'upstream rejected transaction', data: { reason: 'test' } },
    }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    })));

    await expect(rpcCallAt('https://rpc.example', 'eth_sendRawTransaction', ['0x01'])).rejects.toMatchObject({
      code: -32603,
      message: 'RPC eth_sendRawTransaction failed: upstream rejected transaction',
      data: { rpcCode: 4200, rpcData: { reason: 'test' } },
    });
  });
});
