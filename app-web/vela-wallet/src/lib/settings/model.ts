/**
 * Settings view models (spec 023, `design/settings/`).
 *
 * Display-ready shapes only — no preference store, no RPC probing, no storage
 * accounting. Every number and every URL on these screens arrives as a
 * pre-formatted string from the fixture layer, exactly as spec 015/018 did,
 * so the later "wire real settings state" feature replaces `fixtures.ts` and
 * nothing else.
 *
 * The ~40 mocks are a small vocabulary re-dealt: a row with a leading glyph, a
 * segmented control, a select list, a status pill, a callout, a mono URL field
 * and a confirm sheet cover almost all of them. The types below name that
 * vocabulary once; the screens are compositions.
 */
import type { QrCode } from '$lib/wallet/qr';
import type { UtilityIconId } from '$lib/wallet/icons';

/** Mobile mocks ST1–ST16 plus the SR1–SR5 rescue set. */
export type MobileSettingsStateId =
	| 'st1'
	| 'st1b'
	| 'st2'
	| 'st3'
	| 'st3b'
	| 'st4'
	| 'st5'
	| 'st6'
	| 'st7'
	| 'st8'
	| 'st9'
	| 'st9b'
	| 'st10'
	| 'st10b'
	| 'st10c'
	| 'st11'
	| 'st12'
	| 'st13'
	| 'st13b'
	| 'st14'
	| 'st15'
	| 'st16'
	| 'sr1'
	| 'sr2'
	| 'sr2b'
	| 'sr3'
	| 'sr4'
	| 'sr5';

/** Desktop mocks DST1–DST8 (+ DST4b dialog) and the DSR1 rescue dialog. */
export type DesktopSettingsStateId =
	'dst1' | 'dst2' | 'dst3' | 'dst4' | 'dst4b' | 'dst5' | 'dst6' | 'dst7' | 'dst8' | 'dsr1';

export const MOBILE_SETTINGS_STATES: MobileSettingsStateId[] = [
	'st1',
	'st1b',
	'st2',
	'st3',
	'st3b',
	'st4',
	'st5',
	'st6',
	'st7',
	'st8',
	'st9',
	'st9b',
	'st10',
	'st10b',
	'st10c',
	'st11',
	'st12',
	'st13',
	'st13b',
	'st14',
	'st15',
	'st16',
	'sr1',
	'sr2',
	'sr2b',
	'sr3',
	'sr4',
	'sr5'
];

export const DESKTOP_SETTINGS_STATES: DesktopSettingsStateId[] = [
	'dst1',
	'dst2',
	'dst3',
	'dst4',
	'dst4b',
	'dst5',
	'dst6',
	'dst7',
	'dst8',
	'dsr1'
];

/**
 * Which sub-screen the settings surface is showing.
 *
 * On the phone these are pushed pages (ST9…ST14); on the desktop they are the
 * second-level nav's panels (DST1…DST8), which is why one id set serves both.
 * `home` is the phone's list — the desktop has no equivalent, because its nav
 * column IS the list.
 */
export type SettingsPageId =
	| 'home'
	| 'account'
	| 'appearance'
	| 'localization'
	| 'networks'
	| 'network-detail'
	| 'add-network'
	| 'rpc-providers'
	| 'endpoints'
	| 'storage'
	/** Spec 068 — the stored default transaction speed (desktop page; on the
	 *  phone the same preference is a row that opens a sheet). */
	| 'fee-speed'
	/**
	 * Spec 081 FR-016 — the report, as a desktop panel. The phone opens the
	 * same body in a sheet; a wide layout has no sheets (founder, 2026-09-05),
	 * so the nav gains a destination rather than the panel gaining a modal.
	 */
	| 'feedback'
	| 'about';

/**
 * Which modal is open over the current page. The phone draws all of these as
 * bottom sheets; the desktop draws `add-network` and `rpc-fix` as centred
 * dialogs and the pickers as anchored dropdowns (SPEC 设置·桌面, 形态).
 */
