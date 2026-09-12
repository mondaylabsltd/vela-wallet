/**
 * The only place the `network_admin` core touches the outside world — WEB.
 *
 * Ported from src/services/wallet-state-core/network-admin-executor.ts
 * @ e78afdfa (spec 024). Same sixteen operations, same codecs, same probes,
 * same stored bytes. The probes travel WITH the executor — they are its
 * operation-local HTTP, not the RPC pool — because the core's add gate is
 * real: `add_confirmed` refuses a candidate whose compatibility was never
 * verified, so a probe-less settings screen could never add a network
 * (research D1, revised). What genuinely waits for spec 025 is the two cache
 * invalidations (`invalidate_pools`, `clear_bundler_cache`): there is no
 * pool and no bundler cache on web yet, so both are acknowledged no-ops.
 *
 * No branching on business meaning: the dedup gate, the candidate assembly,
 * the contract verdict and the clear-key-removes-provider rule are all
 * decided (and tested) in Rust.
 *
 * Wire vs stored shape: the core speaks snake_case (`rpc_url`,
 * `added_at_iso`); the four storage keys hold the camelCase records the Expo
 * services write (`rpcURL`, `addedAt`). Translating between them is this
 * file's job — the stored bytes stay byte-compatible so a record written by
 * any client reads on every other.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import { getItem, setItem } from '$lib/services/storage';
import { refreshCustomNetworks } from '$lib/services/networks';
import { invalidateAllPools, refreshPool } from '$lib/services/rpc-pool';
import { fetchRawChainData, loadSearchIndex } from '$lib/services/chain-registry';
import { fetchWithTimeout, NET_TIMEOUTS } from '$lib/services/net';
import {
	loadServiceEndpoints,
	saveServiceEndpoints,
	type ServiceEndpoints
} from '$lib/onboarding/core/storage';

import type { NetChainIndexEntry } from '$lib/core/generated/NetChainIndexEntry';
import type { NetCustomNetwork } from '$lib/core/generated/NetCustomNetwork';
import type { NetHealthBody } from '$lib/core/generated/NetHealthBody';
import type { NetRawChainData } from '$lib/core/generated/NetRawChainData';
import type { NetNetworkConfig } from '$lib/core/generated/NetNetworkConfig';
import type { NetProviderKeys } from '$lib/core/generated/NetProviderKeys';
import type { NetShellResult } from '$lib/core/generated/NetShellResult';
import type { NetStoredEndpoints } from '$lib/core/generated/NetStoredEndpoints';
import type { NetEffect } from './network-admin-types';

/** The same keys the Expo `services/storage.ts` owns; value formats unchanged.
 *  `vela.serviceEndpoints` is NOT here: it lives in localStorage with its
 *  onboarding readers (research D3a) and is reached through their module. */
const KEYS = {
	customNetworks: 'vela.customNetworks',
	networkConfig: 'vela.networkConfig',
	rpcProviders: 'vela.rpcProviders'
} as const;

/** `rpcCall`'s / `testRpcLatency`'s / `checkEndpointHealth`'s shared budget. */
const PROBE_TIMEOUT_MS = NET_TIMEOUTS.networkCheck;

/** RIP-7212 P256 precompile address (network-checker.ts:180). */
const P256_PRECOMPILE = '0x0000000000000000000000000000000000000100';

/** sha256("test") signed with a known P-256 key — network-checker.ts:183-189. */
const VALID_P256_CALL =
	'0x' +
	'9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08' +
	'7bf0e18d07660f15994adce5c3836d7bd6167cdb5726f631098f433ebe0be9c0' +
	'3936edbe5c791477e714e58244afb690b9b88b833ff4acdf0fbd1b28bf0b1182' +
	'3be8cbcb3f590087711ae5ed74b9cd06a88058d0bbe700b5f0ec5a1bfac15592' +
	'f989ef9bfaae0fee03c36625e88eae99806a879d813411f876e7e03a2ffd8314';

// ---------------------------------------------------------------------------
// Codecs — wire (snake_case) vs stored (camelCase). Ported verbatim.
// ---------------------------------------------------------------------------

const asString = (value: unknown): string => (typeof value === 'string' ? value : '');
const asOptionalString = (value: unknown): string | null =>
	typeof value === 'string' ? value : null;

