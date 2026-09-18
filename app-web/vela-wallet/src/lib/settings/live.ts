/**
 * The live settings builders (spec 024, research D7): `NetView` → the same
 * display models the drawn components already consume. Siblings of the
 * fixture builders, never replacements — the galleries keep their canon.
 *
 * Presentation only. Every VALUE here is either the core's or the corpus's;
 * the one judgement this file exercises is wording states the core expresses
 * as data (a probe health, a wizard phase, a refusal) — exactly the job the
 * core's module doc assigns to shells.
 */

import { fill } from '$lib/wallet/messages';
import { shortenAddress } from '$lib/wallet/identity';
import { currencyDisplayName } from './core/currency-catalog';
import { moneyText, trimBalance } from '$lib/wallet/live';
import type { SessionAccountRow } from '$lib/core/generated/SessionAccountRow';
import {
	formatBytes,
	GROUP_OF_ITEM,
	STORAGE_ITEM_IDS,
	type DeviceStorageReport,
	type StorageItemId
} from '$lib/services/device-storage';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { SendTreasuryStatus } from '$lib/core/generated/SendTreasuryStatus';
import { chainName } from '$lib/services/networks';
import { chainMeta as chainInfo } from '$lib/services/chains';
import { TEMPO_FEE_TOKEN_DECIMALS } from '$lib/services/tempo';
import { encodeQr } from '$lib/wallet/qr';
import { MASK } from '$lib/wallet/fixtures';

import type { NetChainIndexEntry } from '$lib/core/generated/NetChainIndexEntry';
import type { NetNetworkRow } from '$lib/core/generated/NetNetworkRow';
import type { NetProbeHealth } from '$lib/core/generated/NetProbeHealth';
import type { NetServiceHealth } from '$lib/core/generated/NetServiceHealth';
import type { NetView } from '$lib/core/generated/NetView';
import type { NetWizardView } from '$lib/core/generated/NetWizardView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import {
	dateFormatOptions,
	formatDate,
	formatNumber,
	formatTime,
	numberFormatOptions,
	timeFormatOptions,
	type FormatOption
} from '$lib/services/locale-format';
import { preferences, TEXT_SCALE_LEVELS, type ThemeChoice } from '$lib/services/preferences.svelte';
import { chainLogoURL } from '$lib/services/tokens-model';
import { chainMeta, languageRows, markFor, currencyGlyph } from './fixtures';
import type { SettingsMessages } from './messages';
import type {
	AddNetworkModel,
	BalanceDetailModel,
	ChainMarkModel,
	RelayerModel,
	RpcFixModel,
	SettingsDesktopModel,
	CheckItemModel,
	EndpointsModel,
	NetworkDetailModel,
	NetworkRowModel,
	RpcProvidersModel,
	SelectRowModel,
	SelectSheetModel,
	SettingsHomeModel,
	StatusPillModel,
	UrlFieldModel,
	AccountsSheetModel
} from './model';
import type { EthereumBackupState } from '$lib/services/registry-backup';
import type { EthereumBackupRowModel } from './model';

// ---------------------------------------------------------------------------
// Badges — the probe/health vocabularies, worded once.
// ---------------------------------------------------------------------------

function latencyLabel(ms: number, m: SettingsMessages): string {
	return ms >= 1000 ? `${m.networks.slow} · ${(ms / 1000).toFixed(1)}s` : `${ms}ms`;
}

/** A card-field probe badge. `checking` renders quiet (the field itself says). */
function probePill(
	health: NetProbeHealth | null,
	m: SettingsMessages
): StatusPillModel | undefined {
	if (health === null || health.type === 'checking') return undefined;
	if (health.type === 'ok') {
		return {
			tone: health.latency_ms >= 1000 ? 'warn' : 'ok',
			label: latencyLabel(health.latency_ms, m),
			dot: true
		};
	}
	return { tone: 'error', label: m.networks.offline, dot: true };
}

/** A service-endpoint badge — every `NetServiceHealth` variant worded. */
function servicePill(health: NetServiceHealth, m: SettingsMessages): StatusPillModel | undefined {
	switch (health.type) {
		case 'checking':
			return undefined;
		case 'ok':
			return {
				tone: health.latency_ms >= 1000 ? 'warn' : 'ok',
				label: latencyLabel(health.latency_ms, m),
				dot: true
			};
		case 'not_https':
			return { tone: 'error', label: m.networks.httpsRequired, dot: true };
		case 'unreachable':
			return { tone: 'error', label: m.networks.offline, dot: true };
		case 'invalid_response':
			return { tone: 'error', label: m.networks.invalid, dot: true };
	}
}

// ---------------------------------------------------------------------------
// Networks list + detail
// ---------------------------------------------------------------------------

/**
 * A chain's mark with its logo from the data endpoint, the drawn letter and
 * colour beneath for when the fetch fails. No chain id — an index row the
 * index itself says has no logo — draws the letter and asks for nothing.
 */
function chainMark(id: string, name: string, chainId?: number): ChainMarkModel {
	const drawn = markFor(id, name);
	return chainId === undefined ? drawn : { ...drawn, logoUrl: chainLogoURL(chainId) };
}

export function liveNetworkRows(
	view: NetView,
	m: SettingsMessages,
	expandedId?: string
): NetworkRowModel[] {
	return view.networks.map((row) => ({
		id: row.id,
		mark: chainMark(row.id, row.display_name, row.chain_id),
		name: row.display_name,
		meta: chainMeta(m, row.chain_id),
		badge: row.is_custom ? undefined : probePill(row.rpc_health, m),
		tag: row.is_custom ? m.networks.custom : undefined,
		removable: row.is_custom,
		expanded: expandedId === row.id
	}));
}

