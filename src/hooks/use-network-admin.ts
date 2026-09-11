/**
 * Network administration — WEB, driven by the portable Rust state machine
 * (spec 017, `rust/crates/vela-core/src/app/network_admin.rs`).
 *
 * This file owns no rules. The duplicate-chain gate (now ONE implementation for
 * both the wizard and the EIP-681 scan path — the TypeScript sources had
 * diverged), the RPC candidate assembly, the fastest-endpoint race, the
 * eleven-contract + RIP-7212 verdict, the trim/CR-LF cleaning, the
 * clear-key-removes-provider rule and every pool/bundler-cache flush are decided
 * (and tested) in Rust. The shell injects the clock, renders what the core
 * projects, and words the copy the core deliberately does not carry.
 *
 * ONE module-level session is shared by every mount and by the Send screen's
 * scan recovery — see `wallet-state-core/network-admin-resident.web.ts`.
 */
import { useCallback, useEffect, useState } from 'react';

import type { Network } from '@/models/network';
import type { CompatibilityResult, ServiceEndpoints } from '@/models/types';
import { DEFAULT_SERVICE_ENDPOINTS } from '@/models/types';
import {
  dispatchNetworkAdmin as dispatch,
  ensureNetworkAdmin,
  networkAdminView,
  subscribeNetworkAdmin,
} from '@/services/wallet-state-core/network-admin-resident';
import type { NetEndpointField } from '@/services/wallet-state-core/generated/NetEndpointField';
import type { NetNetworkRow } from '@/services/wallet-state-core/generated/NetNetworkRow';
import type { NetProbeHealth } from '@/services/wallet-state-core/generated/NetProbeHealth';
import type { NetServiceHealth } from '@/services/wallet-state-core/generated/NetServiceHealth';
import type { NetView } from '@/services/wallet-state-core/generated/NetView';
import type { NetWizardErrorKind } from '@/services/wallet-state-core/generated/NetWizardErrorKind';
import type { ProviderId } from '@/services/rpc-providers';

import { useAllNetworks } from './use-networks';
import type {
  AddNetworkController,
  EndpointHealth,
  NetworkCardView,
  NetworkEditorController,
  ProviderTestView,
  RpcProvidersController,
  ServiceEndpointsController,
  ServiceHealth,
} from './network-admin-controller-types';

// ---------------------------------------------------------------------------
// The resident session, bridged to React
// ---------------------------------------------------------------------------

/** Subscribe this component to the shared view. */
function useNetView(): NetView {
  const [view, setView] = useState<NetView>(() => networkAdminView());
  useEffect(() => {
    const unsubscribe = subscribeNetworkAdmin(setView);
    ensureNetworkAdmin();
    setView(networkAdminView());
    return unsubscribe;
  }, []);
  return view;
}

// ---------------------------------------------------------------------------
// Wire → badge mappings (the copy the core deliberately does not carry)
// ---------------------------------------------------------------------------

function toEndpointHealth(health: NetProbeHealth | null): EndpointHealth {
  if (health === null) return { status: 'checking' };
  switch (health.type) {
    case 'checking':
      return { status: 'checking' };
    case 'ok':
      return { status: 'ok', latencyMs: health.latency_ms };
    case 'error':
      return { status: 'error' };
  }
}

/** `SERVICE_IDENTITY` — the words the `invalid_response` badge shows. */
const SERVICE_IDENTITY: Record<NetEndpointField, string | null> = {
  ethereum_data: 'ethereum-data',
  passkey_index: 'webauthn-p256-publickey-index',
  bundler_service: 'vela-relay',
  fiat_rates: null,
};

const ENDPOINT_KEY: Record<NetEndpointField, keyof ServiceEndpoints> = {
  ethereum_data: 'ethereumDataURL',
  passkey_index: 'passkeyIndexURL',
  bundler_service: 'bundlerServiceURL',
  fiat_rates: 'fiatRatesURL',
};