export type SettingsOverlayId =
	| 'none'
	| 'accounts'
	| 'sign-out'
	| 'language'
	| 'currency'
	| 'number-format'
	| 'date-format'
	| 'time-format'
	/** Spec 068: the stored default transaction speed. */
	| 'fee-speed'
	/** Spec 071: the Trusted Signer's page. */
	| 'signer-page'
	/** Spec 075: the tunnel a cross-device pairing goes through. */
	| 'tunnel-page'
	| 'clear-caches'
	/** Spec 058: one storage row's Clear, asked before it happens. */
	| 'clear-storage-item'
	/** Spec 072: removing a custom network, asked before it happens. */
	| 'remove-network'
	/** Spec 072: resetting the service endpoints, asked before it happens. */
	| 'reset-endpoints'
	| 'erase-device'
	| 'feedback'
	| 'add-network'
	| 'rpc-fix'
	| 'balance-detail'
	| 'relayer';

/** Status-pill tone. `neutral` is the unset/idle dot the mocks grey out. */
export type StatusTone = 'ok' | 'warn' | 'error' | 'neutral' | 'accent';

export interface StatusPillModel {
	tone: StatusTone;
	label: string;
	/** A leading dot; the compatibility badges keep it, plain latency drops it. */
	dot?: boolean;
}

/** Callout tone. Maps to the four soft/base colour pairs in the token set. */
export type CalloutTone = 'warning' | 'danger' | 'info' | 'success';

export interface CalloutModel {
	tone: CalloutTone;
	text: string;
	/** Overrides the tone's default glyph (success uses a check, not a triangle). */
	icon?: UtilityIconId;
}

/** Row emphasis. `danger` is the red 退出登录/清理数据 family. */
export type RowTone = 'default' | 'accent' | 'danger';

/** What sits at the end of a settings row. */
export type RowTrailing = 'chevron' | 'external' | 'none';

export interface SettingsRowModel {
	id: string;
	icon?: UtilityIconId;
	title: string;
	subtitle?: string;
	/** Right-aligned current value — "简体中文 · 系统", "12 个网络". */
	value?: string;
	trailing?: RowTrailing;
	tone?: RowTone;
	badge?: StatusPillModel;
}

export interface SettingsSectionModel {
	/** Section label. Absent for the un-labelled first block of ST1. */
	label?: string;
	/** ST1b: 高级 is a disclosure, and it remembers being open. */
	collapsible?: boolean;
	collapsed?: boolean;
	rows: SettingsRowModel[];
	/**
	 * ST1: the appearance block ends in three CONTROLS rather than rows — the
	 * text-size slider and the two segmented pickers. Marking the section says
	 * so in the data, instead of the screen counting indices.
	 */
	appearanceControls?: boolean;
}

/** ST1's account block: avatar + name + address + a trailing text action. */
export interface AccountRowModel {
	name: string;
	addressDisplay: string;
	/** The seed of the artwork — what the identicon viewer shows beside it. */
	addressFull: string;
	/** Inline identicon SVG from vela-core, seeded by the full address. */
	identiconSvg: string;
	/** Trailing text action — "切换账户". */
	action: string;
}

export interface SegmentModel {
	id: string;
	label: string;
	icon?: UtilityIconId;
}

export interface SegmentedModel {
	label: string;
	segments: SegmentModel[];
	selected: string;
}

/** The A ——●—— A slider. `steps` is the tick count, `index` the current stop. */
export interface TextScaleModel {
	label: string;
	steps: number;
	index: number;
}

