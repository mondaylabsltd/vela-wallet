import { generatePrivateKey, privateKeyToAccount } from 'viem/accounts';
import { decodeFunctionResult, encodeFunctionData, getAddress, isAddress, type Hex } from 'viem';
import {
  DEFAULT_RP_ID,
  SAFE_ORIGINS,
  SHARED_WEBAUTHN_OWNER,
} from '@/lib/constants';
import { providerError, serializeError } from '@/lib/errors';
import { configuredNetwork, configuredNetworks } from '@/lib/networks';
import { isTransactionSubmissionMethod } from '@/lib/provider-methods';
import { relaySafeExecution } from '@/lib/relayer';
import { normalizeRpcUrl, permissionPatternForUrl, rpcCallAt, verifyRpcChain } from '@/lib/rpc';
import {
  buildSafeContractSignature,
  hashSafeTypedData,
  parseAndValidateSafeTypedData,
  safeContractSignaturePayload,
  signRequestView,
  validateAssertion,
  type SafeTypedData,
} from '@/lib/signatures';
import {
  bindCredentialIdToSafe,
  clearCredentialIdForSafe,
  credentialBindingKey,
  credentialIdForSafe,
  getSettings,
  localConfirmations as getLocalConfirmations,
  patchSettings,
  saveLocalConfirmation,
  saveSettings,
  toPublicState,
} from '@/lib/storage';
import type {
  Eip1193Request,
  LocalSafeConfirmation,
  RecoverySettings,
  SignRequestView,
  WebAuthnAssertion,
} from '@/lib/types';

interface PendingPasskeyRequest {
  view: SignRequestView;
  resolve: (assertion: WebAuthnAssertion) => void;
  reject: (error: Error) => void;
  windowId?: number;
  timer: ReturnType<typeof setTimeout>;
}

const pendingPasskeys = new Map<string, PendingPasskeyRequest>();
const relayQueues = new Map<number, Promise<unknown>>();

interface LocalConfirmationRpcResult {
  kind: 'local-confirmation';
  result: Hex;
  localConfirmation: LocalSafeConfirmation;
}

const IS_VALID_SIGNATURE_ABI = [
  {
    type: 'function',
    name: 'isValidSignature',
    stateMutability: 'view',
    inputs: [
      { name: 'message', type: 'bytes32' },
      { name: 'signature', type: 'bytes' },
    ],
    outputs: [{ name: 'magicValue', type: 'bytes4' }],
  },
] as const;

const ERC1271_MAGIC_VALUE = '0x1626ba7e';

