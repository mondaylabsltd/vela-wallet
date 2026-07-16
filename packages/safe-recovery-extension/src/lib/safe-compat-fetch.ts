import { SHARED_WEBAUTHN_OWNER } from './constants';
import type { LocalSafeConfirmation } from './types';

const LOCAL_PROPOSAL_TTL_MS = 60_000;

interface JsonRpcPayload {
  id?: string | number | null;
  method?: string;
  params?: unknown[];
}

interface JsonRpcResponse {
  id?: string | number | null;
  result?: unknown;
  [key: string]: unknown;
}

interface TransactionProposal {
  signature?: unknown;
  [key: string]: unknown;
}

interface ArmedLocalProposal {
  signature: string;
  sponsoredExecution: boolean;
  expiresAt: number;
}

export interface SafeOwnerCompatibility {
  isSponsoredExecutionSelected: () => boolean;
  armUnsignedProposal: (signature: string, sponsoredExecution: boolean) => void;
  setLocalConfirmations: (confirmations: LocalSafeConfirmation[]) => void;
  addLocalConfirmation: (confirmation: LocalSafeConfirmation) => void;
}

type SafeClientResponseContext =
  | { kind: 'queue'; chainId: number; safeAddress: string; includeLocalUntrusted?: boolean }
  | { kind: 'details' | 'proposal'; chainId: number; safeAddress?: string };

function isOwnerCodeRequest(payload: JsonRpcPayload): boolean {
  return payload.method === 'eth_getCode' &&
    typeof payload.params?.[0] === 'string' &&
    payload.params[0].toLowerCase() === SHARED_WEBAUTHN_OWNER.toLowerCase();
}

export function rewriteOwnerCodeResponse(request: unknown, response: unknown): unknown {
  const requests = Array.isArray(request) ? request as JsonRpcPayload[] : [request as JsonRpcPayload];
  const ownerRequestIds = new Set(
    requests.filter(isOwnerCodeRequest).map((entry) => String(entry.id ?? '')),
  );
  if (ownerRequestIds.size === 0) return response;

  const rewrite = (entry: JsonRpcResponse): JsonRpcResponse => {
    if (!ownerRequestIds.has(String(entry?.id ?? '')) || typeof entry?.result !== 'string') return entry;
    return { ...entry, result: '0x' };
  };
  return Array.isArray(response) ? (response as JsonRpcResponse[]).map(rewrite) : rewrite(response as JsonRpcResponse);
}

async function requestBody(input: RequestInfo | URL, init?: RequestInit): Promise<string | undefined> {
  const body = init?.body;
  if (typeof body === 'string') return body;
  // ethers v6's browser transport encodes JSON-RPC request bodies as bytes.
  // Safe uses that transport for its read-only smart-wallet classification.
  if (body instanceof ArrayBuffer) return new TextDecoder().decode(body);
  if (ArrayBuffer.isView(body)) {
    return new TextDecoder().decode(new Uint8Array(body.buffer, body.byteOffset, body.byteLength));
  }
  if (typeof Blob !== 'undefined' && body instanceof Blob) return body.text();
  if (typeof URLSearchParams !== 'undefined' && body instanceof URLSearchParams) return body.toString();
  if (typeof Request !== 'undefined' && input instanceof Request) {
    try {
      return await input.clone().text();
    } catch {
      return undefined;
    }
  }
  return undefined;
}

function requestUrl(input: RequestInfo | URL, target: Window): URL | undefined {
  const raw = typeof input === 'string'
    ? input
    : input instanceof URL
      ? input.href
      : input.url;
  try {
    return new URL(raw, target.location?.href ?? 'https://app.safe.global');
  } catch {
    return undefined;
  }
}

function isSafeProposalRequest(input: RequestInfo | URL, target: Window): boolean {
  const url = requestUrl(input, target);
  return url?.protocol === 'https:' &&
    url.hostname === 'safe-client.safe.global' &&
    /^\/v1\/chains\/\d+\/transactions\/0x[a-fA-F0-9]{40}\/propose\/?$/.test(url.pathname);
}