/** One row of a picker list (语言/货币/数字/日期/时间). */
export interface SelectRowModel {
	id: string;
	label: string;
	/** Right-aligned note — "系统 · 简体中文", "印度计数", "24 小时制". */
	note?: string;
	/** Leading circular badge — the currency sheet's ¥ / $ / €. */
	glyph?: string;
	/** Secondary label after the primary one — the currency sheet's 美元. */
	caption?: string;
	/**
	 * A second line UNDER the label, saying what choosing this row buys — the
	 * speed sheet's "Lowest fee, if you can wait" (spec 068, the owner's
	 * ruling). Deliberately not `caption`: that one sits INLINE after the
	 * label, which is the currency sheet's shape and the wrong one for a
	 * sentence. A row with no advantage to state simply omits it.
	 */
	detail?: string;
	selected?: boolean;
	/** Renders in the mono face — every number/date/time sample does. */
	mono?: boolean;
}

export interface SelectSheetModel {
	title: string;
	subtitle?: string;
	rows: SelectRowModel[];
	/** The currency sheet's search field. */
	searchPlaceholder?: string;
	/** The language sheet's footer note + link. */
	footerNote?: string;
	footerLink?: string;
}

export interface AccountsSheetModel {
	title: string;
	/** "3 个账户 · 总计 $3,262.40". */
	summary: string;
	rows: {
		name: string;
		addressDisplay: string;
		/** The seed of the artwork — what the identicon viewer shows beside it. */
		addressFull: string;
		identiconSvg: string;
		amount: string;
		selected: boolean;
	}[];
	primary: string;
	secondary: string;
	/**
	 * The words for taking ONE wallet off this device (2026-09-23). Empty
	 * leaves the affordance undrawn, which is what a fixture board wants.
	 */
	remove?: string;
	removeBody?: string;
	removeCancel?: string;
}

/** ST3/ST13b/ST16 all share this shape; only the tone and the callout differ. */
export interface ConfirmSheetModel {
	title: string;
	body: string;
	/** Second, quieter paragraph — the sign-out sheet's "keeps" line. */
	note?: string;
	callout?: CalloutModel;
	confirm: string;
	cancel: string;
	tone: 'accent' | 'danger';
}

/** A chain's circular avatar: a letter over a fixture-supplied colour. */
export interface ChainMarkModel {
	letter: string;
	color: string;
	/** Live only: the chain's logo on the data endpoint; the letter shows until it loads, and if it never does. */
	logoUrl?: string;
}

export interface NetworkRowModel {
	id: string;
	mark: ChainMarkModel;
	name: string;
	/** "链 1" — the chain-id line under the name. */
	meta: string;
	badge?: StatusPillModel;
	/** ST9: custom networks carry a 自定义 tag and a delete affordance. */
	tag?: string;
	removable?: boolean;
	/** DST4: the desktop list expands in place instead of pushing a page. */
	expanded?: boolean;
}

/** A labelled mono URL field, with an optional trailing status pill. */
export interface UrlFieldModel {
	id: string;
	label: string;
	value: string;
	placeholder?: string;
	hint?: string;
	badge?: StatusPillModel;
	/** SR2's field is outlined in the tone of its state. */
	tone?: 'default' | 'error' | 'success';
}

export interface NetworkDetailModel {
	title: string;
	/** "链 1 · ETH". */
	subtitle: string;
	mark: ChainMarkModel;
	name: string;
	note: string;
	badge: StatusPillModel;
	rpc: UrlFieldModel;
	explorer: UrlFieldModel;
	/** ST9b's red "not saved" callout. */
	callout?: CalloutModel;
}

export interface CheckItemModel {
	label: string;
	ok: boolean;
}

export interface AddNetworkModel {
	title: string;
	subtitle: string;
	searchPlaceholder: string;
	/** The live wizard's controlled query; absent in fixtures (uncontrolled). */
	query?: string;
	/** Search-result rows; empty once a candidate is chosen. */
	results: NetworkRowModel[];
	/** The chosen candidate's header, once one is chosen. */
	candidate?: {
		mark: ChainMarkModel;
		name: string;
		meta: string;
		badge: StatusPillModel;
	};
	checksTitle?: string;
	checks?: CheckItemModel[];
	customRpc?: UrlFieldModel;
	callout?: CalloutModel;
	/** Accent CTA (compatible) or outline CTA + recheck link (incompatible). */
	primary?: string;
	secondary?: string;
	recheck?: string;
}

