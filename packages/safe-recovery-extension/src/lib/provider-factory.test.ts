import { describe, expect, it } from 'vitest';
import { MESSAGE_CHANNEL, SHARED_WEBAUTHN_OWNER } from './constants';
import { createRecoveryProvider } from './provider-factory';
import type { BridgeMessage } from './types';

describe('recovery provider connection handshake', () => {
  it('becomes connected from eth_requestAccounts even when no state broadcast arrived', async () => {
    const outbound: BridgeMessage[] = [];
    const controller = createRecoveryProvider((message) => outbound.push(message));
    const events: Array<[string, unknown]> = [];
    controller.provider.on('connect', (data: unknown) => events.push(['connect', data]));
    controller.provider.on('accountsChanged', (accounts: unknown) => events.push(['accountsChanged', accounts]));

    const request = controller.provider.request({ method: 'eth_requestAccounts' });
    const sent = outbound[0];
    expect(sent?.type).toBe('recovery-request');
    if (!sent || sent.type !== 'recovery-request') throw new Error('Missing recovery request');

    controller.handleMessage({
      type: 'recovery-response',
      channel: MESSAGE_CHANNEL,
      id: sent.id,
      method: 'eth_requestAccounts',
      result: [SHARED_WEBAUTHN_OWNER],
    });

    await expect(request).resolves.toEqual([SHARED_WEBAUTHN_OWNER]);
    expect(controller.provider.isConnected()).toBe(true);
    expect(controller.provider.selectedAddress).toBe(SHARED_WEBAUTHN_OWNER);
    expect(events).toEqual([
      ['connect', { chainId: '0x1' }],
      ['accountsChanged', [SHARED_WEBAUTHN_OWNER]],
    ]);
  });

  it('marks and arms only a sponsored Safe execution signature', async () => {
    const outbound: BridgeMessage[] = [];
    const armed: Array<[string, boolean]> = [];
    const controller = createRecoveryProvider((message) => outbound.push(message), {
      getSponsoredExecution: () => true,
      armUnsignedProposal: (signature, sponsored) => armed.push([signature, sponsored]),
    });

    const request = controller.provider.request({
      method: 'eth_signTypedData_v4',
      params: [SHARED_WEBAUTHN_OWNER, '{}'],
    });
    const sent = outbound[0];
    if (!sent || sent.type !== 'recovery-request') throw new Error('Missing recovery request');
    expect(sent.payload.context).toEqual({ sponsoredExecution: true });

    controller.handleMessage({
      type: 'recovery-response',
      channel: MESSAGE_CHANNEL,
      id: sent.id,
      method: 'eth_signTypedData_v4',
      result: '0x1234',
    });

    await expect(request).resolves.toBe('0x1234');
    expect(armed).toEqual([['0x1234', true]]);
  });
});