/**
 * Decode one stored `CustomNetwork`. A codec, not a policy: serde rejects a
 * record whose fields are missing or mistyped, and a rejected `StoreLoaded`
 * would strand the core unloaded forever (every write is then dropped), so
 * junk is coerced to the empty/zero value.
 */
function decodeCustomNetwork(raw: unknown): NetCustomNetwork | null {
	if (raw === null || typeof raw !== 'object') return null;
	const r = raw as Record<string, unknown>;
	const chainId = Number(r.chainId);
	return {
		id: asString(r.id),
		display_name: asString(r.displayName),
		chain_id: Number.isFinite(chainId) ? chainId : 0,
		icon_label: asString(r.iconLabel),
		icon_color: asString(r.iconColor),
		icon_bg: asString(r.iconBg),
		logo_url: asString(r.logoURL),
		is_l2: r.isL2 === true,
		rpc_url: asString(r.rpcURL),
		explorer_url: asString(r.explorerURL),
		bundler_url: asString(r.bundlerURL),
		native_symbol: asString(r.nativeSymbol),
		added_at_iso: asString(r.addedAt)
	};
}

function encodeCustomNetwork(n: NetCustomNetwork): Record<string, unknown> {
	return {
		id: n.id,
		displayName: n.display_name,
		chainId: n.chain_id,
		iconLabel: n.icon_label,
		iconColor: n.icon_color,
		iconBg: n.icon_bg,
		logoURL: n.logo_url,
		isL2: n.is_l2,
		rpcURL: n.rpc_url,
		explorerURL: n.explorer_url,
		bundlerURL: n.bundler_url,
		nativeSymbol: n.native_symbol,
		addedAt: n.added_at_iso
	};
}

function decodeNetworkConfig(raw: unknown): NetNetworkConfig | null {
	if (raw === null || typeof raw !== 'object') return null;
	const r = raw as Record<string, unknown>;
	const chainId = Number(r.chainId);
	return {
		chain_id: Number.isFinite(chainId) ? chainId : 0,
		rpc_url: asString(r.rpcURL),
		explorer_url: asString(r.explorerURL),
		bundler_url: asString(r.bundlerURL)
	};
}

function encodeNetworkConfig(c: NetNetworkConfig): Record<string, unknown> {
	return {
		chainId: c.chain_id,
		rpcURL: c.rpc_url,
		explorerURL: c.explorer_url,
		bundlerURL: c.bundler_url
	};
}

/** `loadArray`'s body: absent, unparseable or non-array contents read as empty. */
function decodeArray<T>(raw: string | null, decode: (item: unknown) => T | null): T[] {
	if (!raw) return [];
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return [];
	}
	if (!Array.isArray(parsed)) return [];
	return parsed.map(decode).filter((item): item is T => item !== null);
}

/**
 * The RAW endpoints blob — absent fields stay absent so the core applies the
 * same defaults merge the services always did. Read through the onboarding
 * module (D3a): one key, one storage home.
 */
function decodeStoredEndpoints(stored: ServiceEndpoints): NetStoredEndpoints {
	return {
		ethereum_data_url: asOptionalString(stored.ethereumDataURL),
		passkey_index_url: asOptionalString(stored.passkeyIndexURL),
		bundler_service_url: asOptionalString(stored.bundlerServiceURL),
		fiat_rates_url: asOptionalString(stored.fiatRatesURL)
	};
}

function decodeProviderKeys(raw: string | null): NetProviderKeys {
	const empty: NetProviderKeys = { alchemy: null, drpc: null, ankr: null };
	if (!raw) return empty;
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return empty;
	}
	if (parsed === null || typeof parsed !== 'object') return empty;
	const r = parsed as Record<string, unknown>;
	return {
		alchemy: asOptionalString(r.alchemy),
		drpc: asOptionalString(r.drpc),
		ankr: asOptionalString(r.ankr)
	};
}

/**
 * `/chains/eip155-{id}.json` as the core wants it: raw, unfiltered. Only the
 * explorer entries are flattened to their `url` (the core's field is a string
 * list); every parsing *decision* — the `??` defaults, the HTTPS filter, the
 * key-placeholder rejection — belongs to `parse_chain_data` in Rust.
 */