export function liveNetworkDetail(row: NetNetworkRow, m: SettingsMessages): NetworkDetailModel {
	const mismatch = row.rpc_chain_mismatch;
	const rpc: UrlFieldModel = {
		id: 'rpc',
		label: m.networks.rpcUrl,
		value: row.rpc_url,
		hint: m.networks.saveHint,
		badge: probePill(row.rpc_health, m),
		tone: mismatch !== null ? 'error' : 'default'
	};
	return {
		title: row.display_name,
		subtitle: `${chainMeta(m, row.chain_id)} · ${row.native_symbol}`,
		mark: chainMark(row.id, row.display_name, row.chain_id),
		name: row.display_name,
		note: row.is_custom ? m.networks.custom : m.networks.builtinNote,
		badge: probePill(row.rpc_health, m) ?? { tone: 'neutral', label: m.networks.online },
		rpc,
		explorer: {
			id: 'explorer',
			label: m.networks.explorer,
			value: row.explorer_url,
			badge: probePill(row.explorer_health, m)
		},
		callout:
			mismatch !== null
				? {
						tone: 'danger',
						text: fill(m.networks.mismatch, {
							reported: mismatch.reported_chain_id,
							expected: mismatch.expected_chain_id
						})
					}
				: undefined
	};
}

// ---------------------------------------------------------------------------
// Add-network wizard
// ---------------------------------------------------------------------------

function suggestionRow(entry: NetChainIndexEntry, m: SettingsMessages): NetworkRowModel {
	return {
		id: String(entry.chain_id),
		mark: chainMark('', entry.name, entry.has_logo ? entry.chain_id : undefined),
		name: entry.name,
		meta: chainMeta(m, entry.chain_id)
	};
}

export function liveAddNetwork(wizard: NetWizardView, m: SettingsMessages): AddNetworkModel {
	const base = {
		title: m.advanced.addNetworkTitle,
		subtitle: m.addNetwork.description,
		searchPlaceholder: m.addNetwork.searchPlaceholder
	};

	// Search phases: the list is the whole surface.
	if (wizard.phase === 'idle' || wizard.phase === 'searching' || wizard.phase === 'suggested') {
		return { ...base, results: wizard.suggestions.map((s) => suggestionRow(s, m)) };
	}

	const info = wizard.chain_info;
	const name = info?.name ?? '';
	const meta = info !== null ? chainMeta(m, info.chain_id) : '';
	const mark = chainMark('', name, info?.chain_id);

	// Resolving / checking: a candidate with the neutral "checking" badge.
	if (wizard.phase === 'resolving' || wizard.phase === 'checking') {
		return {
			...base,
			subtitle: info !== null ? `${name} · ${meta}` : m.addNetwork.searching,
			results: [],
			candidate: {
				mark,
				name,
				meta: m.addNetwork.checkingCompatibility,
				badge: { tone: 'neutral', label: m.addNetwork.compatibilityCheck, dot: true }
			}
		};
	}

	if (wizard.phase === 'error') {
		// The wizard stopped. The core says why as data; the words are ours —
		// and inconclusive is NEVER worded as incompatible (invariant ③). The
		// scan path now keeps the two apart too (spec 038 #E1): a probe that
		// failed is "unable to verify", with the re-check, and no setup tool.
		const inconclusive = wizard.error?.type === 'check_failed';
		return {
			...base,
			results: [],
			callout: {
				tone: 'warning',
				text: inconclusive ? m.addNetwork.unableToVerify : m.addNetwork.incompatibleHint
			},
			secondary: inconclusive ? undefined : m.addNetwork.openChainSetupTool,
			recheck: m.addNetwork.recheckWithRpc
		};
	}

	// Checked: a verdict — compatible, incompatible, or unverifiable.
	const compat = wizard.compat;
	const unverified = compat === null || compat.rpc_failure !== null;
	const compatible = compat !== null && compat.compatible && compat.rpc_failure === null;

	const checks: CheckItemModel[] =
		compat === null
			? []
			: [
					...compat.contracts.map((c) => ({ label: c.name, ok: c.deployed })),
					{ label: m.addNetwork.checkSigner, ok: compat.p256_available === true }
				];

	if (compatible) {
		return {
			...base,
			subtitle: `${name} · ${meta}`,
			results: [],
			candidate: {
				mark,
				name,
				meta:
					compat.best_rpc_latency_ms !== null
						? fill(m.addNetwork.bestRpc, { latencyMs: compat.best_rpc_latency_ms })
						: meta,
				badge: { tone: 'ok', label: m.addNetwork.compatible, dot: true }
			},
			checksTitle: m.addNetwork.compatibilityCheck,
			checks,
			customRpc: {
				id: 'custom-rpc',
				label: m.addNetwork.customRpcTitle,
				value: wizard.custom_rpc,
				placeholder: m.addNetwork.customRpcPlaceholder
			},
			primary: m.addNetwork.addNetworkBtn
		};
	}

	if (unverified) {
		return {
			...base,
			subtitle: `${name} · ${meta}`,
			results: [],
			candidate: {
				mark,
				name,
				meta: m.addNetwork.compatibilityCheck,
				badge: { tone: 'warn', label: m.addNetwork.unableToVerify, dot: true }
			},
			customRpc: {
				id: 'custom-rpc',
				label: m.addNetwork.customRpcTitle,
				value: wizard.custom_rpc,
				placeholder: m.addNetwork.customRpcPlaceholder
			},
			primary: m.addNetwork.retry,
			recheck: m.addNetwork.recheckWithRpc
		};
	}

	return {
		...base,
		subtitle: `${name} · ${meta}`,
		results: [],
		candidate: {
			mark,
			name,
			meta: m.addNetwork.compatibilityCheck,
			badge: { tone: 'error', label: m.addNetwork.incompatible, dot: true }
		},
		checksTitle: m.addNetwork.compatibilityCheck,
		checks,
		callout: { tone: 'warning', text: m.addNetwork.incompatibleHint },
		secondary: m.addNetwork.openChainSetupTool,
		recheck: m.addNetwork.recheckWithRpc
	};
}