const GET_THRESHOLD_ABI = [
  {
    type: 'function',
    name: 'getThreshold',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const;

function pageOrigin(sender: chrome.runtime.MessageSender): string | undefined {
  const raw = sender.origin ?? sender.url ?? sender.tab?.url;
  if (!raw) return undefined;
  try {
    return new URL(raw).origin;
  } catch {
    return undefined;
  }
}

function isSafePage(sender: chrome.runtime.MessageSender): boolean {
  const origin = pageOrigin(sender);
  return origin !== undefined && SAFE_ORIGINS.has(origin);
}

function isExtensionPage(sender: chrome.runtime.MessageSender): boolean {
  const raw = sender.url ?? sender.tab?.url ?? '';
  return raw.startsWith(chrome.runtime.getURL(''));
}

function rpcUrlFor(settings: RecoverySettings, chainId = settings.chainId): string {
  const url = settings.rpcUrls[String(chainId)];
  if (!url) throw providerError(4902, `No RPC configured for chain ${chainId}. Open the extension settings.`);
  return normalizeRpcUrl(url);
}

async function broadcastState(settings?: RecoverySettings): Promise<void> {
  const current = settings ?? await getSettings();
  const state = toPublicState(current);
  const tabs = await chrome.tabs.query({ url: 'https://app.safe.global/*' });
  await Promise.all(tabs.map((tab) => {
    if (tab.id === undefined) return Promise.resolve();
    return chrome.tabs.sendMessage(tab.id, { action: 'recovery-state-update', state }).catch(() => undefined);
  }));
}

async function broadcastLocalConfirmations(settings?: RecoverySettings): Promise<void> {
  const current = settings ?? await getSettings();
  const confirmations = getLocalConfirmations(current);
  const tabs = await chrome.tabs.query({ url: 'https://app.safe.global/*' });
  await Promise.all(tabs.map((tab) => {
    if (tab.id === undefined) return Promise.resolve();
    return chrome.tabs.sendMessage(tab.id, {
      action: 'recovery-local-confirmations-update',
      confirmations,
    }).catch(() => undefined);
  }));
}

async function switchActiveNetwork(chainId: number): Promise<RecoverySettings> {
  if (!Number.isSafeInteger(chainId) || chainId <= 0) {
    throw providerError(-32602, 'Invalid chain ID.');
  }
  const settings = await getSettings();
  const network = configuredNetwork(settings, chainId);
  if (!network) throw providerError(4902, `No RPC configured for chain ${chainId}.`);
  const origin = permissionPatternForUrl(network.rpcUrl);
  const hasPermission = await chrome.permissions.contains({ origins: [origin] });
  if (!hasPermission) {
    throw providerError(4100, `RPC access for ${network.chainName} is not granted. Save it again in Advanced settings.`);
  }
  await verifyRpcChain(network.rpcUrl, chainId);
  settings.chainId = chainId;
  await saveSettings(settings);
  await broadcastState(settings);
  return settings;
}

function normalizeRpId(input: string): string {
  let value = input.trim().toLowerCase();
  value = value.replace(/^[a-z][a-z0-9+.-]*:\/\//, '').replace(/\/.*$/, '').replace(/:\d+$/, '');
  if (!/^(?=.{1,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)*[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(value)) {
    throw providerError(-32602, 'RP ID must be a valid domain name.');
  }
  return value;
}

async function openPasskeyWindow(view: SignRequestView): Promise<WebAuthnAssertion> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      const pending = pendingPasskeys.get(view.requestId);
      if (!pending) return;
      pendingPasskeys.delete(view.requestId);
      if (pending.windowId !== undefined) void chrome.windows.remove(pending.windowId).catch(() => undefined);
      reject(providerError(4001, 'Passkey request timed out.'));
    }, 3 * 60_000);
    pendingPasskeys.set(view.requestId, { view, resolve, reject, timer });

    chrome.windows.create({
      url: chrome.runtime.getURL(`webauthn.html?request=${encodeURIComponent(view.requestId)}`),
      type: 'popup',
      width: 460,
      height: 690,
      focused: true,
    }).then((window) => {
      const pending = pendingPasskeys.get(view.requestId);
      if (!pending) return;
      if (window.id === undefined) {
        clearTimeout(pending.timer);
        pendingPasskeys.delete(view.requestId);
        reject(providerError(-32603, 'Could not open the passkey approval window.'));
        return;
      }
      pending.windowId = window.id;
    }).catch((error) => {
      const pending = pendingPasskeys.get(view.requestId);
      if (pending) clearTimeout(pending.timer);
      pendingPasskeys.delete(view.requestId);
      reject(providerError(-32603, error?.message ?? 'Could not open the passkey approval window.'));
    });
  });
}