const FIELD_OF_KEY: Record<keyof ServiceEndpoints, NetEndpointField> = {
  ethereumDataURL: 'ethereum_data',
  passkeyIndexURL: 'passkey_index',
  bundlerServiceURL: 'bundler_service',
  fiatRatesURL: 'fiat_rates',
};

function toServiceHealth(field: NetEndpointField, health: NetServiceHealth): ServiceHealth {
  switch (health.type) {
    case 'checking':
      return { status: 'checking' };
    case 'ok':
      return {
        status: 'ok',
        latencyMs: health.latency_ms,
        detail: health.rate_count === null ? undefined : `${health.rate_count} currencies`,
      };
    case 'not_https':
      return { status: 'not_https', detail: 'HTTPS required' };
    case 'unreachable':
      return health.http_status === null
        ? { status: 'unreachable', detail: 'Connection failed' }
        : {
            status: 'unreachable',
            latencyMs: health.latency_ms ?? undefined,
            detail: `HTTP ${health.http_status}`,
          };
    case 'invalid_response': {
      const expected = SERVICE_IDENTITY[field];
      return {
        status: 'invalid_response',
        latencyMs: health.latency_ms,
        // The fiat endpoint has no identity to fail; it lands here only by
        // serving no rates at all.
        detail: expected === null ? 'No rates returned' : `Not a valid ${expected} service`,
      };
    }
  }
}

/** The wizard's error strings, byte-identical to the ones the modal showed. */
function wizardErrorText(error: NetWizardErrorKind | null): string {
  if (error === null) return '';
  switch (error.type) {
    case 'already_added':
      return `This network is already added`;
    case 'not_found':
      return `Chain ${error.chain_id} not found`;
    case 'no_rpc_endpoint':
      return 'No RPC endpoint available for this network';
    case 'not_compatible':
      // Reachable only on the scan path, which never renders this controller.
      return 'Check failed';
    case 'check_failed':
      // Spec 038 #E1: the probes could not run (RPC down, timeout) — not a
      // verdict on the chain. Same path as above; the sentence is the corpus's.
      return "Unable to verify \u2014 RPC request failed";
  }
}

/** `checkNetworkCompatibility`'s `error` sentence, rebuilt from the verdict. */
function compatibilityError(compat: NonNullable<NetView['wizard']['compat']>): string | undefined {
  if (compat.rpc_failure === 'no_https_candidates') return 'No valid HTTPS RPC endpoints available';
  if (compat.rpc_failure === 'all_probes_failed') return 'All RPC endpoints failed or timed out';
  const missing = compat.contracts.filter((c) => !c.deployed);
  const issues: string[] = [];
  if (missing.length > 0) {
    issues.push(
      `${missing.length} contract${missing.length > 1 ? 's' : ''} not deployed: ${missing.map((c) => c.name).join(', ')}`,
    );
  }
  if (!compat.p256_available) {
    issues.push('P256 precompile (RIP-7212) not available — passkey signatures will not work');
  }
  return issues.length > 0 ? issues.join('. ') : undefined;
}

export function toCompatibilityResult(
  compat: NonNullable<NetView['wizard']['compat']>,
): CompatibilityResult {
  return {
    chainId: compat.chain_id,
    compatible: compat.compatible,
    contracts: compat.contracts,
    bestRpcUrl: compat.best_rpc_url ?? undefined,
    bestRpcLatency: compat.best_rpc_latency_ms ?? undefined,
    p256Available: compat.p256_available ?? undefined,
    rpcFailed: compat.rpc_failure !== null,
    error: compatibilityError(compat),
  };
}

/**
 * The visual identity of one row. The core carries the editable state and the
 * verdicts; icons and logo URLs stay shell data, resolved from the same network
 * list every other screen renders. A row the snapshot has not caught up with yet
 * (the moment a custom network is saved) falls back to the record's own icon
 * policy rather than rendering blank.
 */
