import { MESSAGE_CHANNEL } from '@/lib/constants';
import type { BridgeMessage } from '@/lib/types';

export default defineContentScript({
  matches: ['https://app.safe.global/*'],
  runAt: 'document_start',

  main() {
    if (window !== window.top) return;

    const post = (message: BridgeMessage) => window.postMessage(message, '*');

    const loadState = async () => {
      try {
        const [state, confirmations] = await Promise.all([
          chrome.runtime.sendMessage({ action: 'recovery-get-state' }),
          chrome.runtime.sendMessage({ action: 'recovery-get-local-confirmations' }),
        ]);
        if (state) post({ type: 'recovery-state', channel: MESSAGE_CHANNEL, state });
        if (Array.isArray(confirmations)) {
          post({
            type: 'recovery-local-confirmations',
            channel: MESSAGE_CHANNEL,
            confirmations,
          });
        }
      } catch {
        // Extension reloads invalidate old content scripts; a page refresh reconnects it.
      }
    };

    window.addEventListener('message', (event) => {
      if (event.source !== window || event.data?.channel !== MESSAGE_CHANNEL) return;
      const message = event.data as BridgeMessage;
      if (message.type === 'recovery-provider-ready') {
        void loadState();
        return;
      }
      if (message.type !== 'recovery-request') return;

      chrome.runtime.sendMessage({ action: 'recovery-rpc', id: message.id, payload: message.payload }).then(
        (response) => post({
          type: 'recovery-response',
          channel: MESSAGE_CHANNEL,
          id: message.id,
          method: message.payload.method,
          ...response,
        }),
        (error) => post({
          type: 'recovery-response',
          channel: MESSAGE_CHANNEL,
          id: message.id,
          error: { code: -32603, message: error?.message ?? 'Extension is unavailable.' },
        }),
      );
    });

    chrome.runtime.onMessage.addListener((message) => {
      if (message?.action === 'recovery-state-update') {
        post({ type: 'recovery-state', channel: MESSAGE_CHANNEL, state: message.state });
      }
      if (message?.action === 'recovery-provider-event') {
        post({ type: 'recovery-event', channel: MESSAGE_CHANNEL, event: message.event, data: message.data });
      }
      if (message?.action === 'recovery-local-confirmations-update' && Array.isArray(message.confirmations)) {
        post({
          type: 'recovery-local-confirmations',
          channel: MESSAGE_CHANNEL,
          confirmations: message.confirmations,
        });
      }
    });

    // MAIN-world and isolated-world content scripts have no deterministic load
    // order. Request state proactively as well as responding to the ready
    // event, so the injected provider cannot miss its initial state broadcast.
    void loadState();
  },
});