async function signCanonicalSafeTypedData(
  typedData: SafeTypedData,
  settings: RecoverySettings,
  dynamicOffset = 65,
): Promise<Hex> {
  const challenge = hashSafeTypedData(typedData);
  const requestId = crypto.randomUUID();
  const safeAddress = getAddress(typedData.domain.verifyingContract!);
  const view = signRequestView(
    typedData,
    challenge,
    requestId,
    settings.rpId,
    credentialIdForSafe(settings, safeAddress),
    settings.chainNames[String(settings.chainId)] ?? `Chain ${settings.chainId}`,
  );
  const assertion = await openPasskeyWindow(view);
  const extensionOrigin = chrome.runtime.getURL('').replace(/\/$/, '');
  await validateAssertion(assertion, challenge, settings.rpId, extensionOrigin);
  const contractSignature = buildSafeContractSignature(assertion, dynamicOffset);
  const signerCall = encodeFunctionData({
    abi: IS_VALID_SIGNATURE_ABI,
    functionName: 'isValidSignature',
    args: [challenge, safeContractSignaturePayload(contractSignature)],
  });
  const signerResult = await rpcCallAt<Hex>(rpcUrlFor(settings), 'eth_call', [
    { from: view.safeAddress, to: SHARED_WEBAUTHN_OWNER, data: signerCall },
    'pending',
  ]);
  const magicValue = decodeFunctionResult({
    abi: IS_VALID_SIGNATURE_ABI,
    functionName: 'isValidSignature',
    data: signerResult,
  });
  if (magicValue.toLowerCase() !== ERC1271_MAGIC_VALUE) {
    const key = credentialBindingKey(safeAddress);
    const latest = await getSettings();
    if (latest.credentialIds[key] === assertion.credentialId) {
      const credentialIds = clearCredentialIdForSafe(latest.credentialIds, safeAddress);
      const next = await patchSettings({ credentialIds });
      void broadcastState(next);
    }
    throw providerError(4100, 'The passkey signature does not match this Safe\'s configured WebAuthn key.');
  }
  const latest = await getSettings();
  const next = await patchSettings({
    credentialIds: bindCredentialIdToSafe(latest.credentialIds, safeAddress, assertion.credentialId),
    lastSafeAddress: view.safeAddress,
  });
  void broadcastState(next);
  return contractSignature;
}

async function enqueueRelay<T>(chainId: number, task: () => Promise<T>): Promise<T> {
  const previous = relayQueues.get(chainId) ?? Promise.resolve();
  const current = previous.catch(() => undefined).then(task);
  relayQueues.set(chainId, current);
  try {
    return await current;
  } finally {
    if (relayQueues.get(chainId) === current) relayQueues.delete(chainId);
  }
}

function firstParam<T>(params: unknown): T | undefined {
  return Array.isArray(params) ? params[0] as T | undefined : undefined;
}

function safeTypedDataParam(params: unknown): unknown {
  if (!Array.isArray(params) || params.length < 2) {
    throw providerError(-32602, 'SafeTx signing parameters are missing.');
  }
  const owner = params.find((entry) =>
    typeof entry === 'string' && isAddress(entry) && getAddress(entry) === getAddress(SHARED_WEBAUTHN_OWNER));
  if (!owner) throw providerError(4100, 'SafeTx signing account is not the Vela shared signer.');
  const typedData = params.find((entry) => entry !== owner);
  if (typedData === undefined) throw providerError(-32602, 'SafeTx typed data is missing.');
  return typedData;
}

async function assertLocalConfirmationThresholdOne(typedData: SafeTypedData, settings: RecoverySettings): Promise<void> {
  const safeAddress = getAddress(typedData.domain.verifyingContract!);
  const data = encodeFunctionData({ abi: GET_THRESHOLD_ABI, functionName: 'getThreshold' });
  let result: Hex;
  try {
    result = await rpcCallAt<Hex>(rpcUrlFor(settings), 'eth_call', [{ to: safeAddress, data }, 'latest']);
  } catch {
    throw providerError(-32602, 'Local confirmation target is not a deployed Safe.');
  }
  const threshold = decodeFunctionResult({ abi: GET_THRESHOLD_ABI, functionName: 'getThreshold', data: result });
  if (threshold !== 1n) {
    throw providerError(4200, 'Local queued confirmations currently support threshold-1 Safes only.');
  }
}

