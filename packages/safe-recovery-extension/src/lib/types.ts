import type { Hex } from 'viem';

export interface RecoverySettings {
  enabled: boolean;
  rpId: string;
  chainId: number;
  rpcUrls: Record<string, string>;
  chainNames: Record<string, string>;
  credentialId?: string;
  relayerPrivateKey: Hex;
  lastSafeAddress?: string;
  localConfirmations: Record<string, LocalSafeConfirmation>;
}

export interface LocalSafeConfirmation {
  chainId: number;
  safeAddress: string;
  safeTxHash: Hex;
  signature: Hex;
  submittedAt: number;
}

export interface PublicRecoveryState {
  enabled: boolean;
  owner: string;
  rpId: string;
  chainId: number;
  chainName: string;
  rpcUrl: string;
  relayerAddress: string;
  credentialPinned: boolean;
  lastSafeAddress?: string;
}

export interface Eip1193Request {
  method: string;
  params?: unknown[] | Record<string, unknown>;
  context?: {
    sponsoredExecution?: boolean;
  };
}

export interface SerializedProviderError {
  code: number;
  message: string;
  data?: unknown;
}

export interface WebAuthnAssertion {
  credentialId: string;
  authenticatorDataHex: Hex;
  clientDataJSONHex: Hex;
  signatureHex: Hex;
  userHandleHex?: Hex;
}

export interface SignRequestView {
  requestId: string;
  rpId: string;
  challengeHex: Hex;
  credentialId?: string;
  chainId: number;
  chainName: string;
  safeAddress: string;
  to: string;
  value: string;
  operation: number;
  nonce: string;
  dataSelector: string;
}

export type BridgeMessage =
  | { type: 'recovery-request'; channel: string; id: string; payload: Eip1193Request }
  | {
      type: 'recovery-response';
      channel: string;
      id: string;
      method?: string;
      result?: unknown;
      error?: SerializedProviderError;
      localConfirmation?: LocalSafeConfirmation;
    }
  | { type: 'recovery-state'; channel: string; state: PublicRecoveryState }
  | { type: 'recovery-local-confirmations'; channel: string; confirmations: LocalSafeConfirmation[] }
  | { type: 'recovery-event'; channel: string; event: string; data: unknown }
  | { type: 'recovery-provider-ready'; channel: string };