function safeClientResponseContext(
  input: RequestInfo | URL,
  target: Window,
): SafeClientResponseContext | undefined {
  const url = requestUrl(input, target);
  if (url?.protocol !== 'https:' || url.hostname !== 'safe-client.safe.global') return undefined;

  let match = url.pathname.match(
    /^\/v1\/chains\/(\d+)\/safes\/(0x[a-fA-F0-9]{40})\/transactions\/queued\/?$/,
  );
  if (match) {
    return {
      kind: 'queue',
      chainId: Number(match[1]),
      safeAddress: match[2]!,
      includeLocalUntrusted: url.searchParams.get('trusted') !== 'false',
    };
  }

  match = url.pathname.match(
    /^\/v1\/chains\/(\d+)\/transactions\/(0x[a-fA-F0-9]{40})\/propose\/?$/,
  );
  if (match) return { kind: 'proposal', chainId: Number(match[1]), safeAddress: match[2]! };

  match = url.pathname.match(/^\/v1\/chains\/(\d+)\/transactions\/[^/]+\/?$/);
  if (match) return { kind: 'details', chainId: Number(match[1]) };
  return undefined;
}

function selectedSponsoredRadio(target: Window): boolean {
  const option = target.document?.querySelector('[data-testid="relay-execution-method"]');
  const radio = option?.querySelector('input[type="radio"]') as HTMLInputElement | null | undefined;
  return radio?.checked === true && radio.disabled !== true;
}

function isSharedContractSignature(signature: string): boolean {
  if (!/^0x[a-fA-F0-9]+$/.test(signature) || signature.length < 2 + 97 * 2 || signature.length % 2 !== 0) {
    return false;
  }
  const hex = signature.slice(2).toLowerCase();
  const signerWord = hex.slice(0, 64);
  const offsetWord = hex.slice(64, 128);
  const marker = hex.slice(128, 130);
  const declaredLength = BigInt(`0x${hex.slice(130, 194)}`);
  const availableLength = BigInt((hex.length - 194) / 2);
  return signerWord === SHARED_WEBAUTHN_OWNER.slice(2).toLowerCase().padStart(64, '0') &&
    BigInt(`0x${offsetWord}`) === 65n &&
    marker === '00' &&
    declaredLength > 0n &&
    declaredLength <= availableLength;
}

function matchesArmedSignature(actual: string, armed: string): boolean {
  if (actual.toLowerCase() === armed.toLowerCase()) return true;
  // Protocol Kit treats an injected-wallet response as an EOA signature and
  // normalizes its last byte to v=27/28. For this contract signature that byte
  // is only ABI padding; all signed fields and the declared length are intact.
  return actual.length === armed.length &&
    actual.slice(0, -2).toLowerCase() === armed.slice(0, -2).toLowerCase() &&
    (actual.toLowerCase().endsWith('1b') || actual.toLowerCase().endsWith('1c'));
}

export function unsignedLocalProposal(
  payload: unknown,
  armedSignature: string,
): TransactionProposal | undefined {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return undefined;
  const proposal = payload as TransactionProposal;
  if (typeof proposal.signature !== 'string' || !isSharedContractSignature(proposal.signature)) return undefined;
  if (!matchesArmedSignature(proposal.signature, armedSignature)) return undefined;
  const unsigned = { ...proposal };
  delete unsigned.signature;
  return unsigned;
}

function confirmationMatches(
  confirmation: LocalSafeConfirmation,
  chainId: number,
  safeAddress: string,
  safeTxHash: string,
): boolean {
  return confirmation.chainId === chainId &&
    confirmation.safeAddress.toLowerCase() === safeAddress.toLowerCase() &&
    confirmation.safeTxHash.toLowerCase() === safeTxHash.toLowerCase() &&
    isSharedContractSignature(confirmation.signature);
}

