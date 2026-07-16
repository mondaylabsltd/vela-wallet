import { describe, expect, it, vi } from 'vitest';
import { SHARED_WEBAUTHN_OWNER } from './constants';
import {
  installSafeOwnerCompatibility,
  overlayLocalConfirmations,
  rewriteOwnerCodeResponse,
  unsignedLocalProposal,
} from './safe-compat-fetch';
import type { LocalSafeConfirmation } from './types';

function contractSignature(lastByte = '00'): string {
  const signer = SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase().padStart(64, '0');
  const offset = '41'.padStart(64, '0');
  const length = '20'.padStart(64, '0');
  return `0x${signer}${offset}00${length}${'00'.repeat(31)}${lastByte}`;
}

describe('Safe owner compatibility', () => {
  it('rewrites only the shared signer code lookup', () => {
    const request = [
      { jsonrpc: '2.0', id: 1, method: 'eth_getCode', params: [SHARED_WEBAUTHN_OWNER, 'latest'] },
      { jsonrpc: '2.0', id: 2, method: 'eth_getCode', params: ['0x0000000000000000000000000000000000001234', 'latest'] },
    ];
    expect(rewriteOwnerCodeResponse(request, [
      { jsonrpc: '2.0', id: 1, result: '0x6000' },
      { jsonrpc: '2.0', id: 2, result: '0x6000' },
    ])).toEqual([
      { jsonrpc: '2.0', id: 1, result: '0x' },
      { jsonrpc: '2.0', id: 2, result: '0x6000' },
    ]);
  });

  it('is active immediately, before recovery state is delivered', async () => {
    const nativeFetch = vi.fn(async () => new Response(JSON.stringify({ jsonrpc: '2.0', id: 7, result: '0x6000' }), {
      headers: { 'content-type': 'application/json' },
    }));
    const target = { fetch: nativeFetch } as unknown as Window;
    installSafeOwnerCompatibility(target);

    const response = await target.fetch('https://rpc.example', {
      method: 'POST',
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 7,
        method: 'eth_getCode',
        params: [SHARED_WEBAUTHN_OWNER, 'latest'],
      }),
    });

    await expect(response.json()).resolves.toEqual({ jsonrpc: '2.0', id: 7, result: '0x' });
    expect(nativeFetch).toHaveBeenCalledOnce();
  });

  it('handles the byte request body used by ethers v6', async () => {
    const nativeFetch = vi.fn(async () => new Response(JSON.stringify({ jsonrpc: '2.0', id: 8, result: '0x6000' }), {
      headers: { 'content-type': 'application/json' },
    }));
    const target = { fetch: nativeFetch } as unknown as Window;
    installSafeOwnerCompatibility(target);

    const payload = JSON.stringify({
      jsonrpc: '2.0',
      id: 8,
      method: 'eth_getCode',
      params: [SHARED_WEBAUTHN_OWNER, 'latest'],
    });
    const response = await target.fetch('https://rpc.example', {
      method: 'POST',
      body: new TextEncoder().encode(payload),
    });

    await expect(response.json()).resolves.toEqual({ jsonrpc: '2.0', id: 8, result: '0x' });
    expect(nativeFetch).toHaveBeenCalledOnce();
  });

  it('removes only the one-time shared-owner signature from a local proposal', async () => {
    const signed = contractSignature();
    const protocolKitAdjusted = contractSignature('1b');
    expect(unsignedLocalProposal({ safeTxHash: '0xabc', signature: protocolKitAdjusted }, signed)).toEqual({
      safeTxHash: '0xabc',
    });
    expect(unsignedLocalProposal({ safeTxHash: '0xabc', signature: protocolKitAdjusted }, '0x1234')).toBeUndefined();

    const nativeFetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => new Response(JSON.stringify({ txId: 'multisig_0xabc' }), {
      headers: { 'content-type': 'application/json' },
    }));
    const selectedRadio = { checked: true, disabled: false };
    const target = {
      fetch: nativeFetch,
      location: { href: 'https://app.safe.global/home' },
      document: {
        querySelector: () => ({ querySelector: () => selectedRadio }),
      },
    } as unknown as Window;
    const compatibility = installSafeOwnerCompatibility(target);
    compatibility.armUnsignedProposal(signed, true);

    await target.fetch(
      `https://safe-client.safe.global/v1/chains/100/transactions/${SHARED_WEBAUTHN_OWNER}/propose`,
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ safeTxHash: '0xabc', signature: protocolKitAdjusted }),
      },
    );

    const forwarded = nativeFetch.mock.calls[0]?.[1];
    expect(JSON.parse(String(forwarded?.body))).toEqual({ safeTxHash: '0xabc' });
  });

  it('does not arm a sponsored proposal when the relay option is not selected', async () => {
    const signed = contractSignature();
    const nativeFetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => new Response('{}'));
    const selectedRadio = { checked: false, disabled: false };
    const target = {
      fetch: nativeFetch,
      location: { href: 'https://app.safe.global/home' },
      document: {
        querySelector: () => ({ querySelector: () => selectedRadio }),
      },
    } as unknown as Window;
    const compatibility = installSafeOwnerCompatibility(target);
    compatibility.armUnsignedProposal(signed, true);

    const body = JSON.stringify({ safeTxHash: '0xabc', signature: signed });
    await target.fetch(
      `https://safe-client.safe.global/v1/chains/100/transactions/${SHARED_WEBAUTHN_OWNER}/propose`,
      { method: 'POST', body },
    );

    expect(nativeFetch.mock.calls[0]?.[1]?.body).toBe(body);
  });

  it('allows the standalone Sign action to create an unsigned queued proposal', async () => {
    const signed = contractSignature();
    const nativeFetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => new Response('{}'));
    const target = {
      fetch: nativeFetch,
      location: { href: 'https://app.safe.global/home' },
      document: { querySelector: () => null },
    } as unknown as Window;
    const compatibility = installSafeOwnerCompatibility(target);
    compatibility.armUnsignedProposal(signed, false);

    await target.fetch(
      `https://safe-client.safe.global/v1/chains/100/transactions/${SHARED_WEBAUTHN_OWNER}/propose`,
      { method: 'POST', body: JSON.stringify({ safeTxHash: '0xabc', signature: signed }) },
    );

    expect(JSON.parse(String(nativeFetch.mock.calls[0]?.[1]?.body))).toEqual({ safeTxHash: '0xabc' });
  });

  it('overlays a persisted confirmation into queue summaries and transaction details', () => {
    const safeAddress = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
    const safeTxHash = `0x${'ab'.repeat(32)}`;
    const local: LocalSafeConfirmation = {
      chainId: 100,
      safeAddress,
      safeTxHash: safeTxHash as `0x${string}`,
      signature: contractSignature() as `0x${string}`,
      submittedAt: 123456,
    };
    const queue = overlayLocalConfirmations({
      results: [{
        type: 'TRANSACTION',
        transaction: {
          id: `multisig_${safeAddress}_${safeTxHash}`,
          txStatus: 'AWAITING_CONFIRMATIONS',
          executionInfo: {
            type: 'MULTISIG',
            confirmationsRequired: 1,
            confirmationsSubmitted: 0,
            missingSigners: [{ value: SHARED_WEBAUTHN_OWNER }],
          },
        },
      }],
    }, { kind: 'queue', chainId: 100, safeAddress }, [local]);
    expect((queue.payload as any).results[0].transaction).toMatchObject({
      txStatus: 'AWAITING_EXECUTION',
      executionInfo: { confirmationsSubmitted: 1, missingSigners: [] },
    });

    const details = overlayLocalConfirmations({
      safeAddress,
      txStatus: 'AWAITING_CONFIRMATIONS',
      detailedExecutionInfo: {
        type: 'MULTISIG',
        safeTxHash,
        confirmationsRequired: 1,
        confirmations: [],
      },
    }, { kind: 'details', chainId: 100 }, [local]);
    expect(details.payload).toMatchObject({
      txStatus: 'AWAITING_EXECUTION',
      detailedExecutionInfo: {
        confirmations: [{
          signer: { value: SHARED_WEBAUTHN_OWNER },
          signature: local.signature,
          submittedAt: 123456,
        }],
      },
    });
  });

  it('loads the trusted queue plus only locally confirmed unsigned items', async () => {
    const safeAddress = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
    const localHash = `0x${'ab'.repeat(32)}`;
    const unrelatedHash = `0x${'cd'.repeat(32)}`;
    const trustedHash = `0x${'ef'.repeat(32)}`;
    const queueResponse = {
      results: [
        {
          type: 'TRANSACTION',
          transaction: {
            id: `multisig_${safeAddress}_${localHash}`,
            txStatus: 'AWAITING_CONFIRMATIONS',
            executionInfo: {
              type: 'MULTISIG', confirmationsRequired: 1, confirmationsSubmitted: 0,
              missingSigners: [{ value: SHARED_WEBAUTHN_OWNER }],
            },
          },
        },
        {
          type: 'TRANSACTION',
          transaction: {
            id: `multisig_${safeAddress}_${unrelatedHash}`,
            executionInfo: { type: 'MULTISIG', confirmationsRequired: 1, confirmationsSubmitted: 0 },
          },
        },
        {
          type: 'TRANSACTION',
          transaction: {
            id: `multisig_${safeAddress}_${trustedHash}`,
            executionInfo: { type: 'MULTISIG', confirmationsRequired: 1, confirmationsSubmitted: 1 },
          },
        },
      ],
    };
    const nativeFetch = vi.fn(async (_input: RequestInfo | URL) => new Response(JSON.stringify(queueResponse), {
      headers: { 'content-type': 'application/json' },
    }));
    const target = {
      fetch: nativeFetch,
      location: { href: 'https://app.safe.global/home' },
      document: { querySelector: () => null },
    } as unknown as Window;
    const compatibility = installSafeOwnerCompatibility(target);
    compatibility.setLocalConfirmations([{
      chainId: 100,
      safeAddress,
      safeTxHash: localHash as `0x${string}`,
      signature: contractSignature() as `0x${string}`,
      submittedAt: 123456,
    }]);

    const response = await target.fetch(
      `https://safe-client.safe.global/v1/chains/100/safes/${safeAddress}/transactions/queued`,
    );
    expect(String(nativeFetch.mock.calls[0]?.[0])).toContain('trusted=false');
    const json = await response.json() as any;
    expect(json.results.map((item: any) => item.transaction.id)).toEqual([
      `multisig_${safeAddress}_${localHash}`,
      `multisig_${safeAddress}_${trustedHash}`,
    ]);
    expect(json.results[0].transaction.executionInfo.confirmationsSubmitted).toBe(1);
  });
});