export interface ProviderCardModel {
	id: string;
	name: string;
	badge: StatusPillModel;
	field: UrlFieldModel;
	/** The blue trailing action inside the field — 检查密钥 / 获取密钥. */
	action: string;
	/**
	 * Where the action GOES, when it is a link: "Get key" opens the
	 * provider's key page. Absent, the action is the key test.
	 */
	actionUrl?: string;
	/** "支持 12 个网络，共 12 个 · 平均 112ms". */
	support?: string;
	/** The "获取密钥 →" link under an unset provider. */
	link?: string;
	/** Where that link goes — the provider's own site (the label is not a URL). */
	linkUrl?: string;
}

export interface RpcProvidersModel {
	title: string;
	subtitle: string;
	description: string;
	providers: ProviderCardModel[];
}

export interface EndpointsModel {
	title: string;
	description: string;
	fields: UrlFieldModel[];
	reset: string;
	/** Spec 072 (FR-010): what Reset asks before every field goes back. */
	resetSheet: ConfirmSheetModel;
	/** Desktop-only trailing link (DST6). */
	guide?: string;
}

export interface StorageSegmentModel {
	id: string;
	label: string;
	/** 0–1 share of the bar. */
	fraction: number;
	color: string;
}

export interface StorageItemModel {
	id: string;
	label: string;
	/** "200 条 · 1.0 MB" — already joined by the fixture layer. */
	meta: string;
	action: string;
	/** User data clears are destructive; cache clears are not. */
	destructive?: boolean;
}

export interface StorageGroupModel {
	label: string;
	items: StorageItemModel[];
	/** The 清除全部缓存 link under the cache group. */
	action?: string;
}

export interface StorageModel {
	title: string;
	subtitle: string;
	/** "2.4" and "MB", split so the number can carry the display type. */
	amount: string;
	unit: string;
	summary: string;
	segments: StorageSegmentModel[];
	groups: StorageGroupModel[];
}

export interface KeyValueRowModel {
	/** Names a row a live overlay rewrites (the network count). */
	id?: string;
	label: string;
	value: string;
	/** Values in the mono face — every technical detail is. */
	mono?: boolean;
	/** Link rows carry the external glyph. */
	external?: boolean;
	/** Where a link row goes. A row that draws the glyph opens something. */
	href?: string;
}

export interface AboutModel {
	title: string;
	tagline: string;
	version: string;
	sectionTechnical: string;
	rows: KeyValueRowModel[];
	sectionLinks?: string;
	links: KeyValueRowModel[];
	footer: string;
}

export interface FeedbackModel {
	title: string;
	subtitle: string;
	placeholder: string;
	addSteps: string;
	/** Spec 081: the steps box the 添加步骤 button reveals. */
	stepsPlaceholder: string;
	previewToggle: string;
	/**
	 * What the disclosure shows — and, since spec 081, literally the payload's
	 * `environment` field. The consent note under it says "only what you see is
	 * sent"; these lines are the only reason that sentence is true.
	 */
	previewLines: string[];
	consent: string;
	send: string;
	/** The button's own busy label — busy is never disabled (founder's rule). */
	sending: string;
	/** Filed: the two bodies carry `{{number}}`. */
	success: { title: string; bodyNew: string; bodyDeduped: string; view: string };
	/** The endpoint could not; the prefilled form still can. */
	fallback: { title: string; body: string; open: string };
	githubLink: string;
}

/**
 * How the last send ended (spec 081 FR-016).
 *
 * `filed: false` is not an error state — it carries `fallbackUrl`, the
 * prefilled issue form, which is the road that still works when the endpoint
 * is unprovisioned (503), rate-limited (429) or unreachable. A sheet that
 * showed only an apology would be dropping the person's report.
 */