function hashFromTransactionId(id: unknown): string | undefined {
  if (typeof id !== 'string') return undefined;
  return id.match(/_(0x[a-fA-F0-9]{64})$/)?.[1];
}

function overlayTransactionDetails(
  payload: Record<string, any>,
  chainId: number,
  confirmations: readonly LocalSafeConfirmation[],
  safeAddressHint?: string,
): { payload: Record<string, any>; changed: boolean } {
  const info = payload.detailedExecutionInfo;
  if (!info || info.type !== 'MULTISIG' || !Array.isArray(info.confirmations)) {
    return { payload, changed: false };
  }
  if (payload.txStatus === 'SUCCESS' || payload.txStatus === 'FAILED' || payload.txStatus === 'CANCELLED') {
    return { payload, changed: false };
  }
  const safeAddress = typeof payload.safeAddress === 'string' ? payload.safeAddress : safeAddressHint;
  const safeTxHash = info.safeTxHash;
  if (!safeAddress || typeof safeTxHash !== 'string') return { payload, changed: false };
  if (info.confirmations.some((item: any) =>
    item?.signer?.value?.toLowerCase?.() === SHARED_WEBAUTHN_OWNER.toLowerCase())) {
    return { payload, changed: false };
  }
  const local = confirmations.find((entry) => confirmationMatches(entry, chainId, safeAddress, safeTxHash));
  if (!local) return { payload, changed: false };

  const nextConfirmations = [
    ...info.confirmations,
    {
      signer: { value: SHARED_WEBAUTHN_OWNER },
      signature: local.signature,
      submittedAt: local.submittedAt,
    },
  ];
  const confirmationsRequired = Number(info.confirmationsRequired ?? 0);
  return {
    changed: true,
    payload: {
      ...payload,
      txStatus: confirmationsRequired > 0 && nextConfirmations.length >= confirmationsRequired
        ? 'AWAITING_EXECUTION'
        : payload.txStatus,
      detailedExecutionInfo: { ...info, confirmations: nextConfirmations },
    },
  };
}

function overlayQueue(
  payload: Record<string, any>,
  chainId: number,
  safeAddress: string,
  confirmations: readonly LocalSafeConfirmation[],
  includeLocalUntrusted = false,
): { payload: Record<string, any>; changed: boolean } {
  if (!Array.isArray(payload.results)) return { payload, changed: false };
  let changed = false;
  const results = payload.results.map((item: any) => {
    const transaction = item?.type === 'TRANSACTION' ? item.transaction : undefined;
    const info = transaction?.executionInfo;
    const safeTxHash = hashFromTransactionId(transaction?.id);
    if (!transaction || info?.type !== 'MULTISIG' || !safeTxHash) return item;
    const local = confirmations.find((entry) => confirmationMatches(entry, chainId, safeAddress, safeTxHash));
    if (!local) return item;

    const missingSigners = Array.isArray(info.missingSigners) ? info.missingSigners : [];
    const sharedIsMissing = missingSigners.some((entry: any) =>
      entry?.value?.toLowerCase?.() === SHARED_WEBAUTHN_OWNER.toLowerCase());
    const submitted = Number(info.confirmationsSubmitted ?? 0);
    const required = Number(info.confirmationsRequired ?? 0);
    // The summary response has no signer identities for submitted
    // confirmations. Only overlay when it explicitly marks the shared owner as
    // missing, or for the unambiguous unsigned 1/1 case.
    if (!sharedIsMissing && !(submitted === 0 && required === 1)) return item;

    const nextSubmitted = submitted + 1;
    changed = true;
    return {
      ...item,
      transaction: {
        ...transaction,
        txStatus: required > 0 && nextSubmitted >= required ? 'AWAITING_EXECUTION' : transaction.txStatus,
        executionInfo: {
          ...info,
          confirmationsSubmitted: nextSubmitted,
          missingSigners: missingSigners.filter((entry: any) =>
            entry?.value?.toLowerCase?.() !== SHARED_WEBAUTHN_OWNER.toLowerCase()),
        },
      },
    };
  }).filter((item: any) => {
    if (!includeLocalUntrusted || item?.type !== 'TRANSACTION') return true;
    const transaction = item.transaction;
    const info = transaction?.executionInfo;
    if (info?.type !== 'MULTISIG' || Number(info.confirmationsSubmitted ?? 0) > 0) return true;
    const safeTxHash = hashFromTransactionId(transaction?.id);
    return Boolean(safeTxHash && confirmations.some((entry) =>
      confirmationMatches(entry, chainId, safeAddress, safeTxHash)));
  });
  if (results.length !== payload.results.length) changed = true;
  return { payload: changed ? { ...payload, results } : payload, changed };
}

