import { hexToBytes, type Hex } from 'viem';
import type { SignRequestView, WebAuthnAssertion } from '@/lib/types';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const requestId = new URLSearchParams(location.search).get('request') ?? '';

function bytesToHex(buffer: ArrayBuffer | null): Hex | undefined {
  if (!buffer) return undefined;
  let result = '0x';
  for (const value of new Uint8Array(buffer)) result += value.toString(16).padStart(2, '0');
  return result as Hex;
}

function hexToBuffer(value: Hex): ArrayBuffer {
  return Uint8Array.from(hexToBytes(value)).buffer;
}

function showError(message: string) {
  $('loading').classList.add('hidden');
  $('details').classList.add('hidden');
  $('error').classList.remove('hidden');
  $('error-message').textContent = message;
}

async function sendResult(payload: Record<string, unknown>) {
  await chrome.runtime.sendMessage({ action: 'webauthn-result', requestId, ...payload });
}

async function load(): Promise<SignRequestView> {
  const response = await chrome.runtime.sendMessage({ action: 'webauthn-get-request', requestId });
  if (response?.error) throw new Error(response.error.message);
  return response as SignRequestView;
}

function render(view: SignRequestView) {
  $('network').textContent = `${view.chainName} (${view.chainId})`;
  $('safe').textContent = view.safeAddress;
  $('to').textContent = view.to;
  $('value').textContent = view.value;
  $('selector').textContent = view.dataSelector;
  $('nonce').textContent = view.nonce;
  $('rp').textContent = view.rpId;
  const warning = $('operation-warning');
  if (view.operation === 0) {
    warning.textContent = 'Standard Safe CALL transaction';
    warning.classList.add('normal');
  } else {
    warning.textContent = `Warning: Safe operation ${view.operation} (DELEGATECALL). Review the destination carefully.`;
  }
  $('loading').classList.add('hidden');
  $('details').classList.remove('hidden');
}

async function sign(view: SignRequestView) {
  const button = $('sign') as HTMLButtonElement;
  button.disabled = true;
  button.textContent = 'Waiting for passkey…';
  try {
    const publicKey: PublicKeyCredentialRequestOptions = {
      challenge: hexToBuffer(view.challengeHex),
      rpId: view.rpId,
      userVerification: 'required',
    };
    if (view.credentialId) {
      publicKey.allowCredentials = [{ type: 'public-key', id: hexToBuffer(view.credentialId as Hex) }];
    }
    const credential = await navigator.credentials.get({ publicKey }) as PublicKeyCredential | null;
    if (!credential) throw new Error('No passkey credential was returned.');
    const response = credential.response as AuthenticatorAssertionResponse;
    const assertion: WebAuthnAssertion = {
      credentialId: bytesToHex(credential.rawId)!,
      authenticatorDataHex: bytesToHex(response.authenticatorData)!,
      clientDataJSONHex: bytesToHex(response.clientDataJSON)!,
      signatureHex: bytesToHex(response.signature)!,
      ...(response.userHandle ? { userHandleHex: bytesToHex(response.userHandle) } : {}),
    };
    await sendResult({ assertion });
  } catch (error) {
    const domError = error as DOMException;
    const cancelled = domError?.name === 'NotAllowedError' || domError?.name === 'AbortError';
    await sendResult({ error: { code: cancelled ? 4001 : -32603, message: domError?.message || 'Passkey failed.' } });
    showError(cancelled ? 'The passkey request was cancelled.' : (domError?.message ?? 'Passkey failed.'));
  } finally {
    button.disabled = false;
    button.textContent = 'Verify and sign';
  }
}

$('close').addEventListener('click', () => window.close());
$('cancel').addEventListener('click', () => {
  void sendResult({ error: { code: 4001, message: 'User rejected the Safe transaction.' } });
});

load().then((view) => {
  render(view);
  $('sign').addEventListener('click', () => void sign(view));
}).catch((error) => showError(error?.message ?? 'This request is no longer active.'));