export interface FeedbackResult {
	filed: boolean;
	/** Filed: the issue number and its page. */
	number?: number;
	url?: string;
	deduped?: boolean;
	/** Not filed: the prefilled form, with the person's own words in it. */
	fallbackUrl?: string;
}

/** SR1: the amber "these networks are down" banner and its per-chain fixes. */
export interface RpcBannerModel {
	text: string;
	chips: { id: string; mark: ChainMarkModel; name: string; action: string }[];
}

export interface RpcFixModel {
	title: string;
	mark: ChainMarkModel;
	name: string;
	/** "链 137 · POL". */
	meta: string;
	badge: StatusPillModel;
	callout: CalloutModel;
	field: UrlFieldModel;
	primary: string;
	/** Where to get a working endpoint — absent once the fix succeeds. */
	providersLabel?: string;
	providers?: { label: string; href: string }[];
	report?: string;
}

/** SR3: the quiet rate-limited balance breakdown. */
export interface BalanceDetailModel {
	title: string;
	summary: string;
	sectionPending: string;
	pendingNote: string;
	pending: {
		id: string;
		mark: ChainMarkModel;
		name: string;
		status: string;
		tone: StatusTone;
		action?: string;
	}[];
	sectionDone: string;
	done: { id: string; mark: ChainMarkModel; name: string; amount: string }[];
	/** Spec 038 #E8: the tokens the hero's notice is about, by name and network. */
	sectionUnpriced: string;
	unpriced: { id: string; mark: ChainMarkModel; name: string; detail: string }[];
}

/** SR4: fund this chain's bundler treasury. */
export interface RelayerModel {
	title: string;
	lead: string;
	mark: ChainMarkModel;
	name: string;
	amountHint: string;
	/** Caption under the QR — SR4's 金库 · 打包器运营者 line. */
	qrCaption: string;
	addressDisplay: string;
	copyLabel: string;
	callout: CalloutModel;
	primary: string;
	/** The full address behind the display, for the copy (spec 028 Phase 8). Live only. */
	address?: string;
	/** The code of that address. Absent in the gallery, where the drawn placeholder stands. */
	code?: QrCode;
	/**
	 * Present when this is a network Vela ships, i.e. one whose relayer the
	 * OPERATOR is expected to keep funded. Then telling them is the fix, and
	 * the funding path below is a secondary, folded-away option — nobody
	 * should be nudged into paying for something that is not theirs to pay
	 * for. Absent on a network the person added, where the operator may have
	 * no way to hold gas at all and funding it is the only path there is.
	 */
	report?: {
		label: string;
		/** The disclosure that opens the funding half. */
		selfFundLabel: string;
	};
}

/** SR5: the passkey index is unreachable, and onboarding needs it. */
export interface IndexDownModel {
	title: string;
	subtitle: string;
	callout: CalloutModel;
	field: UrlFieldModel;
	primary: string;
	secondary: string;
	footer: string;
}