// ---------------------------------------------------------------------------
// Providers + endpoints
// ---------------------------------------------------------------------------

const PROVIDER_NAMES = { alchemy: 'Alchemy', drpc: 'dRPC', ankr: 'Ankr' } as const;

export function liveRpcProviders(view: NetView, m: SettingsMessages): RpcProvidersModel {
	return {
		title: m.advanced.rpcProvidersTitle,
		subtitle: m.advanced.rpcProvidersSubtitle,
		description: m.rpcProviders.description,
		providers: view.providers.map((p) => {
			const test = p.test;
			return {
				id: p.provider,
				name: PROVIDER_NAMES[p.provider],
				badge: p.has_key
					? { tone: 'ok' as const, label: m.rpcProviders.connected, dot: true }
					: { tone: 'neutral' as const, label: m.rpcProviders.notSet, dot: true },
				field: {
					id: p.provider,
					label: '',
					value: p.key,
					placeholder: p.has_key ? undefined : m.rpcProviders.notSet
				},
				action: p.has_key ? m.rpcProviders.checkKey : m.rpcProviders.getKey,
				support:
					test !== null && test.done
						? fill(m.rpcProviders.supportsCount, { count: test.ok_count, total: test.total })
						: undefined,
				link: p.has_key ? undefined : `${m.rpcProviders.getKey} →`
			};
		})
	};
}

const ENDPOINT_COPY = {
	ethereum_data: { label: 'chainDataLabel', hint: 'chainDataHint' },
	passkey_index: { label: 'passkeyLabel', hint: 'passkeyHint' },
	bundler_service: { label: 'bundlerLabel', hint: 'bundlerHint' },
	fiat_rates: { label: 'fiatLabel', hint: 'fiatHint' }
} as const;

export function liveEndpoints(view: NetView, m: SettingsMessages): EndpointsModel {
	return {
		title: m.advanced.endpointsTitle,
		description: m.endpoints.description,
		fields: view.endpoints.map((e) => {
			const copy = ENDPOINT_COPY[e.field];
			return {
				id: e.field,
				label: m.endpoints[copy.label],
				value: e.value,
				placeholder: e.default_value,
				hint: m.endpoints[copy.hint],
				badge: servicePill(e.health, m)
			};
		}),
		reset: m.endpoints.reset,
		guide: m.endpoints.guide
	};
}

// ---------------------------------------------------------------------------
// The overlay — live sections over a fixture-built page model.
// ---------------------------------------------------------------------------

/**
 * Replace the network-owned sections of a built settings model with the
 * core's view. Identity, appearance, localization, storage and about stay
 * exactly as built — their machines are later features.
 */
export function withLiveNetworks(
	model: SettingsHomeModel,
	view: NetView,
	m: SettingsMessages,
	expandedId?: string
): SettingsHomeModel {
	const expanded = view.networks.find((row) => row.id === expandedId);
	return {
		...model,
		networks: { ...model.networks, rows: liveNetworkRows(view, m, expandedId) },
		networkDetail: expanded !== undefined ? liveNetworkDetail(expanded, m) : model.networkDetail,
		addNetwork: liveAddNetwork(view.wizard, m),
		rpcProviders: liveRpcProviders(view, m),
		endpoints: liveEndpoints(view, m)
	};
}

/** The desktop shape of the same overlay: `detail` rides inside `networks`. */
export function withLiveNetworksDesktop(
	model: SettingsDesktopModel,
	view: NetView,
	m: SettingsMessages,
	expandedId?: string
): SettingsDesktopModel {
	const expanded = view.networks.find((row) => row.id === expandedId);
	return {
		...model,
		networks: {
			...model.networks,
			rows: liveNetworkRows(view, m, expandedId),
			detail: expanded !== undefined ? liveNetworkDetail(expanded, m) : model.networks.detail
		},
		addNetwork: liveAddNetwork(view.wizard, m),
		rpcProviders: liveRpcProviders(view, m),
		endpoints: liveEndpoints(view, m)
	};
}

/**
 * The display-currency overlay (phase 5): the localization row shows the
 * committed code and the sheet marks it selected. The value's sample amount
 * is presentation the fixture already words; with no rate source yet the row
 * shows the code alone — honest, not a mocked conversion.
 */
export function withLiveCurrency(
	model: SettingsHomeModel,
	view: CurrencyView,
	catalog?: LiveCurrencyCatalog
): SettingsHomeModel {
	return {
		...model,
		sections: model.sections.map((section) => ({
			...section,
			// The phone's row says the code alone — the drawn ST1 shape, and the
			// width a phone row has; the desktop's row carries the sample too.
			rows: section.rows.map((row) => (row.id === 'currency' ? { ...row, value: view.code } : row))
		})),
		currencySheet: {
			...model.currencySheet,
			rows: liveCurrencyRows(model.currencySheet.rows, view, catalog)
		}
	};
}

/** The provider-driven list, when one has answered (spec 028 Phase 9, T491). */
export interface LiveCurrencyCatalog {
	/** USD first, then what the rate sources can price. Empty ⇒ the drawn list stands. */
	codes: string[];
	/** For the browser's currency names. */
	locale: string;
}

/**
 * "USD · $1,234.56" — the code and a sample in it, once a rate is committed;
 * the code alone while the currency cannot be priced (024's rule: a defaulted
 * 1 under a ¥ is a lie).
 */
function currencyRowValue(view: CurrencyView): string {
	return view.rate === null ? view.code : `${view.code} · ${moneyText(1234.56, view)}`;
}

