import {
  MESSAGE_CHANNEL,
  PROVIDER_ICON,
  PROVIDER_NAME,
  PROVIDER_RDNS,
  PROVIDER_UUID,
} from './constants';
import { ProviderRpcError } from './errors';
import type { BridgeMessage, LocalSafeConfirmation, PublicRecoveryState } from './types';

type PostMessage = (message: BridgeMessage, targetOrigin: string) => void;

interface ProviderHooks {
  getSponsoredExecution?: () => boolean;
  armUnsignedProposal?: (signature: string, sponsoredExecution: boolean) => void;
  addLocalConfirmation?: (confirmation: LocalSafeConfirmation) => void;
}

export interface ProviderController {
  provider: Record<string, any>;
  handleMessage: (message: BridgeMessage) => void;
  state: { recovery?: PublicRecoveryState; connected: boolean };
}

export const PROVIDER_INFO = Object.freeze({
  uuid: PROVIDER_UUID,
  name: PROVIDER_NAME,
  icon: PROVIDER_ICON,
  rdns: PROVIDER_RDNS,
});

export function createRecoveryProvider(postMessage: PostMessage, hooks: ProviderHooks = {}): ProviderController {
  const pending = new Map<
    string,
    {
      method: string;
      sponsoredExecution: boolean;
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();
  const listeners = new Map<string, Set<(...args: any[]) => void>>();
  const state: ProviderController['state'] = { connected: false };
  let counter = 0;

  const emit = (event: string, ...args: any[]) => {
    for (const listener of listeners.get(event) ?? []) {
      try {
        listener(...args);
      } catch {
        // A dApp listener must not break provider state propagation.
      }
    }
  };

  const request = async ({ method, params }: { method: string; params?: unknown }): Promise<unknown> => {
    if (!method || typeof method !== 'string') throw new ProviderRpcError(-32600, 'Invalid EIP-1193 request.');
    const id = `vela-recovery-${Date.now()}-${++counter}`;
    const sponsoredExecution =
      (method === 'eth_signTypedData' || method === 'eth_signTypedData_v4') &&
      hooks.getSponsoredExecution?.() === true;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new ProviderRpcError(-32603, `${method} timed out.`));
      }, 3 * 60_000);
      pending.set(id, { method, sponsoredExecution, resolve, reject, timer });
      postMessage(
        {
          type: 'recovery-request',
          channel: MESSAGE_CHANNEL,
          id,
          payload: {
            method,
            params: params as any,
            context: sponsoredExecution ? { sponsoredExecution: true } : undefined,
          },
        },
        '*',
      );
    });
  };

  const provider: Record<string, any> = {
    isVelaSafeRecovery: true,
    request,
    enable: () => request({ method: 'eth_requestAccounts' }),
    isConnected: () => state.connected,
    on(event: string, listener: (...args: any[]) => void) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event)!.add(listener);
      return provider;
    },
    addListener(event: string, listener: (...args: any[]) => void) {
      return provider.on(event, listener);
    },
    removeListener(event: string, listener: (...args: any[]) => void) {
      listeners.get(event)?.delete(listener);
      return provider;
    },
    once(event: string, listener: (...args: any[]) => void) {
      const wrapped = (...args: any[]) => {
        provider.removeListener(event, wrapped);
        listener(...args);
      };
      return provider.on(event, wrapped);
    },
    removeAllListeners(event?: string) {
      if (event) listeners.delete(event);
      else listeners.clear();
      return provider;
    },
    send(methodOrPayload: string | { method: string; params?: unknown; id?: number }, paramsOrCallback?: unknown) {
      if (typeof methodOrPayload === 'string') return request({ method: methodOrPayload, params: paramsOrCallback });
      const id = methodOrPayload.id ?? ++counter;
      if (typeof paramsOrCallback === 'function') {
        request(methodOrPayload).then(
          (result) => (paramsOrCallback as Function)(null, { id, jsonrpc: '2.0', result }),
          (error) => (paramsOrCallback as Function)(error),
        );
        return;
      }
      return request(methodOrPayload);
    },
    sendAsync(payload: { method: string; params?: unknown; id?: number }, callback: Function) {
      const id = payload.id ?? ++counter;
      request(payload).then(
        (result) => callback(null, { id, jsonrpc: '2.0', result }),
        (error) => callback(error),
      );
    },
    selectedAddress: null as string | null,
    chainId: '0x1',
    networkVersion: '1',
  };

  const applyState = (next: PublicRecoveryState) => {
    const previous = state.recovery;
    state.recovery = next;
    const accounts = next.enabled ? [next.owner] : [];
    const chainId = `0x${next.chainId.toString(16)}`;
    const wasConnected = state.connected;
    state.connected = next.enabled;
    provider.selectedAddress = next.enabled ? next.owner : null;
    provider.chainId = chainId;
    provider.networkVersion = String(next.chainId);

    if (!previous) {
      if (next.enabled) emit('connect', { chainId });
      return;
    }
    if (previous.chainId !== next.chainId) emit('chainChanged', chainId);
    if (previous.enabled !== next.enabled) {
      emit('accountsChanged', accounts);
      if (next.enabled && !wasConnected) emit('connect', { chainId });
      if (!next.enabled && wasConnected) emit('disconnect', new ProviderRpcError(4900, 'Recovery provider disabled.'));
    }
  };

  const applyReturnedAccounts = (result: unknown, emitAccountsChanged: boolean) => {
    if (!Array.isArray(result) || !result.every((account) => typeof account === 'string')) return;
    const accounts = result as string[];
    const wasConnected = state.connected;
    state.connected = accounts.length > 0;
    provider.selectedAddress = accounts[0] ?? null;
    if (accounts.length > 0 && !wasConnected) emit('connect', { chainId: provider.chainId });
    if (emitAccountsChanged) emit('accountsChanged', accounts);
  };

  const handleMessage = (message: BridgeMessage) => {
    if (!message || message.channel !== MESSAGE_CHANNEL) return;
    if (message.type === 'recovery-response') {
      const item = pending.get(message.id);
      if (!item) return;
      pending.delete(message.id);
      clearTimeout(item.timer);
      if (message.error) item.reject(new ProviderRpcError(message.error.code, message.error.message, message.error.data));
      else {
        // Arm the one-time unsigned proposal rewrite before resolving the RPC
        // promise. Safe submits its proposal immediately after this resolves.
        if (message.localConfirmation) hooks.addLocalConfirmation?.(message.localConfirmation);
        if (
          (item.method === 'eth_signTypedData' || item.method === 'eth_signTypedData_v4') &&
          typeof message.result === 'string'
        ) {
          hooks.armUnsignedProposal?.(message.result, item.sponsoredExecution);
        }
        // The isolated content script and the MAIN-world provider can load in
        // either order. Do not leave dApps waiting for a state broadcast that
        // may have happened before their listener was installed: the account
        // response itself is authoritative for the connection handshake.
        if (item.method === 'eth_accounts' || item.method === 'eth_requestAccounts') {
          applyReturnedAccounts(message.result, item.method === 'eth_requestAccounts');
        }
        item.resolve(message.result);
      }
      return;
    }
    if (message.type === 'recovery-state') applyState(message.state);
    if (message.type === 'recovery-event') emit(message.event, message.data);
  };

  return { provider, handleMessage, state };
}