function toNetwork(row: NetNetworkRow, known: Map<number, Network>): Network {
  const match = known.get(row.chain_id);
  if (match) return match;
  return {
    id: row.id,
    displayName: row.display_name,
    chainId: row.chain_id,
    iconLabel: row.native_symbol.slice(0, 4),
    iconColor: '#888888',
    iconBg: '#F0F0F0',
    logoURL: '',
    isL2: false,
    rpcURL: row.rpc_url,
    explorerURL: row.explorer_url,
    bundlerURL: row.bundler_url,
  };
}

// ---------------------------------------------------------------------------
// Network editor
// ---------------------------------------------------------------------------

export function useNetworkEditor(): NetworkEditorController {
  const view = useNetView();
  const networks = useAllNetworks();
  const known = new Map(networks.map((n) => [n.chainId, n]));

  // Until the store read lands the core projects no rows; render the same
  // built-in list the TypeScript editor showed while ITS load was in flight.
  const cards: NetworkCardView[] =
    view.networks.length > 0
      ? view.networks.map((row) => ({
          network: toNetwork(row, known),
          isCustom: row.is_custom,
          rpcURL: row.rpc_url,
          explorerURL: row.explorer_url,
          healths: [toEndpointHealth(row.rpc_health), toEndpointHealth(row.explorer_health)],
          // The invariant-④ gate's verdict. The core only ever sets this from a
          // chain id the endpoint actually reported, so the screen can state
          // both numbers as fact.
          rpcMismatch:
            row.rpc_chain_mismatch === null
              ? undefined
              : {
                  expectedChainId: row.rpc_chain_mismatch.expected_chain_id,
                  reportedChainId: row.rpc_chain_mismatch.reported_chain_id,
                },
        }))
      : networks.map((network) => ({
          network,
          isCustom: false,
          rpcURL: network.rpcURL,
          explorerURL: network.explorerURL,
          healths: [{ status: 'checking' }, { status: 'checking' }],
        }));

  return {
    cards,
    // The core is resident and hydrated at first use; there is nothing to reload.
    open: useCallback(() => { ensureNetworkAdmin(); }, []),
    expand: useCallback((chainId: number) => {
      dispatch({ type: 'override_expanded', chain_id: chainId });
    }, []),
    // Collapsing is pure UI: the core keeps the card's drafts, and re-expanding
    // starts a fresh probe wave by itself.
    collapse: useCallback(() => {}, []),
    setRpcURL: useCallback((chainId: number, value: string) => {
      dispatch({ type: 'override_field_edited', chain_id: chainId, field: 'rpc', value });
    }, []),
    setExplorerURL: useCallback((chainId: number, value: string) => {
      dispatch({ type: 'override_field_edited', chain_id: chainId, field: 'explorer', value });
    }, []),
    save: useCallback((chainId: number) => {
      dispatch({ type: 'override_blurred', chain_id: chainId });
    }, []),
    remove: useCallback((id: string) => {
      dispatch({ type: 'delete_confirmed', id });
    }, []),
  };
}

// ---------------------------------------------------------------------------
// Service endpoints
// ---------------------------------------------------------------------------

export function useServiceEndpoints(): ServiceEndpointsController {
  const view = useNetView();

  return {
    // Same reason as the network rows: before the store read lands the core has
    // nothing to project, and the editor used to open on the defaults.
    fields:
      view.endpoints.length > 0
        ? view.endpoints.map((endpoint) => ({
            key: ENDPOINT_KEY[endpoint.field],
            value: endpoint.value,
            health: toServiceHealth(endpoint.field, endpoint.health),
          }))
        : (Object.keys(FIELD_OF_KEY) as (keyof ServiceEndpoints)[]).map((key) => ({
            key,
            value: DEFAULT_SERVICE_ENDPOINTS[key],
            health: { status: 'checking' as const },
          })),
    open: useCallback(() => {
      dispatch({ type: 'endpoints_opened' });
    }, []),
    setValue: useCallback((key: keyof ServiceEndpoints, value: string) => {
      dispatch({ type: 'endpoint_edited', field: FIELD_OF_KEY[key], value });
    }, []),
    save: useCallback((key: keyof ServiceEndpoints) => {
      dispatch({ type: 'endpoint_blurred', field: FIELD_OF_KEY[key] });
    }, []),
    refresh: useCallback(() => {
      dispatch({ type: 'endpoints_refresh_requested' });
    }, []),
    resetToDefaults: useCallback(() => {
      dispatch({ type: 'reset_endpoints_to_defaults' });
    }, []),
  };
}