function liveCurrencyRows(
	drawn: SelectRowModel[],
	view: CurrencyView,
	catalog?: LiveCurrencyCatalog
): SelectRowModel[] {
	if (catalog === undefined || catalog.codes.length === 0) {
		return drawn.map((row) => ({ ...row, selected: row.id === view.code }));
	}
	return catalog.codes.map((code) => ({
		id: code,
		label: code,
		glyph: currencyGlyph(code),
		caption:
			currencyDisplayName(code, catalog.locale) ?? drawn.find((row) => row.id === code)?.caption,
		selected: code === view.code
	}));
}

/** DST3's 货币 row, live: the committed value, and the menu it opens. */
export function withLiveCurrencyDesktop(
	model: SettingsDesktopModel,
	view: CurrencyView,
	catalog?: LiveCurrencyCatalog
): SettingsDesktopModel {
	const drawn = model.dropdown?.rowId === 'currency' ? model.dropdown.rows : [];
	return {
		...model,
		localization: {
			...model.localization,
			rows: model.localization.rows.map((row) =>
				row.id === 'currency'
					? {
							...row,
							value: currencyRowValue(view),
							options: liveCurrencyRows(drawn, view, catalog)
						}
					: row
			)
		}
	};
}

/**
 * The connected sites, in the drawn storage row (spec 027 T350).
 *
 * 023 drew this group with one fixture row — "Connected dApps · 4 sites" and a
 * destructive "Disconnect all". A grant is a standing permission, so a person
 * has to be able to see WHICH sites hold one, not only how many; this fills the
 * same drawn rows with one per site, keeping the group action as the way to cut
 * them all off at once.
 *
 * `id` is the origin, so the row's own `onclear` says exactly which grant to
 * revoke. Off the extension there are no grants and the fixture row stands.
 */
export function withLiveConnections<M extends { storage: SettingsHomeModel['storage'] }>(
	model: M,
	grants: { origin: string; address: string }[],
	m: SettingsMessages
): M {
	const groups = model.storage.groups.map((group) => {
		if (group.label !== m.storage.connections) return group;
		if (grants.length === 0) {
			return {
				...group,
				items: [
					{
						id: 'dapps',
						label: m.storage.itemDapps,
						meta: fill(m.storage.sitesCount, { count: 0 }),
						action: m.storage.disconnectAll,
						destructive: true
					}
				]
			};
		}
		return {
			...group,
			items: grants.map((grant) => ({
				id: grant.origin,
				label: hostOf(grant.origin),
				meta: shortenAddress(grant.address),
				// Singular: this row cuts off ONE site. Saying "Disconnect all" on
				// a row that disconnects one is the kind of label that gets tapped
				// by someone who meant something else.
				action: m.storage.disconnectOne,
				destructive: true
			}))
		};
	});
	return { ...model, storage: { ...model.storage, groups } };
}

function hostOf(origin: string): string {
	try {
		return new URL(origin).host || origin;
	} catch {
		return origin;
	}
}

// ---------------------------------------------------------------------------
// Preferences — the rows that have never done anything (spec 028 T431/T432)
// ---------------------------------------------------------------------------

/** The theme segment ids the mocks drew, against the values we store. */
const THEME_SEGMENT: Record<string, ThemeChoice> = {
	light: 'light',
	dark: 'dark',
	auto: 'system'
};

const THEME_ID: Record<ThemeChoice, string> = { light: 'light', dark: 'dark', system: 'auto' };

/** A mock segment id → what `preferences` is told. Unknown ids change nothing. */
export function themeFromSegment(id: string): ThemeChoice | undefined {
	return THEME_SEGMENT[id];
}

function noteFor(
	noteKey: FormatOption<string>['noteKey'],
	m: SettingsMessages
): string | undefined {
	switch (noteKey) {
		case 'system':
			// The fixture's own wording for the first row, kept verbatim so the
			// live sheet reads as the drawn one.
			return `${m.common.automatic} · ${m.common.system}`;
		case 'indian':
			return m.formatNote.indian;
		case 'h24':
			return m.formatNote.h24;
		case 'h12':
			return m.formatNote.h12;
		default:
			return undefined;
	}
}

/**
 * One format sheet: every preset, its live example as the label, the chosen one
 * ticked.
 *
 * The examples are GENERATED rather than the fixture's canon strings, because
 * the label of a format option has to be that format doing its job — a row
 * reading "1,234,567.89" while the app groups differently is the picker lying
 * about what it will do.
 */
function formatSheet<K extends string>(
	sheet: SelectSheetModel,
	options: FormatOption<K>[],
	current: K,
	m: SettingsMessages
): SelectSheetModel {
	return {
		...sheet,
		rows: options.map((option) => ({
			id: option.key,
			label: option.example,
			mono: true,
			note: noteFor(option.noteKey, m),
			selected: option.key === current
		}))
	};
}

/** What the localization list shows beside each row: the current example. */
function currentExamples(): { number: string; date: string; time: string } {
	return {
		number: formatNumber(1234567.89, { minimumFractionDigits: 2, maximumFractionDigits: 2 }),
		date: formatDate(FORMAT_SAMPLE),
		time: formatTime(FORMAT_SAMPLE)
	};
}

/** The same sample the pickers use, so a row and its sheet agree. */
const FORMAT_SAMPLE = new Date(2026, 5, 13, 13, 45);

/**
 * The language choices with the ACTIVE one ticked (spec 038 #E5). On the web
 * a locale is a route: the page is in `locale` whatever the stored choice
 * says, and a row ticked for a language the person is not reading is a lie —
 * the founder saw 简体中文 ticked on `/en`. "Follow system" is ticked only
 * while nothing is pinned; choosing a language navigates (settings page).
 */
function liveLanguageRows(m: SettingsMessages, locale: string): SelectRowModel[] {
	return languageRows(m, locale).map((row) => ({
		...row,
		selected: preferences.language === 'auto' ? row.id === 'system' : row.id === locale
	}));
}