function decodeRawChainData(data: unknown): NetRawChainData | null {
	if (data === null || typeof data !== 'object') return null;
	const r = data as Record<string, unknown>;
	const native = (r.nativeCurrency ?? null) as Record<string, unknown> | null;
	const chainId = typeof r.chainId === 'number' ? r.chainId : null;
	const decimals = native && typeof native.decimals === 'number' ? native.decimals : null;
	return {
		chain_id: chainId,
		name: asOptionalString(r.name),
		short_name: asOptionalString(r.shortName),
		native_currency_name: native ? asOptionalString(native.name) : null,
		native_currency_symbol: native ? asOptionalString(native.symbol) : null,
		native_currency_decimals: decimals,
		rpc: Array.isArray(r.rpc) ? r.rpc.filter((u): u is string => typeof u === 'string') : [],
		explorers: Array.isArray(r.explorers)
			? r.explorers.map((e) =>
					e !== null && typeof e === 'object' ? asString((e as Record<string, unknown>).url) : ''
				)
			: [],
		testnet: r.testnet === true
	};
}

/** The core's `chain_id` is a `u32`; an id the core cannot represent is not a row. */
const MAX_CHAIN_ID = 0xffff_ffff;

/**
 * The live index carries chain ids above `u32::MAX` (7078815900 is the first).
 * One such row used to pass through as a JS number, serde then refused the
 * ENTIRE `search_index` result over it, and the add-network wizard sat on
 * 搜索中 forever with nothing in any log. A row the core cannot represent is
 * dropped here — the shell's one job on this path is to hand the core rows it
 * can read (found by the iOS session, spec 050; fixed for web in 028 US5).
 */
function decodeSearchIndex(rows: unknown): NetChainIndexEntry[] {
	if (!Array.isArray(rows)) return [];
	const out: NetChainIndexEntry[] = [];
	for (const raw of rows) {
		if (raw === null || typeof raw !== 'object') continue;
		const r = raw as Record<string, unknown>;
		const chainId = Number(r.chainId);
		if (!Number.isInteger(chainId) || chainId < 0 || chainId > MAX_CHAIN_ID) continue;
		out.push({
			chain_id: chainId,
			name: asString(r.name),
			short_name: asString(r.shortName),
			native_currency_symbol: asString(r.nativeCurrencySymbol),
			has_logo: r.hasLogo === true
		});
	}
	return out;
}

/** Cleared keys are dropped, not stored as null — the Expo saver's shape. */
function encodeProviderKeys(keys: NetProviderKeys): Record<string, string> {
	const out: Record<string, string> = {};
	if (keys.alchemy !== null) out.alchemy = keys.alchemy;
	if (keys.drpc !== null) out.drpc = keys.drpc;
	if (keys.ankr !== null) out.ankr = keys.ankr;
	return out;
}

// ---------------------------------------------------------------------------
// Probes — ported verbatim; the executor's own HTTP, not a service layer.
// ---------------------------------------------------------------------------

/** `parseInt(json.result, 16)` guarded exactly as `probeRpcChainId` guards it. */
function parseReportedChainId(result: unknown): number | null {
	if (typeof result !== 'string') return null;
	const id = parseInt(result, 16);
	return Number.isFinite(id) && id > 0 ? id : null;
}

async function jsonRpc(url: string, method: string, params: unknown[]): Promise<unknown> {
	const res = await fetchWithTimeout(
		url,
		{
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params })
		},
		{ timeoutMs: PROBE_TIMEOUT_MS }
	);
	if (!res.ok) return null;
	const json = (await res.json()) as { error?: unknown; result?: unknown };
	if (json.error) return null;
	return json.result ?? null;
}

/**
 * The WebSocket half of the rpc probe, reporting the chain id rather than
 * mere truthiness so it answers the unified probe vocabulary.
 */
function probeWebSocket(url: string, start: number): Promise<NetShellResult> {
	return new Promise<NetShellResult>((resolve) => {
		let settled = false;
		const ws = new WebSocket(url);
		const done = (reported: number | null) => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			try {
				ws.close();
			} catch {
				// closing a dead socket is not news
			}
			resolve({
				type: 'probed',
				url,
				reported_chain_id: reported,
				latency_ms: Date.now() - start
			});
		};
		const timer = setTimeout(() => done(null), PROBE_TIMEOUT_MS);
		ws.onopen = () => {
			ws.send(JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }));
		};
		ws.onmessage = (event: MessageEvent) => {
			try {
				done(parseReportedChainId(JSON.parse(String(event.data)).result));
			} catch {
				done(null);
			}
		};
		ws.onerror = () => done(null);
	});
}

