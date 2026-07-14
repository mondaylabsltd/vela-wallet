import { MESSAGE_CHANNEL } from '@/lib/constants';
import { createRecoveryProvider, PROVIDER_INFO } from '@/lib/provider-factory';
import { installSafeOwnerCompatibility } from '@/lib/safe-compat-fetch';
import type { BridgeMessage } from '@/lib/types';

export default defineContentScript({
  matches: ['https://app.safe.global/*'],
  world: 'MAIN',
  runAt: 'document_start',

  main() {
    if (window !== window.top) return;
    // Safe memoizes smart-wallet classification. Install this before it can
    // cache the shared validator as an on-chain-only approveHash wallet.
    const compatibility = installSafeOwnerCompatibility(window);
    const { provider, handleMessage } = createRecoveryProvider((message, targetOrigin) => {
      window.postMessage(message, targetOrigin);
    }, {
      getSponsoredExecution: compatibility.isSponsoredExecutionSelected,
      armUnsignedProposal: compatibility.armUnsignedProposal,
      addLocalConfirmation: compatibility.addLocalConfirmation,
    });

    window.addEventListener('message', (event) => {
      if (event.source !== window || event.data?.channel !== MESSAGE_CHANNEL) return;
      const message = event.data as BridgeMessage;
      if (message.type === 'recovery-local-confirmations') {
        compatibility.setLocalConfirmations(message.confirmations);
      }
      handleMessage(message);
    });

    const detail = Object.freeze({ info: PROVIDER_INFO, provider });
    const announce = () => window.dispatchEvent(new CustomEvent('eip6963:announceProvider', { detail }));
    window.addEventListener('eip6963:requestProvider', announce);
    announce();

    const existing = (window as any).ethereum;
    if (!existing) {
      Object.defineProperty(window, 'ethereum', { value: provider, configurable: true });
    } else {
      try {
        if (!Array.isArray(existing.providers)) existing.providers = [existing];
        if (!existing.providers.includes(provider)) existing.providers.push(provider);
      } catch {
        // EIP-6963 remains the primary discovery path when another wallet locks window.ethereum.
      }
    }
    Object.defineProperty(window, 'velaSafeRecovery', { value: provider, configurable: false });
    window.postMessage({ type: 'recovery-provider-ready', channel: MESSAGE_CHANNEL }, '*');
  },
});