/** The slider at the stored stop. Six stops, whatever a board drew. */
function liveTextScale<T extends { steps: number; index: number }>(scale: T): T {
	return { ...scale, steps: TEXT_SCALE_LEVELS.length, index: preferences.textScaleIndex };
}

/**
 * The preference rows, wired to what is actually stored (spec 028 T432).
 *
 * Everything here was drawn in 023 and inert until now: the theme and avatar
 * segments showed a fixed selection, and the three format sheets showed five
 * canon strings with the first one ticked no matter what the app did. This
 * overlay makes each of them show — and offer — the truth.
 */
export function withLivePreferences(
	model: SettingsHomeModel,
	m: SettingsMessages,
	languageValue: string,
	locale: string
): SettingsHomeModel {
	const examples = currentExamples();
	const value: Record<string, string> = {
		'number-format': examples.number,
		'date-format': examples.date,
		'time-format': examples.time,
		language: languageValue
	};
	return {
		...model,
		sections: model.sections.map((section) => ({
			...section,
			rows: section.rows.map((row) =>
				value[row.id] === undefined ? row : { ...row, value: value[row.id] }
			)
		})),
		appearance: {
			...model.appearance,
			theme: { ...model.appearance.theme, selected: THEME_ID[preferences.theme] },
			avatar: { ...model.appearance.avatar, selected: preferences.avatarStyle },
			textScale: liveTextScale(model.appearance.textScale)
		},
		languageSheet: {
			...model.languageSheet,
			rows: liveLanguageRows(m, locale)
		},
		numberSheet: formatSheet(model.numberSheet, numberFormatOptions(), preferences.numberFormat, m),
		dateSheet: formatSheet(model.dateSheet, dateFormatOptions(), preferences.dateFormat, m),
		timeSheet: formatSheet(model.timeSheet, timeFormatOptions(), preferences.timeFormat, m)
	};
}

/**
 * The desktop's own two panels (spec 028 T433).
 *
 * Its localization rows are `FormRowModel`s with a dropdown each, so the OPTIONS
 * ride on the row — the fixture's single pinned-open `dropdown` stays exactly
 * what it is (the DST3 board), and a live row carries the menu it opens.
 */
export function withLivePreferencesDesktop(
	model: SettingsDesktopModel,
	m: SettingsMessages,
	languageValue: string,
	locale: string
): SettingsDesktopModel {
	const examples = currentExamples();
	const empty: SelectSheetModel = { title: '', rows: [] };
	const menus: Record<string, SelectRowModel[]> = {
		'number-format': formatSheet(empty, numberFormatOptions(), preferences.numberFormat, m).rows,
		'date-format': formatSheet(empty, dateFormatOptions(), preferences.dateFormat, m).rows,
		'time-format': formatSheet(empty, timeFormatOptions(), preferences.timeFormat, m).rows
	};
	const values: Record<string, string> = {
		'number-format': examples.number,
		'date-format': examples.date,
		'time-format': examples.time
	};
	return {
		...model,
		appearance: {
			...model.appearance,
			// The language row is a dropdown like the localization rows, and like
			// them it carries the menu it opens; choosing one navigates (the page).
			language: {
				...model.appearance.language,
				value: languageValue,
				options: liveLanguageRows(m, locale)
			},
			textScale: {
				...model.appearance.textScale,
				scale: liveTextScale(model.appearance.textScale.scale)
			},
			theme: {
				...model.appearance.theme,
				segmented: { ...model.appearance.theme.segmented, selected: THEME_ID[preferences.theme] }
			},
			avatar: {
				...model.appearance.avatar,
				segmented: {
					...model.appearance.avatar.segmented,
					selected: preferences.avatarStyle
				}
			}
		},
		localization: {
			...model.localization,
			rows: model.localization.rows.map((row) =>
				values[row.id] === undefined
					? row
					: { ...row, value: values[row.id], options: menus[row.id] }
			)
		}
	};
}

/**
 * The failure the erase can have, said where the person is looking.
 *
 * The sheet already draws a danger callout — the one listing what is lost — so
 * a failed erase replaces that text rather than growing a second surface. The
 * distinction it preserves is the whole reason `EraseIncompleteError` exists:
 * data is still here, and the person is still signed in.
 */
export function withEraseFailure(
	model: SettingsHomeModel,
	m: SettingsMessages,
	failed: boolean
): SettingsHomeModel {
	if (!failed) return model;
	return {
		...model,
		eraseSheet: {
			...model.eraseSheet,
			callout: { tone: 'danger', text: m.erase.failed }
		}
	};
}

// ---------------------------------------------------------------------------
// The account switcher (spec 028 Phase 8) — the session's rows, the balance
// core's totals.
// ---------------------------------------------------------------------------

export interface LiveAccountsInput {
	/** The session's own rows, in the session's order (`SwitchAccount.index`). */
	rows: SessionAccountRow[];
	activeIndex: number;
	/** Per-account totals in USD by lowercased address — the balance core's switcher cache. */
	balances: ReadonlyMap<string, number>;
	currency: CurrencyView;
	identicon: (address: string, name: string) => string;
}

function liveAccountRows(input: LiveAccountsInput) {
	return input.rows.map((row, position) => {
		const usd = input.balances.get(row.account.address.toLowerCase());
		return {
			name: row.account.name,
			addressDisplay: shortenAddress(row.account.address),
			addressFull: row.account.address,
			identiconSvg: input.identicon(row.account.address, row.account.name),
			// No cached total yet: an empty cell, never a mocked figure.
			amount: usd === undefined ? '' : moneyText(usd, input.currency),
			selected: position === input.activeIndex
		};
	});
}

/** "3 accounts · $3,262.40 total" over what is actually known. */
function liveAccountsSummary(input: LiveAccountsInput, m: SettingsMessages['accounts']): string {
	let total = 0;
	for (const row of input.rows) total += input.balances.get(row.account.address.toLowerCase()) ?? 0;
	return `${fill(m.countPrefix, { count: input.rows.length })}${fill(m.total, {
		amount: moneyText(total, input.currency)
	})}`;
}

