import { generatePrivateKey, privateKeyToAccount } from 'viem/accounts';
import {
  DEFAULT_CHAIN_NAMES,
  DEFAULT_RP_ID,
  DEFAULT_RPC_URLS,
  SHARED_WEBAUTHN_OWNER,
  STORAGE_KEY,
} from './constants';
import type { LocalSafeConfirmation, PublicRecoveryState, RecoverySettings } from './types';

const MAX_LOCAL_CONFIRMATIONS = 100;

function defaults(): RecoverySettings {
  return {
    enabled: false,
    rpId: DEFAULT_RP_ID,
    chainId: 1,
    rpcUrls: Object.fromEntries(Object.entries(DEFAULT_RPC_URLS)),
    chainNames: Object.fromEntries(Object.entries(DEFAULT_CHAIN_NAMES)),
    relayerPrivateKey: generatePrivateKey(),
    localConfirmations: {},
  };
}

export async function getSettings(): Promise<RecoverySettings> {
  const stored = (await chrome.storage.local.get(STORAGE_KEY))[STORAGE_KEY] as Partial<RecoverySettings> | undefined;
  const base = defaults();
  const merged: RecoverySettings = {
    ...base,
    ...stored,
    rpcUrls: { ...base.rpcUrls, ...(stored?.rpcUrls ?? {}) },
    chainNames: { ...base.chainNames, ...(stored?.chainNames ?? {}) },
    localConfirmations: { ...(stored?.localConfirmations ?? {}) },
  };

  try {
    privateKeyToAccount(merged.relayerPrivateKey);
  } catch {
    merged.relayerPrivateKey = generatePrivateKey();
  }

  if (!stored || stored.relayerPrivateKey !== merged.relayerPrivateKey) {
    await saveSettings(merged);
  }
  return merged;
}

export async function saveSettings(settings: RecoverySettings): Promise<void> {
  await chrome.storage.local.set({ [STORAGE_KEY]: settings });
}

export async function patchSettings(patch: Partial<RecoverySettings>): Promise<RecoverySettings> {
  const current = await getSettings();
  const next = { ...current, ...patch };
  await saveSettings(next);
  return next;
}

export function localConfirmationKey(
  chainId: number,
  safeAddress: string,
  safeTxHash: string,
): string {
  return `${chainId}:${safeAddress.toLowerCase()}:${safeTxHash.toLowerCase()}`;
}

export function localConfirmations(settings: RecoverySettings): LocalSafeConfirmation[] {
  return Object.values(settings.localConfirmations).sort((left, right) => right.submittedAt - left.submittedAt);
}

export async function saveLocalConfirmation(
  confirmation: LocalSafeConfirmation,
): Promise<RecoverySettings> {
  const current = await getSettings();
  const key = localConfirmationKey(
    confirmation.chainId,
    confirmation.safeAddress,
    confirmation.safeTxHash,
  );
  const entries = Object.entries({
    ...current.localConfirmations,
    [key]: confirmation,
  }).sort(([, left], [, right]) => right.submittedAt - left.submittedAt).slice(0, MAX_LOCAL_CONFIRMATIONS);
  const next = { ...current, localConfirmations: Object.fromEntries(entries) };
  await saveSettings(next);
  return next;
}

export function toPublicState(settings: RecoverySettings): PublicRecoveryState {
  return {
    enabled: settings.enabled,
    owner: SHARED_WEBAUTHN_OWNER,
    rpId: settings.rpId,
    chainId: settings.chainId,
    chainName: settings.chainNames[String(settings.chainId)] ?? `Chain ${settings.chainId}`,
    rpcUrl: settings.rpcUrls[String(settings.chainId)] ?? '',
    relayerAddress: privateKeyToAccount(settings.relayerPrivateKey).address,
    credentialPinned: Boolean(settings.credentialId),
    lastSafeAddress: settings.lastSafeAddress,
  };
}