export function overlayLocalConfirmations(
  payload: unknown,
  context: SafeClientResponseContext,
  confirmations: readonly LocalSafeConfirmation[],
): { payload: unknown; changed: boolean } {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    return { payload, changed: false };
  }
  if (context.kind === 'queue') {
    return overlayQueue(
      payload as Record<string, any>,
      context.chainId,
      context.safeAddress,
      confirmations,
      context.includeLocalUntrusted,
    );
  }
  return overlayTransactionDetails(
    payload as Record<string, any>,
    context.chainId,
    confirmations,
    context.safeAddress,
  );
}

function withRequestBody(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  body: string,
): [RequestInfo | URL, RequestInit | undefined] {
  if (typeof Request !== 'undefined' && input instanceof Request) {
    return [new Request(input, { body }), init];
  }
  return [input, { ...init, body }];
}

function withRequestUrl(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  url: URL,
): [RequestInfo | URL, RequestInit | undefined] {
  if (typeof Request !== 'undefined' && input instanceof Request) {
    return [new Request(url, input), init];
  }
  return [url.href, init];
}

function withLocalQueueVisibility(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  target: Window,
  confirmations: readonly LocalSafeConfirmation[],
): [RequestInfo | URL, RequestInit | undefined] {
  const context = safeClientResponseContext(input, target);
  if (context?.kind !== 'queue' || !context.includeLocalUntrusted) return [input, init];
  const hasLocal = confirmations.some((entry) =>
    entry.chainId === context.chainId &&
    entry.safeAddress.toLowerCase() === context.safeAddress.toLowerCase());
  if (!hasLocal) return [input, init];
  const url = requestUrl(input, target);
  if (!url) return [input, init];
  // The service marks unsigned proposals as untrusted. Fetch all queued items,
  // then filter the response back to the normal trusted set plus only those
  // unsigned items that have a matching extension-held confirmation.
  url.searchParams.set('trusted', 'false');
  return withRequestUrl(input, init, url);
}

