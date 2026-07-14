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
    credentialIds: {},
    relayerPrivateKey: generatePrivateKey(),
    localConfirmations: {},
  };
}

export async function getSettings(): Promise<RecoverySettings> {
  const stored = (await chrome.storage.local.get(STORAGE_KEY))[STORAGE_KEY] as
    | (Partial<RecoverySettings> & { createdSafes?: unknown })
    | undefined;
  const base = defaults();
  const merged: RecoverySettings = {
    enabled: stored?.enabled ?? base.enabled,
    rpId: stored?.rpId ?? base.rpId,
    chainId: stored?.chainId ?? base.chainId,
    rpcUrls: { ...base.rpcUrls, ...(stored?.rpcUrls ?? {}) },
    chainNames: { ...base.chainNames, ...(stored?.chainNames ?? {}) },
    credentialIds: { ...(stored?.credentialIds ?? {}) },
    credentialId: stored?.credentialId,
    relayerPrivateKey: stored?.relayerPrivateKey ?? base.relayerPrivateKey,
    lastSafeAddress: stored?.lastSafeAddress,
    localConfirmations: { ...(stored?.localConfirmations ?? {}) },
  };
  let shouldSave = !stored || Boolean(stored && 'createdSafes' in stored);

  // Collapse the short-lived RP/chain/Safe binding format to a Safe-only key.
  // Prefer the binding for the last active configuration if several old keys
  // point at the same Safe address.
  const preferredOldKey = stored?.lastSafeAddress
    ? `${(stored.rpId ?? merged.rpId).toLowerCase()}:${stored.chainId ?? merged.chainId}:${stored.lastSafeAddress.toLowerCase()}`
    : undefined;
  const credentialEntries = Object.entries(merged.credentialIds);
  const safeOnlyEntries = credentialEntries.filter(([key]) => /^0x[0-9a-f]{40}$/i.test(key));
  const preferredEntries = credentialEntries.filter(([key]) =>
    key === preferredOldKey && !/^0x[0-9a-f]{40}$/i.test(key));
  const remainingEntries = credentialEntries.filter(([key]) =>
    !/^0x[0-9a-f]{40}$/i.test(key) && key !== preferredOldKey);
  const credentialIds: Record<string, string> = {};
  for (const [storedKey, credentialId] of [...safeOnlyEntries, ...preferredEntries, ...remainingEntries]) {
    const safeAddress = storedKey.match(/0x[0-9a-f]{40}$/i)?.[0];
    if (!safeAddress) {
      credentialIds[storedKey] = credentialId;
      continue;
    }
    const key = credentialBindingKey(safeAddress);
    credentialIds[key] ??= credentialId;
    if (key !== storedKey) shouldSave = true;
  }
  merged.credentialIds = credentialIds;

  // Earlier versions pinned one credential globally. Preserve that successful
  // association only for the last Safe, then remove the global pin so another
  // Safe can discover its own passkey.
  if (stored?.credentialId) {
    if (stored.lastSafeAddress) {
      const key = credentialBindingKey(stored.lastSafeAddress);
      merged.credentialIds[key] ??= stored.credentialId;
    }
    delete merged.credentialId;
    shouldSave = true;
  }

  try {
    privateKeyToAccount(merged.relayerPrivateKey);
  } catch {
    merged.relayerPrivateKey = generatePrivateKey();
    shouldSave = true;
  }

  if (shouldSave) await saveSettings(merged);
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

export function credentialBindingKey(safeAddress: string): string {
  return safeAddress.toLowerCase();
}

export function credentialIdForSafe(
  settings: RecoverySettings,
  safeAddress: string,
): string | undefined {
  return settings.credentialIds[credentialBindingKey(safeAddress)];
}

export function bindCredentialIdToSafe(
  credentialIds: Record<string, string>,
  safeAddress: string,
  credentialId: string,
): Record<string, string> {
  return { ...credentialIds, [credentialBindingKey(safeAddress)]: credentialId };
}

export function clearCredentialIdForSafe(
  credentialIds: Record<string, string>,
  safeAddress: string,
): Record<string, string> {
  const next = { ...credentialIds };
  delete next[credentialBindingKey(safeAddress)];
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
  const credentialPinned = settings.lastSafeAddress
    ? Boolean(credentialIdForSafe(settings, settings.lastSafeAddress))
    : false;
  return {
    enabled: settings.enabled,
    owner: SHARED_WEBAUTHN_OWNER,
    rpId: settings.rpId,
    chainId: settings.chainId,
    chainName: settings.chainNames[String(settings.chainId)] ?? `Chain ${settings.chainId}`,
    rpcUrl: settings.rpcUrls[String(settings.chainId)] ?? '',
    relayerAddress: privateKeyToAccount(settings.relayerPrivateKey).address,
    credentialPinned,
    lastSafeAddress: settings.lastSafeAddress,
  };
}