async function probeRpc(url: string): Promise<NetShellResult> {
	const start = Date.now();
	if (url.startsWith('wss://') || url.startsWith('ws://')) {
		return probeWebSocket(url, start);
	}
	const result = await jsonRpc(url, 'eth_chainId', []);
	return {
		type: 'probed',
		url,
		reported_chain_id: parseReportedChainId(result),
		latency_ms: Date.now() - start
	};
}

/** The `/api/health` fetch — verdict left to the core. */
async function fetchServiceHealthBody(
	baseURL: string
): Promise<{ body: NetHealthBody; latencyMs: number }> {
	const start = Date.now();
	const res = await fetchWithTimeout(
		`${baseURL}/api/health?_t=${start}`,
		{ method: 'GET' },
		{ timeoutMs: PROBE_TIMEOUT_MS }
	);
	const latencyMs = Date.now() - start;
	if (!res.ok) return { body: { type: 'http_error', status: res.status }, latencyMs };
	const json = JSON.parse(await res.text());
	return {
		body: {
			type: 'identity',
			service: asOptionalString(json?.service),
			status: asOptionalString(json?.status)
		},
		latencyMs
	};
}

/**
 * The fiat probe. The two accepted shapes decide the COUNT here (a
 * shell-side shape question); whether a count of zero is a failure is the
 * core's call.
 */
async function fetchFiatRatesBody(
	url: string
): Promise<{ body: NetHealthBody; latencyMs: number }> {
	const start = Date.now();
	const res = await fetchWithTimeout(url, { method: 'GET' }, { timeoutMs: PROBE_TIMEOUT_MS });
	const latencyMs = Date.now() - start;
	if (!res.ok) return { body: { type: 'http_error', status: res.status }, latencyMs };
	const data = (await res.json()) as unknown;
	const rates = (data as { rates?: unknown } | null)?.rates;
	const count = Array.isArray(data)
		? data.length
		: rates !== null && typeof rates === 'object'
			? Object.keys(rates).length
			: 0;
	return { body: { type: 'rates', rate_count: count }, latencyMs };
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

export async function executeNetworkAdminOperation(effect: NetEffect): Promise<NetShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_store': {
			const [customNetworks, networkConfigs, providers] = await Promise.all([
				getItem(KEYS.customNetworks),
				getItem(KEYS.networkConfig),
				getItem(KEYS.rpcProviders)
			]);
			return {
				type: 'store_loaded',
				custom_networks: decodeArray(customNetworks, decodeCustomNetwork),
				network_configs: decodeArray(networkConfigs, decodeNetworkConfig),
				endpoints: decodeStoredEndpoints(loadServiceEndpoints()),
				provider_keys: decodeProviderKeys(providers)
			};
		}

		case 'write_custom_networks':
			await setItem(
				KEYS.customNetworks,
				JSON.stringify(operation.networks.map(encodeCustomNetwork))
			);
			// Cache invalidation, not a decision (the Expo comment's rule): the
			// synchronous network snapshot must re-read the bytes just written.
			await refreshCustomNetworks();
			return { type: 'written' };

		case 'write_network_configs':
			await setItem(KEYS.networkConfig, JSON.stringify(operation.configs.map(encodeNetworkConfig)));
			return { type: 'written' };

		case 'write_service_endpoints': {
			// Through the onboarding module (D3a): same key, same camelCase shape.
			// The core sends the complete merged record — four strings, verbatim,
			// exactly as the Expo saver received them.
			const e = operation.endpoints;
			saveServiceEndpoints({
				ethereumDataURL: e.ethereum_data_url,
				passkeyIndexURL: e.passkey_index_url,
				bundlerServiceURL: e.bundler_service_url,
				fiatRatesURL: e.fiat_rates_url
			});
			return { type: 'written' };
		}

		case 'write_rpc_providers':
			await setItem(KEYS.rpcProviders, JSON.stringify(encodeProviderKeys(operation.keys)));
			return { type: 'written' };

		case 'start_search_debounce':
			await new Promise<void>((resolve) => setTimeout(resolve, operation.ms));
			return { type: 'debounce_elapsed' };

		case 'fetch_search_index':
			return { type: 'search_index', chains: decodeSearchIndex(await loadSearchIndex()) };

		case 'fetch_chain_info':
			return {
				type: 'chain_info',
				chain_id: operation.chain_id,
				data: decodeRawChainData(await fetchRawChainData(operation.chain_id))
			};

		case 'probe_rpc':
			return probeRpc(operation.url);

		case 'probe_reachable': {
			// An explorer is a website, not a JSON API: it sends no CORS headers,
			// so a normal fetch is blocked and every explorer falsely reads
			// offline. `no-cors` still sends the request, and "resolved without
			// throwing" is the only honest cross-origin liveness signal.
			const start = Date.now();
			await fetchWithTimeout(
				operation.url,
				{ method: 'GET', mode: 'no-cors', redirect: 'follow' },
				{ timeoutMs: PROBE_TIMEOUT_MS }
			);
			return { type: 'reachable', url: operation.url, ok: true, latency_ms: Date.now() - start };
		}

		case 'rpc_get_code': {
			const code = await jsonRpc(operation.url, 'eth_getCode', [operation.address, 'latest']);
			return {
				type: 'code',
				url: operation.url,
				address: operation.address,
				code: typeof code === 'string' ? code : null
			};
		}

		case 'rpc_call_p256': {
			// `gas: 0x100000` is the zkSync-compatibility quirk of the legacy call.
			const result = await jsonRpc(operation.url, 'eth_call', [
				{ to: P256_PRECOMPILE, data: VALID_P256_CALL, gas: '0x100000' },
				'latest'
			]);
			return {
				type: 'p256_call',
				url: operation.url,
				result: typeof result === 'string' ? result : null
			};
		}

		case 'fetch_service_health': {
			const { body, latencyMs } = await fetchServiceHealthBody(operation.base_url);
			return { type: 'service_health', field: operation.field, body, latency_ms: latencyMs };
		}

		case 'fetch_fiat_rates': {
			const { body, latencyMs } = await fetchFiatRatesBody(operation.url);
			return { type: 'fiat_rates', body, latency_ms: latencyMs };
		}

		case 'invalidate_pools':
			// Live since spec 025: the pool facade is the one resident router.
			if (operation.chain_id === null) invalidateAllPools();
			else await refreshPool(operation.chain_id);
			return { type: 'invalidated' };

		case 'clear_bundler_cache':
			// Still an acknowledged no-op: the bundler CACHE is 026's (there is
			// no bundler client yet). Answered, never skipped.
			return { type: 'bundler_cache_cleared' };

		default: {
			const never: never = operation;
			throw new Error(`unhandled network_admin operation: ${JSON.stringify(never)}`);
		}
	}
}