/** Everything one phone settings state needs. */
export interface SettingsHomeModel {
	state: MobileSettingsStateId;
	title: string;
	page: SettingsPageId;
	overlay: SettingsOverlayId;
	/** Which tab the bottom bar highlights — 钱包 for the SR rescue states. */
	tab: 'wallet' | 'contacts' | 'explore' | 'settings';
	tabs: { wallet: string; contacts: string; explore: string; settings: string };
	account: AccountRowModel;
	/** Live only (spec 062): the keys that control the wallet, and their Ethereum backup. */
	keys?: WalletKeysModel;
	sections: SettingsSectionModel[];
	appearance: { theme: SegmentedModel; textScale: TextScaleModel };
	signOut: { label: string };
	erase: { title: string; subtitle: string };
	/** Pages, all pre-built so the state switcher is a pure choice. */
	networks: {
		title: string;
		subtitle: string;
		rows: NetworkRowModel[];
		addLabel: string;
		/** The custom row's delete control (spec 028 Phase 8) — it used to borrow `addLabel`. */
		removeLabel: string;
		/** The question the delete control asks first (spec 072); its title is the network's name. */
		removeSheet: ConfirmSheetModel;
	};
	networkDetail: NetworkDetailModel;
	addNetwork: AddNetworkModel;
	rpcProviders: RpcProvidersModel;
	endpoints: EndpointsModel;
	storage: StorageModel;
	about: AboutModel;
	/** Overlays. */
	accountsSheet: AccountsSheetModel;
	signOutSheet: ConfirmSheetModel;
	languageSheet: SelectSheetModel;
	currencySheet: SelectSheetModel;
	numberSheet: SelectSheetModel;
	dateSheet: SelectSheetModel;
	timeSheet: SelectSheetModel;
	/** Spec 068 — the default transaction speed, three rows named by what they buy. */
	feeSpeedSheet: SelectSheetModel;
	clearCachesSheet: ConfirmSheetModel;
	eraseSheet: ConfirmSheetModel;
	feedback: FeedbackModel;
	/** Rescue states. */
	rpcBanner?: RpcBannerModel;
	rpcFix: RpcFixModel;
	balanceDetail: BalanceDetailModel;
	relayer: RelayerModel;
	indexDown: IndexDownModel;
	/** Scrim title behind a sheet — "设置", "钱包", "转账", "设备存储". */
	backdropTitle: string;
	closeLabel: string;
}

/** One second-level nav entry on the desktop (DST1's nav column). */
export interface SettingsNavItemModel {
	id: SettingsPageId;
	icon: UtilityIconId;
	label: string;
}

/** A desktop panel row: a label on the left, one control on the right. */
export interface FormRowModel {
	id: string;
	label: string;
	kind: 'dropdown' | 'segmented' | 'slider';
	/** Dropdown's current value. */
	value?: string;
	/**
	 * What this row's dropdown offers, when it is live (spec 028 T433). Absent
	 * in the fixtures, where `SettingsDesktopModel.dropdown` pins ONE menu open
	 * because DST3 is a board OF that menu — a live panel instead carries every
	 * row's options and opens whichever one is tapped.
	 */
	options?: SelectRowModel[];
}

export interface DropdownModel {
	/** Which form row the open menu hangs from (DST3 pins it to 数字格式). */
	rowId: string;
	rows: SelectRowModel[];
}

/**
 * The Ethereum backup row (spec 062). One line, three states; the row is a
 * button only while there is something to do — a link is not a verdict, so
 * the wallet route checks again before it opens the sheet.
 */
export interface EthereumBackupRowModel {
	title: string;
	subtitle: string;
	tone: 'neutral' | 'positive' | 'caution';
	actionable: boolean;
	/**
	 * The action is "ask again", not "do the backup". Only `could_not_check`:
	 * a person tapping there wants another attempt, which is what the founder
	 * ruled on 2026-09-23. Android draws the same distinction with
	 * `RowTrailing.Retry`.
	 */
	retry?: boolean;
}

/**
 * The keys that control this wallet (spec 062) — drawn above the backup, because
 * a person offered "back up your keys" is owed the sight of them first.
 *
 * `rows` is empty while the first answer is in flight; `note` is set when the
 * registry did not answer and the rows are only what this device remembers.
 */
export interface WalletKeysModel {
	title: string;
	subtitle: string;
	/** "3" — how many keys, beside the title. Empty while loading. */
	count: string;
	loading: boolean;
	note?: string;
	rows: WalletKeyRowModel[];
	/** The founding record's standing on Ethereum; absent = nothing to draw. */
	backup?: EthereumBackupRowModel;
	/** Under the backup: PUBLIC keys only, private keys never leave the device. */
	backupExplain: string;
	copy: { action: string; done: string };
}