async function saveSignedSafeTx(
  typedData: SafeTypedData,
  signature: Hex,
  settings: RecoverySettings,
): Promise<LocalConfirmationRpcResult> {
  const localConfirmation: LocalSafeConfirmation = {
    chainId: settings.chainId,
    safeAddress: getAddress(typedData.domain.verifyingContract!),
    safeTxHash: hashSafeTypedData(typedData),
    signature,
    submittedAt: Date.now(),
  };
  const next = await saveLocalConfirmation(localConfirmation);
  await broadcastLocalConfirmations(next);
  return { kind: 'local-confirmation', result: signature, localConfirmation };
}

function isLocalConfirmationResult(value: unknown): value is LocalConfirmationRpcResult {
  return Boolean(value && typeof value === 'object' && (value as LocalConfirmationRpcResult).kind === 'local-confirmation');
}

async function handleProviderRequest(request: Eip1193Request): Promise<unknown> {
  if (JSON.stringify(request).length > 1_000_000) throw providerError(-32602, 'Request is too large.');
  const settings = await getSettings();
  const { method, params } = request;

  if (method === 'eth_accounts') return settings.enabled ? [SHARED_WEBAUTHN_OWNER] : [];
  if (method === 'eth_requestAccounts') {
    if (!settings.enabled) {
      throw providerError(4100, 'Open Vela Wallet and enable Safe access first.');
    }
    return [SHARED_WEBAUTHN_OWNER];
  }
  if (method === 'eth_chainId') return `0x${settings.chainId.toString(16)}`;
  if (method === 'net_version') return String(settings.chainId);
  if (method === 'web3_clientVersion') return 'VelaWalletSafeIntegration/0.2.0';
  if (method === 'wallet_getPermissions') {
    return settings.enabled ? [{ parentCapability: 'eth_accounts', caveats: [] }] : [];
  }
  if (method === 'wallet_requestPermissions') {
    if (!settings.enabled) throw providerError(4100, 'Enable Safe access in Vela Wallet first.');
    return [{ parentCapability: 'eth_accounts', caveats: [] }];
  }
  if (method === 'wallet_revokePermissions') {
    const next = await patchSettings({ enabled: false });
    await broadcastState(next);
    return null;
  }

  if (!settings.enabled) throw providerError(4100, 'Vela Wallet Safe access is disabled.');

  if (method === 'wallet_switchEthereumChain') {
    const chain = firstParam<{ chainId?: string }>(params)?.chainId;
    const chainId = typeof chain === 'string' ? Number.parseInt(chain, 16) : 0;
    await switchActiveNetwork(chainId);
    return null;
  }

  if (method === 'wallet_addEthereumChain') {
    const chain = firstParam<{ chainId?: string; chainName?: string; rpcUrls?: string[] }>(params);
    const chainId = typeof chain?.chainId === 'string' ? Number.parseInt(chain.chainId, 16) : 0;
    const rpcUrl = chain?.rpcUrls?.[0];
    if (!Number.isSafeInteger(chainId) || chainId <= 0 || !rpcUrl) throw providerError(-32602, 'Invalid chain configuration.');
    const normalized = normalizeRpcUrl(rpcUrl);
    const hasPermission = await chrome.permissions.contains({ origins: [permissionPatternForUrl(normalized)] });
    if (!hasPermission) {
      throw providerError(4100, 'Open the extension and grant access to this chain RPC first.');
    }
    await verifyRpcChain(normalized, chainId);
    settings.rpcUrls[String(chainId)] = normalized;
    settings.chainNames[String(chainId)] = chain.chainName?.slice(0, 80) || `Chain ${chainId}`;
    settings.chainId = chainId;
    await saveSettings(settings);
    await broadcastState(settings);
    return null;
  }

  if (method === 'eth_signTypedData_v4' || method === 'eth_signTypedData') {
    const typedData = parseAndValidateSafeTypedData(safeTypedDataParam(params), settings.chainId);
    await assertLocalConfirmationThresholdOne(typedData, settings);
    const signature = await signCanonicalSafeTypedData(typedData, settings);
    return saveSignedSafeTx(typedData, signature, settings);
  }
  if (method === 'personal_sign' || method === 'eth_sign' || method === 'eth_signTypedData_v3') {
    throw providerError(4200, 'Vela Wallet signs canonical SafeTx EIP-712 requests only.');
  }
  // viem 2.52 can use the wallet namespace for the same JSON-RPC account
  // submission. Both methods must enter the identical, passkey-gated Safe
  // relay path; neither gives the page access to the local gas key.
  if (isTransactionSubmissionMethod(method)) {
    const transaction = firstParam<Record<string, any>>(params);
    if (!transaction) throw providerError(-32602, 'Missing transaction.');
    if (typeof transaction.from !== 'string' || !isAddress(transaction.from) || getAddress(transaction.from) !== getAddress(SHARED_WEBAUTHN_OWNER)) {
      throw providerError(4100, 'The Safe transaction must originate from the Vela shared signer identity.');
    }
    const rpcUrl = rpcUrlFor(settings);
    const txHash = await enqueueRelay(settings.chainId, () => relaySafeExecution(
      rpcUrl,
      settings.chainId,
      settings.relayerPrivateKey,
      transaction,
      (typedData, dynamicOffset) => signCanonicalSafeTypedData(typedData, settings, dynamicOffset),
    ));
    if (transaction.to && isAddress(transaction.to)) {
      const next = await patchSettings({ lastSafeAddress: getAddress(transaction.to) });
      void broadcastState(next);
    }
    return txHash;
  }
  if (method === 'eth_sendRawTransaction' || method.startsWith('wallet_')) {
    throw providerError(4200, `${method} is not supported by the Vela Wallet Safe integration.`);
  }

  return rpcCallAt(rpcUrlFor(settings), method, params ?? []);
}