/**
 * The switcher as one sheet, from nothing but the session and the balance
 * core — for the header's account button (founder call, 2026-09-05), which
 * opens it over the wallet and the address book, where no settings model is
 * standing underneath to overlay.
 */
export function liveAccountsSheet(
	input: LiveAccountsInput,
	m: SettingsMessages['accounts']
): AccountsSheetModel {
	return {
		title: m.title,
		summary: liveAccountsSummary(input, m),
		rows: liveAccountRows(input),
		primary: m.createNew,
		secondary: m.signInExisting
	};
}

/**
 * The phone's account sheet (ST2), live: every account this browser is signed
 * into, the active one checked, each with the total the balance core has
 * cached for it. The rows were fixture data until now — `identity.ts` swapped
 * only the active row's name and address, and a tap did nothing.
 */
export function withLiveAccounts(
	model: SettingsHomeModel,
	input: LiveAccountsInput,
	m: SettingsMessages
): SettingsHomeModel {
	return {
		...model,
		accountsSheet: {
			...model.accountsSheet,
			summary: liveAccountsSummary(input, m.accounts),
			rows: liveAccountRows(input)
		}
	};
}

/** DST1's account page, likewise. */
export function withLiveAccountsDesktop(
	model: SettingsDesktopModel,
	input: LiveAccountsInput,
	m: SettingsMessages
): SettingsDesktopModel {
	return {
		...model,
		account: {
			...model.account,
			summary: liveAccountsSummary(input, m.accounts),
			rows: liveAccountRows(input)
		}
	};
}

// ---------------------------------------------------------------------------
// Device storage (spec 028 Phase 8) — what is actually held, per drawn row.
// ---------------------------------------------------------------------------

function sizeText(bytes: number): string {
	const { amount, unit } = formatBytes(bytes);
	return `${amount} ${unit}`;
}

/** The drawn rows that describe something this client does not have. */
const WEB_HAS_NO: Partial<Record<StorageItemId, true>> = { browsing: true };

/** The row's meta line: the fixture's own shape, with the measured figures. */
function storageItemMeta(
	id: StorageItemId,
	report: DeviceStorageReport,
	m: SettingsMessages
): string {
	const item = report.items[id];
	const size = sizeText(item.bytes);
	switch (id) {
		case 'transactions':
		case 'browsing':
			return `${fill(m.storage.records, { count: item.count })} · ${size}`;
		case 'contacts':
			return `${fill(m.storage.contactsCount, { count: item.count })} · ${size}`;
		case 'custom':
			return `${fill(m.storage.itemsCount, { count: item.count })} · ${size}`;
		case 'dapps':
			return fill(m.storage.sitesCount, { count: item.count });
		default:
			return size;
	}
}

/**
 * ST13 / DST7 with this device's numbers: the headline, the bar's shares and
 * every row's meta line come from `measureDeviceStorage`. The connections
 * row keeps what `withLiveConnections` wrote (the grants are its), so this
 * runs before it.
 */
export function withLiveStorage<M extends { storage: SettingsHomeModel['storage'] }>(
	model: M,
	report: DeviceStorageReport,
	m: SettingsMessages
): M {
	const { amount, unit } = formatBytes(report.totalBytes);
	const total = report.totalBytes;
	const share = (bytes: number) => (total === 0 ? 0 : bytes / total);
	const segmentGroup: Record<string, 'user' | 'cache' | 'sessions'> = {
		user: 'user',
		cache: 'cache',
		sessions: 'sessions'
	};
	const isItem = (id: string): id is StorageItemId =>
		(STORAGE_ITEM_IDS as readonly string[]).includes(id);
	return {
		...model,
		storage: {
			...model.storage,
			amount,
			unit,
			summary: fill(m.storage.summary, { count: report.keyCount }),
			segments: model.storage.segments.map((segment) => {
				const group = segmentGroup[segment.id];
				return group === undefined
					? segment
					: { ...segment, fraction: share(report.groups[group]) };
			}),
			groups: model.storage.groups.map((group) => ({
				...group,
				// 浏览记录 is the phone's: the web has no in-app browser (spec 022),
				// so its key list is empty by construction and the row would only
				// ever say "0 records" about a thing that cannot exist here.
				items: group.items
					.filter((item) => !WEB_HAS_NO[item.id as StorageItemId])
					.map((item) =>
						isItem(item.id) && GROUP_OF_ITEM[item.id] !== 'sessions'
							? { ...item, meta: storageItemMeta(item.id, report, m) }
							: item
					)
			}))
		}
	};
}

// ---------------------------------------------------------------------------
// The rescue sheets (spec 028 Phase 8): SR2 RPC fix and SR3 balance detail
// over the wallet, SR4 relayer treasury over the send. Drawn as settings
// components; every figure here is the core's, every word the corpus's.
// ---------------------------------------------------------------------------

/** The slice of the settings corpus the wallet route's rescues speak. */
export interface RescueMessages {
	rescue: SettingsMessages['rescue'];
	balanceDetail: SettingsMessages['balanceDetail'];
	relayer: SettingsMessages['relayer'];
	networks: Pick<
		SettingsMessages['networks'],
		'chainId' | 'online' | 'slow' | 'offline' | 'mismatch'
	>;
	addNetwork: Pick<SettingsMessages['addNetwork'], 'checkingCompatibility'>;
	common: Pick<SettingsMessages['common'], 'done' | 'close'>;
}