export interface WalletKeyRowModel {
	/** The owner's label, or "Key n" when nobody recorded one. */
	name: string;
	/** Who holds it when the core's catalog knows; else the method's generic line. */
	holderFallback: string;
	/**
	 * Spec 075: the holder line already SETTLED — drawn as it stands, and the
	 * AAGUID catalog is not asked.
	 *
	 * Set for a key behind a Trusted Signer page. The vault on the page's far side
	 * is the one thing this wallet cannot reach, so letting the catalog name it
	 * ("Apple Passwords", "Built-in passkey") points the person away from where
	 * the key is — which is the page. The device pass of 2026-09-22 found
	 * exactly that row.
	 */
	holder?: string;
	/** `197d…647b` — the public key, shortened: what tells two unnamed keys apart. */
	fingerprint: string;
	/**
	 * "Verify to use", "Cloud-synced" / "Device-bound" — drawn as pills, the
	 * explorer's way. First, and the only filled one, `signs_here`: the key this
	 * device signs with (founder, 2026-09-26: it must stand out).
	 */
	pills: { text: string; tone: 'signs_here' | 'verified' | 'synced' | 'local' }[];
	/**
	 * What the row opens onto: the registry explorer's facts, each copyable.
	 * Empty when only the device answered — then there is nothing to open.
	 */
	details: { label: string; value: string; mono: boolean; copy: boolean }[];
	/** The row in the create flow's shape — what `PasskeyProviderMark` draws from. */
	key: import('$lib/onboarding/generated/CreateKeyRow').CreateKeyRow;
}

export interface SettingsDesktopModel {
	state: DesktopSettingsStateId;
	title: string;
	page: SettingsPageId;
	overlay: SettingsOverlayId;
	nav: SettingsNavItemModel[];
	/** The wallet sidebar this page reuses — spec 015's model, verbatim. */
	closeLabel: string;
	account: {
		title: string;
		summary: string;
		rows: AccountsSheetModel['rows'];
		primary: string;
		secondary: string;
		signOutLabel: string;
		signOutNote: string;
		erase: { title: string; subtitle: string; action: string };
		/** Live only (spec 062): the keys that control the wallet, and their Ethereum backup. */
		keys?: WalletKeysModel;
	};
	appearance: {
		title: string;
		language: FormRowModel;
		textScale: FormRowModel & { scale: TextScaleModel };
		theme: FormRowModel & { segmented: SegmentedModel };
	};
	localization: {
		title: string;
		description: string;
		rows: FormRowModel[];
	};
	/**
	 * Spec 068 — the same shape as `localization` on purpose: one dropdown
	 * row, so the desktop's control is the desktop's usual control and not a
	 * second pattern invented for one preference.
	 */
	feeSpeed: {
		title: string;
		description: string;
		rows: FormRowModel[];
	};
	networks: {
		title: string;
		subtitle: string;
		addLabel: string;
		removeLabel: string;
		removeSheet: ConfirmSheetModel;
		rows: NetworkRowModel[];
		detail: NetworkDetailModel;
	};
	rpcProviders: RpcProvidersModel;
	endpoints: EndpointsModel;
	storage: StorageModel;
	/** The desktop's clear-all-caches confirm, as a dialog (spec 028 Phase 8). */
	clearCachesSheet: ConfirmSheetModel;
	/**
	 * The phone's erase sheet, as a dialog (specs 072 and 081 FR-017) — the same
	 * words and the same failure, because it is the same sheet.
	 *
	 * The wide layout drew the danger card from the first day and had nowhere to
	 * go from it: the card had no handler and the model had no sheet, so the one
	 * irreversible control on the screen was a picture.
	 */
	eraseSheet: ConfirmSheetModel;
	/** The report panel (spec 081 FR-016) — the phone's sheet, as a page. */
	feedback: FeedbackModel;
	about: AboutModel;
	addNetwork: AddNetworkModel;
	rpcFix: RpcFixModel;
	/** Open dropdown, when the state has one (DST3). */
	dropdown?: DropdownModel;
	/** DSR1 renders the wallet page behind its dialog, banner and all. */
	rpcBanner?: RpcBannerModel;
	backdropTitle: string;
}