export function networkAdminOperationFailure(effect: NetEffect): NetShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_store':
			// An unreadable store reads as "nothing configured" — the historical
			// `catch { [] }`. It still marks the core loaded, so writes are never
			// silently dropped.
			return {
				type: 'store_loaded',
				custom_networks: [],
				network_configs: [],
				endpoints: {
					ethereum_data_url: null,
					passkey_index_url: null,
					bundler_service_url: null,
					fiat_rates_url: null
				},
				provider_keys: { alchemy: null, drpc: null, ankr: null }
			};
		case 'write_custom_networks':
		case 'write_network_configs':
		case 'write_service_endpoints':
		case 'write_rpc_providers':
			// Best-effort, as the TS savers' callers swallow storage errors; the
			// core's in-memory ledger stays authoritative.
			return { type: 'written' };
		case 'start_search_debounce':
			return { type: 'debounce_elapsed' };
		case 'fetch_search_index':
			return { type: 'search_index', chains: [] };
		case 'fetch_chain_info':
			return { type: 'chain_info', chain_id: operation.chain_id, data: null };
		case 'probe_rpc':
			return { type: 'probed', url: operation.url, reported_chain_id: null, latency_ms: 0 };
		case 'probe_reachable':
			return { type: 'reachable', url: operation.url, ok: false, latency_ms: 0 };
		case 'rpc_get_code':
			return { type: 'code', url: operation.url, address: operation.address, code: null };
		case 'rpc_call_p256':
			return { type: 'p256_call', url: operation.url, result: null };
		case 'fetch_service_health':
			return {
				type: 'service_health',
				field: operation.field,
				body: { type: 'failed' },
				latency_ms: 0
			};
		case 'fetch_fiat_rates':
			return { type: 'fiat_rates', body: { type: 'failed' }, latency_ms: 0 };
		case 'invalidate_pools':
			return { type: 'invalidated' };
		case 'clear_bundler_cache':
			return { type: 'bundler_cache_cleared' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled network_admin operation: ${JSON.stringify(never)}`);
		}
	}
}