/** Only what the sheets read, so the wallet page carries no more corpus than it needs. */
export function pickRescueMessages(m: SettingsMessages): RescueMessages {
	return {
		rescue: m.rescue,
		balanceDetail: m.balanceDetail,
		relayer: m.relayer,
		networks: {
			chainId: m.networks.chainId,
			online: m.networks.online,
			slow: m.networks.slow,
			offline: m.networks.offline,
			mismatch: m.networks.mismatch
		},
		addNetwork: { checkingCompatibility: m.addNetwork.checkingCompatibility },
		common: { done: m.common.done, close: m.common.close }
	};
}

/** Where a working endpoint comes from — the drawn four. */
const RPC_PROVIDER_LINKS = [
	{ label: 'Alchemy', href: 'https://alchemy.com' },
	{ label: 'QuickNode', href: 'https://quicknode.com' },
	{ label: 'dRPC', href: 'https://drpc.org' },
	{ label: 'Chainlist', href: 'https://chainlist.org' }
];

function rescueMark(chainId: number): ChainMarkModel {
	return chainMark('', chainName(chainId), chainId);
}

function rescueLatencyPill(ms: number, m: RescueMessages): StatusPillModel {
	return {
		tone: ms >= 1000 ? 'warn' : 'ok',
		label: ms >= 1000 ? `${m.networks.slow} · ${(ms / 1000).toFixed(1)}s` : `${ms}ms`,
		dot: true
	};
}

export interface LiveRpcFixInput {
	row: NetNetworkRow;
	/** The URL being typed, until it is saved. */
	draft: string | null;
	/** A save went to the core from this sheet; its probe then decides "restored". */
	saved: boolean;
}

/**
 * SR2 for one network. Failing until a URL saved HERE probes healthy — the
 * row's stored health alone does not restore it, because the sheet opened
 * on a chain the balance core could not read.
 */
export function liveRpcFix(input: LiveRpcFixInput, m: RescueMessages): RpcFixModel {
	const { row, draft, saved } = input;
	const health = row.rpc_health;
	const settled = saved && draft === null;
	const restored = settled && health !== null && health.type === 'ok';
	const checking = settled && health !== null && health.type === 'checking';
	const mismatch = row.rpc_chain_mismatch;

	let badge: StatusPillModel;
	if (health !== null && health.type === 'ok' && restored) {
		badge = rescueLatencyPill(health.latency_ms, m);
	} else if (checking) {
		badge = { tone: 'neutral', label: m.addNetwork.checkingCompatibility, dot: true };
	} else {
		badge = { tone: 'error', label: m.networks.offline, dot: true };
	}

	return {
		title: m.rescue.rpcFixTitle,
		mark: chainMark(row.id, row.display_name, row.chain_id),
		name: row.display_name,
		meta: `${fill(m.networks.chainId, { chainId: row.chain_id })} · ${row.native_symbol}`,
		badge,
		callout:
			mismatch !== null
				? {
						tone: 'danger',
						text: fill(m.networks.mismatch, {
							reported: mismatch.reported_chain_id,
							expected: mismatch.expected_chain_id
						})
					}
				: restored
					? { tone: 'success', text: m.rescue.rpcFixRestored, icon: 'check' }
					: { tone: 'warning', text: m.rescue.rpcFixWarning },
		field: {
			id: 'rpc',
			label: m.rescue.rpcFixLabel,
			value: draft ?? row.rpc_url,
			badge: restored ? badge : undefined,
			tone: restored ? 'success' : 'error'
		},
		primary: restored ? m.common.done : m.rescue.rpcFixSaveBtn,
		providersLabel: restored ? undefined : m.rescue.rpcProvidersTitle,
		providers: restored ? undefined : RPC_PROVIDER_LINKS,
		report: restored ? undefined : m.rescue.rpcReport
	};
}

/**
 * SR3: the balance by network — the chains still being read (rate-limited,
 * quietly retrying) or unreachable (with a retry), and the chains that
 * settled, largest first. The same figures the hero sums.
 */
export function liveBalanceDetail(
	view: BalanceView,
	currency: CurrencyView,
	m: RescueMessages,
	/** The hero's own sentence, reused as the third section's title (spec 038 #E8). */
	unpricedLabel: string
): BalanceDetailModel {
	const pending: BalanceDetailModel['pending'] = view.rate_limited_chain_ids.map((id) => ({
		id: String(id),
		mark: rescueMark(id),
		name: chainName(id),
		status: m.balanceDetail.statusRetrying,
		tone: 'neutral'
	}));
	for (const id of view.banner_chain_ids) {
		if (pending.some((row) => row.id === String(id))) continue;
		pending.push({
			id: String(id),
			mark: rescueMark(id),
			name: chainName(id),
			status: m.balanceDetail.statusFailed,
			tone: 'error',
			action: m.balanceDetail.retry
		});
	}

	const perChain = new Map<number, number>();
	for (const token of view.tokens) {
		const usd = Number(token.balance) * (token.price_usd ?? 0);
		if (!Number.isFinite(usd)) continue;
		perChain.set(token.chain_id, (perChain.get(token.chain_id) ?? 0) + usd);
	}
	const done = [...perChain.entries()]
		.filter(([id]) => !pending.some((row) => row.id === String(id)))
		.sort((a, b) => b[1] - a[1])
		.map(([id, usd]) => ({
			id: String(id),
			mark: rescueMark(id),
			name: chainName(id),
			amount: view.hidden ? MASK : moneyText(usd, currency)
		}));

	const total = view.display_total_usd ?? view.cached_total_usd;
	return {
		title: m.balanceDetail.title,
		summary: fill(m.balanceDetail.total, {
			amount: view.hidden || total === null ? MASK : moneyText(total, currency)
		}),
		sectionPending: m.balanceDetail.networksLabel,
		pendingNote: m.balanceDetail.networksNote,
		pending,
		sectionDone: m.balanceDetail.updatedLabel,
		done,
		// The hero's "some tokens couldn't be priced" is a link to this dialog,
		// and the dialog must answer it by name (spec 038 #E8).
		sectionUnpriced: unpricedLabel,
		unpriced: view.unpriced_tokens.map((token) => ({
			id: `${token.chain_id}:${token.token_address ?? token.symbol}`,
			mark: rescueMark(token.chain_id),
			name: token.symbol,
			detail: `${chainName(token.chain_id)} · ${view.hidden ? MASK : trimBalance(token.balance)}`
		}))
	};
}