async function withLocalConfirmationOverlay(
  input: RequestInfo | URL,
  response: Response,
  target: Window,
  confirmations: readonly LocalSafeConfirmation[],
): Promise<Response> {
  const context = safeClientResponseContext(input, target);
  if (!context || confirmations.length === 0 || !response.ok) return response;
  let json: unknown;
  try {
    json = await response.clone().json();
  } catch {
    return response;
  }
  const overlaid = overlayLocalConfirmations(json, context, confirmations);
  if (!overlaid.changed) return response;
  const headers = new Headers(response.headers);
  headers.delete('content-length');
  return new Response(JSON.stringify(overlaid.payload), {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

/**
 * Safe Wallet classifies every address with code as an on-chain-only smart
 * wallet. The shared WebAuthn signer is a validator contract, not a wallet that
 * can originate approveHash transactions. On app.safe.global only, make that
 * one classification lookup appear EOA-like so Safe asks for an off-chain
 * signature. All actual RPC validation in the extension still sees real code.
 */
export function installSafeOwnerCompatibility(target: Window): SafeOwnerCompatibility {
  const nativeFetch = target.fetch.bind(target);
  let armedLocalProposal: ArmedLocalProposal | undefined;
  let localConfirmations: LocalSafeConfirmation[] = [];

  const addLocalConfirmation = (confirmation: LocalSafeConfirmation) => {
    if (!isSharedContractSignature(confirmation.signature)) return;
    const key = `${confirmation.chainId}:${confirmation.safeAddress.toLowerCase()}:${confirmation.safeTxHash.toLowerCase()}`;
    localConfirmations = [
      confirmation,
      ...localConfirmations.filter((entry) =>
        `${entry.chainId}:${entry.safeAddress.toLowerCase()}:${entry.safeTxHash.toLowerCase()}` !== key),
    ];
  };

  const compatibility: SafeOwnerCompatibility = {
    isSponsoredExecutionSelected: () => selectedSponsoredRadio(target),
    armUnsignedProposal: (signature: string, sponsoredExecution: boolean) => {
      if (sponsoredExecution && !selectedSponsoredRadio(target)) return;
      if (!isSharedContractSignature(signature)) return;
      armedLocalProposal = {
        signature,
        sponsoredExecution,
        expiresAt: Date.now() + LOCAL_PROPOSAL_TTL_MS,
      };
    },
    setLocalConfirmations: (confirmations) => {
      localConfirmations = [];
      for (const confirmation of confirmations) addLocalConfirmation(confirmation);
    },
    addLocalConfirmation,
  };

  target.fetch = (async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const body = await requestBody(input, init);
    if (!body) {
      const visibleRequest = withLocalQueueVisibility(input, init, target, localConfirmations);
      const response = await nativeFetch(visibleRequest[0], visibleRequest[1]);
      return withLocalConfirmationOverlay(input, response, target, localConfirmations);
    }

    let payload: unknown;
    try {
      payload = JSON.parse(body);
    } catch {
      const response = await nativeFetch(input, init);
      return withLocalConfirmationOverlay(input, response, target, localConfirmations);
    }

    if (isSafeProposalRequest(input, target)) {
      const armed = armedLocalProposal;
      const selectionMatches = !armed?.sponsoredExecution || selectedSponsoredRadio(target);
      if (armed && armed.expiresAt >= Date.now() && selectionMatches) {
        const unsigned = unsignedLocalProposal(payload, armed.signature);
        if (unsigned) {
          // Consume the arm once. Only the hosted proposal is made unsigned
          // because its ERC-1271 caller is wrong for SafeWebAuthnSharedSigner.
          // The validated signature stays in extension storage and is merged
          // into queue/details responses (and remains in memory for a relay).
          armedLocalProposal = undefined;
          const rewritten = withRequestBody(input, init, JSON.stringify(unsigned));
          const response = await nativeFetch(rewritten[0], rewritten[1]);
          return withLocalConfirmationOverlay(input, response, target, localConfirmations);
        }
      }
      if (armed && armed.expiresAt < Date.now()) armedLocalProposal = undefined;
      const response = await nativeFetch(input, init);
      return withLocalConfirmationOverlay(input, response, target, localConfirmations);
    }

    const hasTarget = (Array.isArray(payload) ? payload : [payload]).some(isOwnerCodeRequest);
    if (!hasTarget) {
      const response = await nativeFetch(input, init);
      return withLocalConfirmationOverlay(input, response, target, localConfirmations);
    }

    const response = await nativeFetch(input, init);
    let json: unknown;
    try {
      json = await response.clone().json();
    } catch {
      return response;
    }
    const rewritten = rewriteOwnerCodeResponse(payload, json);
    const headers = new Headers(response.headers);
    headers.delete('content-length');
    return new Response(JSON.stringify(rewritten), {
      status: response.status,
      statusText: response.statusText,
      headers,
    });
  }) as typeof fetch;

  return compatibility;
}