async function popupState() {
  const settings = await getSettings();
  const state = toPublicState(settings);
  const networks = configuredNetworks(settings).map(({ chainId, chainName }) => ({ chainId, chainName }));
  const nativeSymbol = configuredNetwork(settings, settings.chainId)?.nativeSymbol ?? 'native';
  let balanceWei: string | undefined;
  let rpcError: string | undefined;
  try {
    balanceWei = await rpcCallAt<string>(rpcUrlFor(settings), 'eth_getBalance', [state.relayerAddress, 'pending']);
  } catch (error) {
    rpcError = (error as Error)?.message ?? 'RPC unavailable';
  }
  return { ...state, networks, nativeSymbol, balanceWei, rpcError };
}

async function handleExtensionAction(message: any): Promise<unknown> {
  if (message.action === 'popup-get-state') return popupState();
  if (message.action === 'popup-set-enabled') {
    const next = await patchSettings({ enabled: Boolean(message.enabled) });
    await broadcastState(next);
    return toPublicState(next);
  }
  if (message.action === 'popup-switch-network') {
    const next = await switchActiveNetwork(Number(message.chainId));
    return toPublicState(next);
  }
  if (message.action === 'popup-save-config') {
    const chainId = Number(message.chainId);
    if (!Number.isSafeInteger(chainId) || chainId <= 0) throw providerError(-32602, 'Chain ID is invalid.');
    const rpId = normalizeRpId(String(message.rpId || DEFAULT_RP_ID));
    const rpcUrl = normalizeRpcUrl(String(message.rpcUrl || ''));
    const origins = [permissionPatternForUrl(rpcUrl), `https://${rpId}/*`];
    const hasPermission = await chrome.permissions.contains({ origins });
    if (!hasPermission) throw providerError(4100, 'Required RPC or passkey host permission was not granted.');
    await verifyRpcChain(rpcUrl, chainId);
    const settings = await getSettings();
    settings.chainId = chainId;
    settings.rpId = rpId;
    settings.rpcUrls[String(chainId)] = rpcUrl;
    settings.chainNames[String(chainId)] = String(message.chainName || `Chain ${chainId}`).slice(0, 80);
    await saveSettings(settings);
    await broadcastState(settings);
    return toPublicState(settings);
  }
  if (message.action === 'popup-clear-credential') {
    const settings = await getSettings();
    const credentialIds = settings.lastSafeAddress
      ? clearCredentialIdForSafe(settings.credentialIds, settings.lastSafeAddress)
      : settings.credentialIds;
    const next = await patchSettings({ credentialIds, credentialId: undefined });
    await broadcastState(next);
    return toPublicState(next);
  }
  if (message.action === 'popup-export-relayer') {
    return { privateKey: (await getSettings()).relayerPrivateKey };
  }
  if (message.action === 'popup-import-relayer') {
    const privateKey = String(message.privateKey || '') as Hex;
    try {
      privateKeyToAccount(privateKey);
    } catch {
      throw providerError(-32602, 'Relayer private key must be a valid 32-byte hex key.');
    }
    const next = await patchSettings({ relayerPrivateKey: privateKey });
    await broadcastState(next);
    return toPublicState(next);
  }
  if (message.action === 'popup-rotate-relayer') {
    const next = await patchSettings({ relayerPrivateKey: generatePrivateKey() });
    await broadcastState(next);
    return toPublicState(next);
  }
  if (message.action === 'webauthn-get-request') {
    const pending = pendingPasskeys.get(String(message.requestId));
    if (!pending) throw providerError(4001, 'This passkey request is no longer active.');
    return pending.view;
  }
  if (message.action === 'webauthn-result') {
    const requestId = String(message.requestId);
    const pending = pendingPasskeys.get(requestId);
    if (!pending) return null;
    pendingPasskeys.delete(requestId);
    clearTimeout(pending.timer);
    if (message.error) pending.reject(providerError(message.error.code ?? 4001, message.error.message ?? 'Passkey request cancelled.'));
    else pending.resolve(message.assertion as WebAuthnAssertion);
    if (pending.windowId !== undefined) void chrome.windows.remove(pending.windowId).catch(() => undefined);
    return null;
  }
  throw providerError(4200, 'Unknown extension action.');
}