/**
 * How much the treasury is short, worded in the asset's own units. The core
 * carries base units as decimal strings; a value that already has a point is
 * taken as human decimal (the older `TreasuryStatus` shape).
 */
/**
 * The smallest top-up worth asking a person for, in whole native coin.
 *
 * The relay's own float floor is two orders of magnitude lower, and asking for
 * exactly the shortfall to it — 0.0001 — buys a relayer that drops back under
 * the floor after roughly one operation, so the same sheet reappears. A round
 * 0.01 is a contribution that actually starts the thing (spec 060).
 */
const MIN_TOP_UP_SUGGESTION = '0.01';

/** The larger of the real shortfall and {@link MIN_TOP_UP_SUGGESTION}. */
function topUpSuggestion(floor: string, balance: string, decimals: number): string {
	const shortfall = shortfallText(floor, balance, decimals);
	const asNumber = Number(shortfall);
	return Number.isFinite(asNumber) && asNumber > Number(MIN_TOP_UP_SUGGESTION)
		? shortfall
		: MIN_TOP_UP_SUGGESTION;
}

function shortfallText(floor: string, balance: string, decimals: number): string {
	if (floor.includes('.') || balance.includes('.')) {
		const diff = Number(floor) - Number(balance);
		return trimBalance((Number.isFinite(diff) && diff > 0 ? diff : 0).toString());
	}
	let units: bigint;
	try {
		const f = BigInt(floor);
		const b = BigInt(balance);
		units = f > b ? f - b : 0n;
	} catch {
		units = 0n;
	}
	const digits = units.toString().padStart(decimals + 1, '0');
	const whole = digits.slice(0, digits.length - decimals);
	const frac = digits.slice(digits.length - decimals);
	return trimBalance(decimals === 0 ? whole : `${whole}.${frac}`);
}

/**
 * SR4: fund this chain's relay treasury. Every figure is the send core's
 * probe (`treasury_bootstrap`); the code encodes the treasury's real address.
 */
export function liveRelayer(status: SendTreasuryStatus, m: RescueMessages): RelayerModel {
	const pathUsd = status.asset === 'path_usd';
	const decimals = pathUsd ? TEMPO_FEE_TOKEN_DECIMALS : 18;
	const symbol = pathUsd ? 'pathUSD' : (chainInfo(status.chain_id)?.nativeSymbol ?? '');
	// WHO can fix this is the core's verdict (`operator_served`), not a guess
	// from the chain id here. On a network Vela ships the operator owns that
	// relayer; on one the person added — a devnet, an internal chain — there
	// may be nobody else who could hold gas on it (spec 060).
	// Whether to offer it is the decision; WHERE it goes is the component's, as
	// it already is for the feedback link (`FeedbackBody.svelte`).
	const report = status.operator_served
		? { label: m.relayer.reportBtn, selfFundLabel: m.relayer.selfFundToggle }
		: undefined;
	return {
		title: m.relayer.title,
		report,
		lead: status.operator_served ? m.relayer.operatorLead : m.relayer.customLead,
		mark: rescueMark(status.chain_id),
		name: chainName(status.chain_id),
		amountHint: fill(m.relayer.amountHint, {
			amount: topUpSuggestion(status.floor, status.balance, decimals),
			symbol
		}),
		qrCaption: m.relayer.addressLabel,
		addressDisplay: shortenAddress(status.address),
		address: status.address,
		code: encodeQr(status.address),
		copyLabel: m.relayer.copyBtn,
		callout: { tone: 'warning', text: m.relayer.disclaimer },
		primary: m.relayer.retryBtn
	};
}

// ---------------------------------------------------------------------------
// The Ethereum backup row (spec 062 §5a)
// ---------------------------------------------------------------------------

/** The row for a state, or `undefined` when there is nothing to draw:
 *  no registry on Ethereum (the feature is dark) or no record to back up. */
export function ethereumBackupRow(
	state: EthereumBackupState | 'checking',
	m: SettingsMessages
): EthereumBackupRowModel | undefined {
	switch (state) {
		case 'checking':
			return {
				title: m.backup.title,
				subtitle: m.backup.checking,
				tone: 'neutral',
				actionable: false
			};
		case 'backed_up':
			return {
				title: m.backup.title,
				subtitle: m.backup.backedUp,
				tone: 'positive',
				actionable: false
			};
		case 'not_backed_up':
			return {
				title: m.backup.title,
				subtitle: m.backup.notBackedUp,
				tone: 'caution',
				actionable: true
			};
		case 'could_not_check':
			return {
				title: m.backup.title,
				subtitle: m.backup.couldNotCheck,
				tone: 'neutral',
				actionable: false
			};
		case 'unavailable':
		case 'not_registered':
			return undefined;
	}
}

/** The phone: the row joins the first group, after the address book. */
export function withLiveEthereumBackup<M extends { sections: SettingsHomeModel['sections'] }>(
	model: M,
	state: EthereumBackupState | 'checking',
	m: SettingsMessages
): M {
	const row = ethereumBackupRow(state, m);
	if (row === undefined) return model;
	const settingsRow = {
		id: 'ethereum-backup',
		icon: 'upload' as const,
		title: row.title,
		subtitle: row.subtitle,
		trailing: row.actionable ? ('chevron' as const) : ('none' as const)
	};
	return {
		...model,
		sections: model.sections.map((section, index) =>
			index === 0 ? { ...section, rows: [...section.rows, settingsRow] } : section
		)
	};
}