// ---------------------------------------------------------------------------
// Add-network wizard
// ---------------------------------------------------------------------------

export function useAddNetworkWizard(): AddNetworkController {
  const view = useNetView();
  const wizard = view.wizard;

  return {
    query: wizard.query,
    suggestions: wizard.suggestions.map((s) => ({
      chainId: s.chain_id,
      name: s.name,
      nativeCurrencySymbol: s.native_currency_symbol,
    })),
    searching: wizard.phase === 'searching',
    loading: wizard.phase === 'resolving' || wizard.phase === 'checking',
    // The core's save is a ledger write followed by a best-effort persist, so
    // there is no window to spin through.
    saving: false,
    error: wizardErrorText(wizard.error),
    chainInfo: wizard.chain_info
      ? {
          chainId: wizard.chain_info.chain_id,
          name: wizard.chain_info.name,
          nativeSymbol: wizard.chain_info.native_symbol,
          isTestnet: wizard.chain_info.is_testnet,
        }
      : null,
    compat: wizard.compat ? toCompatibilityResult(wizard.compat) : null,
    // The recheck/retry affordances re-select the resolved chain; the core clears
    // `chain_info` for every state where nothing is selected.
    selectedChainId: wizard.chain_info?.chain_id ?? null,
    customRpc: wizard.custom_rpc,
    addedChainId: view.last_added_chain_id,
    setQuery: useCallback((text: string) => {
      dispatch({ type: 'search_input', query: text });
    }, []),
    setCustomRpc: useCallback((value: string) => {
      dispatch({ type: 'custom_rpc_edited', value });
    }, []),
    select: useCallback((chainId: number, keepCustomRpc = false) => {
      dispatch({ type: 'chain_selected', chain_id: chainId, keep_custom_rpc: keepCustomRpc });
    }, []),
    add: useCallback(() => {
      dispatch({ type: 'add_confirmed', now_iso: new Date().toISOString() });
    }, []),
    reset: useCallback(() => {
      dispatch({ type: 'wizard_reset' });
    }, []),
  };
}

// ---------------------------------------------------------------------------
// RPC providers
// ---------------------------------------------------------------------------

export function useRpcProviders(): RpcProvidersController {
  const view = useNetView();

  const keys: Partial<Record<ProviderId, string>> = {};
  const tests: Partial<Record<ProviderId, ProviderTestView>> = {};
  for (const provider of view.providers) {
    keys[provider.provider] = provider.key;
    if (provider.test) {
      tests[provider.provider] = {
        status: provider.test.done ? 'done' : 'testing',
        results: provider.test.results.map((r) => ({
          chainId: r.chain_id,
          ok: r.ok,
          latencyMs: r.latency_ms,
        })),
      };
    }
  }

  return {
    keys,
    tests,
    open: useCallback(() => {
      dispatch({ type: 'providers_opened' });
    }, []),
    setKey: useCallback((id: ProviderId, value: string) => {
      dispatch({ type: 'provider_key_edited', provider: id, value });
    }, []),
    blur: useCallback((id: ProviderId) => {
      dispatch({ type: 'provider_key_blurred', provider: id });
    }, []),
    test: useCallback((id: ProviderId) => {
      dispatch({ type: 'provider_test_requested', provider: id });
    }, []),
  };
}