export default defineBackground(() => {
  const configureSidePanel = () => {
    if (!chrome.sidePanel?.setPanelBehavior) return;
    void chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true }).catch(() => undefined);
  };

  configureSidePanel();
  chrome.runtime.onInstalled.addListener(() => {
    configureSidePanel();
    void getSettings();
  });

  chrome.windows.onRemoved.addListener((windowId) => {
    for (const [requestId, pending] of pendingPasskeys) {
      if (pending.windowId !== windowId) continue;
      pendingPasskeys.delete(requestId);
      clearTimeout(pending.timer);
      pending.reject(providerError(4001, 'Passkey window was closed.'));
    }
  });

  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (
      message?.action === 'recovery-get-state' ||
      message?.action === 'recovery-get-local-confirmations' ||
      message?.action === 'recovery-rpc'
    ) {
      if (!isSafePage(sender)) {
        sendResponse({ error: serializeError(providerError(4100, 'Requests are accepted only from app.safe.global.')) });
        return false;
      }
      const promise = message.action === 'recovery-get-state'
        ? getSettings().then(toPublicState)
        : message.action === 'recovery-get-local-confirmations'
          ? getSettings().then(getLocalConfirmations)
          : handleProviderRequest(message.payload).then(
            (value) => isLocalConfirmationResult(value)
              ? { result: value.result, localConfirmation: value.localConfirmation }
              : { result: value },
            (error) => ({ error: serializeError(error) }),
          );
      promise.then(sendResponse, (error) => sendResponse({ error: serializeError(error) }));
      return true;
    }

    if (!isExtensionPage(sender)) {
      sendResponse({ error: serializeError(providerError(4100, 'Extension action denied.')) });
      return false;
    }
    handleExtensionAction(message).then(sendResponse, (error) => sendResponse({ error: serializeError(error) }));
    return true;
  });
});
