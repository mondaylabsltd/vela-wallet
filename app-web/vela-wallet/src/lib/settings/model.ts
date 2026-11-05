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
import type { SigningVenue } from '$lib/core/generated/SigningVenue';

/**
 * Mobile mocks ST1–ST16 plus the SR1–SR5 rescue set, and spec 102's
 * ST17/ST17b ("Where you review and sign", a `getvela.app` account and one on
 * its own domain) and ST18/ST18b (Settings → Signing pages, and an address
 * refused).
 */
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
	| 'st10d'
	| 'st12'
	| 'st13'
	| 'st13b'
	| 'st14'
	| 'st15'
	| 'st16'
	| 'st17'
	| 'st17b'
	| 'st18'
	| 'st18b'
	| 'sr1'
	| 'sr2'
	| 'sr2b'
	| 'sr3'
	| 'sr4'
	| 'sr5';

/**
 * Desktop mocks DST1–DST8 (+ DST4b dialog), the DSR1 rescue dialog, and spec
 * 102's DST9 (Settings → Signing pages; DST1's account panel carries "Where
 * you review and sign"). DST5 (and the phone's ST11) drew RPC Providers, a
 * page retired 2026-10-29; their numbers are not reused.
 */
export type DesktopSettingsStateId =
	'dst1' | 'dst2' | 'dst3' | 'dst4' | 'dst4b' | 'dst6' | 'dst7' | 'dst8' | 'dst9' | 'dsr1';

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
	'st10d',
	'st12',
	'st13',
	'st13b',
	'st14',
	'st15',
	'st16',
	'st17',
	'st17b',
	'st18',
	'st18b',
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
	'dst6',
	'dst7',
	'dst8',
	'dst9',
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
	/**
	 * The official X / Telegram / Discord links (founder, 2026-09-27): the
	 * phone's Community group, as a desktop nav destination beside About.
	 */
	| 'community'
	/**
	 * Spec 102 — where THIS account's transactions and messages are reviewed
	 * and signed (a pushed page on the phone; part of the account panel on the
	 * desktop).
	 */
	| 'signing-venue'
	/** Spec 102 — the signing pages this device trusts (Settings → Signing pages). */
	| 'signing-pages'
	/**
	 * Spec 103 — the AI agent: whether this wallet answers a remote agent
	 * through a ClientMCP bridge, and which bridge. Web only, so the route adds
	 * its row and nav entry; the fixtures draw neither.
	 */
	| 'agent'
	/**
	 * dapp-11 — the sites that hold a grant, each with Disconnect. Extension
	 * only (no grants elsewhere), so the route adds its row and nav entry.
	 */
	| 'sites'
	/**
	 * "Share anonymous usage statistics", a desktop panel beside the AI
	 * agent's (the phone draws the switch in 高级, under those rows). Web and
	 * extension only, so the route adds its nav entry.
	 */
	| 'privacy'
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
	| 'clear-caches'
	/** Spec 058: one storage row's Clear, asked before it happens. */
	| 'clear-storage-item'
	/** Spec 072: removing a custom network, asked before it happens. */
	| 'remove-network'
	/** Spec 072: resetting the service endpoints, asked before it happens. */
	| 'reset-endpoints'
	/** set-5: leaving the add-network wizard with something entered, asked first. */
	| 'discard-add-network'
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
	/**
	 * A row that LEAVES the app (the Community links): drawn as a link that
	 * opens this URL in a new tab, with no opener and no referrer, instead of
	 * a button that raises `onselect`.
	 */
	href?: string;
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
		/** ms-24: no figure yet, and it is being read: drawn as loading. */
		loading?: boolean;
		selected: boolean;
		/** ob-2: a quiet line under the address — "saved on this device · syncing…". */
		note?: string;
	}[];
	primary: string;
	secondary: string;
	/**
	 * The words for taking ONE wallet off this device (2026-09-23). Empty
	 * leaves the affordance undrawn, which is what a fixture board wants.
	 */
	remove?: string;
	/** r4D-12: the ✕'s accessible name, naming the row (`{{name}}`). */
	removeNamed?: string;
	removeBody?: string;
	removeCancel?: string;
	/**
	 * §18 item 2: a stored account record this browser cannot read, said under
	 * the list — never left out of it in silence. Absent: none.
	 */
	unreadable?: string;
}

/** ST3/ST13b/ST16 all share this shape; only the tone and the callout differ. */
export interface ConfirmSheetModel {
	title: string;
	body: string;
	/** Second, quieter paragraph — the sign-out sheet's "keeps" line. */
	note?: string;
	callout?: CalloutModel;
	/**
	 * A loss this action would cause that nothing brings back — the erase's
	 * wallets not yet synced, by name (ob-2). Drawn as its own danger callout,
	 * so a failure that replaces `callout` does not take it away.
	 */
	warning?: string;
	/**
	 * Present ⇒ the confirm stays disabled until this box is ticked: the
	 * explicit acknowledgement of `warning`.
	 */
	ack?: string;
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
	/**
	 * What removing it asks, when not the custom network's question — a Vela
	 * network shown on the person's word leaves the list and keeps its settings.
	 */
	removeBody?: string;
	/** DST4: the desktop list expands in place instead of pushing a page. */
	expanded?: boolean;
	/** Spec 105: a network the person added moves to the other space — the control's words. */
	move?: string;
	/**
	 * Owner decision 2026-10-28: the person's own node is not answering —
	 * amber while public nodes serve the network, red when it is read through
	 * their node alone and cannot be reached.
	 */
	notice?: { tone: 'warn' | 'error'; text: string };
}

/**
 * Spec 105: which space the network list shows, under its heading, and the
 * switch to the other one.
 */
export interface NetworkSpaceHeadingModel {
	heading: string;
	hint: string;
	switchLabel: string;
	testnet: boolean;
}

/** A labelled mono URL field, with an optional trailing status pill. */
export interface UrlFieldModel {
	id: string;
	label: string;
	value: string;
	placeholder?: string;
	/** A line under the field, always shown: what the field's state is. */
	hint?: string;
	/**
	 * What the field is for — behind an ⓘ beside its label, opened in place
	 * (a disclosure, not a hover: phones have none). States never go here:
	 * they are the badge and the `hint`, always shown.
	 */
	info?: string;
	badge?: StatusPillModel;
	/** SR2's field is outlined in the tone of its state. */
	tone?: 'default' | 'error' | 'success';
	/** A credential: drawn masked. */
	secret?: boolean;
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
	/**
	 * Owner decision 2026-10-28: "Only use my node" — offered only for a
	 * network whose RPC is the person's own.
	 */
	onlyMine?: SwitchRowModel;
	/** The same notice as the row's, beside the RPC it is about. */
	nodeCallout?: CalloutModel;
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
		/**
		 * The verdict, as a pill. Absent on a stop that is not a verdict (no
		 * RPC endpoint listed, a check that could not be made): the callout
		 * under the header says what happened, in the core's sentence.
		 */
		badge?: StatusPillModel;
	};
	checksTitle?: string;
	checks?: CheckItemModel[];
	customRpc?: UrlFieldModel;
	callout?: CalloutModel;
	/** Accent CTA (compatible) or outline CTA + recheck link (incompatible). */
	primary?: string;
	/**
	 * "Open Chain Setup Tool", and where it goes — the core's
	 * `NetCompatibility.setup_url` (`…/chain-setup?chain=<id>`). Present only
	 * for a refusal that has something to deploy: a network with no P-256
	 * verifier gets no such button.
	 */
	secondary?: { label: string; href: string };
	recheck?: string;
	/**
	 * Spec 105: the space the network goes to, the person's choice, shown
	 * when the network may be added. Preselected from what the chain registry
	 * says the chain is (set-5, the core's rule); a testnet when it says nothing.
	 */
	space?: SegmentedModel;
	/**
	 * set-5: the question leaving asks while the wizard holds something the
	 * person entered — a search, a chosen network, an RPC. Absent when there
	 * is nothing to lose: leaving then just leaves.
	 */
	discard?: ConfirmSheetModel;
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

/** dapp-11: Settings › Connected sites. */
export interface SitesPanelModel {
	title: string;
	subtitle: string;
	/** Said when no site holds a grant. */
	empty: string;
	/** One row per site (`id` = its origin), under the consequence of disconnecting one. */
	group: StorageGroupModel;
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

/** A settings row that is a switch (`ui/SwitchRow.svelte`). */
export interface SwitchRowModel {
	id: string;
	/** The leading glyph, where the switch stands among rows that carry one. */
	icon?: UtilityIconId;
	title: string;
	subtitle: string;
	on: boolean;
}

/** Spec 103: Settings → AI agent, built by the route from what is stored and the live connection. */
export interface AgentPanelModel {
	title: string;
	description: string;
	enable: SwitchRowModel;
	bridge: UrlFieldModel;
	token: UrlFieldModel;
	/** The connection, as the bridge last said. */
	status: StatusPillModel;
	/** Another tab of this wallet has the agent: the button that takes it back here. */
	useHere?: string;
	/** Where the agent connects, derived from the bridge address — absent until it parses. */
	endpoint?: { label: string; value: string; hint: string };
	/** The control the agent pressed last. */
	lastAction?: { label: string; value: string };
	/** Spec 106: connecting with clientmcp.dev — the usual way in. */
	link: AgentLinkModel;
	/** Spec 107 E8: the wallets the agent may use, a switch each. */
	wallets: {
		title: string;
		hint: string;
		/** Said when none is on: the agent can do nothing. */
		none?: string;
		rows: SwitchRowModel[];
	};
}

/** Spec 106: Settings → AI agent's clientmcp.dev part. */
export interface AgentLinkModel {
	/** "Connected as @login" — present when clientmcp.dev issued the token. */
	connectedAs?: string;
	/** "Client c-…": which of the person's clients this wallet is (spec 106). */
	clientId?: string;
	connect: string;
	connectHint: string;
	/** On the way to clientmcp.dev, or trading the code: what the button says meanwhile. */
	busy?: string;
	/** Why the last attempt did not connect, in the person's words. */
	problem?: string;
	/** Where the person connects their AI agent (the site's account page). */
	agent?: { label: string; hint: string; href: string };
	/**
	 * The MCP server address for the person's AI agent, to copy from here —
	 * masked until Show (a credential).
	 */
	agentAddress?: {
		label: string;
		hint: string;
		value: string;
		show: string;
		hide: string;
		copy: string;
		copied: string;
		share: string;
	};
	reconnect: string;
	disconnect: string;
	/** "Use your own bridge": the address and token fields, folded. */
	ownBridge: string;
	/** The fields start unfolded: a bridge address or token is configured by hand. */
	ownOpen: boolean;
}

export interface AboutModel {
	title: string;
	/** The wordmark beside the mark — a brand, never translated. */
	name: string;
	/** "v0.9.8 (ced8298)": the version and the build it was cut from. */
	version: string;
	/** What a tap on the version puts on the clipboard, for a support request. */
	versionCopy: string;
	/** The version button's name, and the word that confirms the copy. */
	copyLabel: string;
	copiedLabel: string;
	sectionLinks?: string;
	links: KeyValueRowModel[];
	/** The page's one slogan. */
	footer: string;
}

/**
 * "This account is a standard Safe — you can also open it in Safe{Wallet}":
 * the sentence around the link, and where the link goes.
 */
export interface SafeWalletLineModel {
	before: string;
	app: string;
	after: string;
	href: string;
}

export interface FeedbackModel {
	title: string;
	subtitle: string;
	placeholder: string;
	/** The box's accessible name: the placeholder goes as soon as typing starts. */
	label: string;
	/**
	 * The diagnostics, folded into one quiet line (round 3, owner): "Version
	 * and device info are included · View". `view` opens the rows below and
	 * the consent with them; `hide` folds them again.
	 */
	details: { included: string; view: string; hide: string };
	/**
	 * What the folded rows show — and, since spec 081, literally the payload's
	 * `environment` field. The consent note with them says "only what you see
	 * is sent"; these lines are the only reason that sentence is true.
	 */
	previewLines: string[];
	consent: string;
	send: string;
	/** The button's own busy label — busy is never disabled (founder's rule). */
	sending: string;
	/**
	 * Filed: a thank-you (`title`, `body`), the issue it became as a quiet
	 * link (`followUp`), `dropped` when some screenshots could not be stored,
	 * and `done`, which closes.
	 */
	success: {
		title: string;
		body: string;
		followUp: string;
		dropped: string;
		done: string;
	};
	/**
	 * The endpoint could not file it: `title` and `body` are separate lines
	 * (never glued with a dash), `screenshots` a paragraph of its own — the
	 * form cannot carry them — and `retry` (common.tryAgain) is what the send
	 * button reads in this state.
	 */
	fallback: { title: string; body: string; open: string; screenshots: string; retry: string };
	/**
	 * Screenshots (078 round 3). Copy only; which images are attached is the
	 * sheet's own state until Send. `limit` (a sixth refused) and `full` (five
	 * attached) are filled with the cap; `remove` keeps `{{index}}` (1-based)
	 * for each tile.
	 */
	screenshots: {
		/** The tiles' list, for assistive tech. */
		label: string;
		add: string;
		public: string;
		remove: string;
		limit: string;
		full: string;
		unsupported: string;
		dropHint: string;
		max: number;
		/**
		 * The viewer a tile opens (078 §C): `view` is each tile's a11y label
		 * and keeps `{{index}}` (1-based); `close` is the ✕; `removeFromViewer`
		 * the visible button under the picture.
		 */
		view: string;
		close: string;
		removeFromViewer: string;
	};
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
	/** Filed, but this many screenshots could not be stored. */
	screenshotsDropped?: number;
	/** Not filed: the prefilled form, with the person's own words in it. */
	fallbackUrl?: string;
	/** The send carried screenshots — the fallback says they cannot follow. */
	withScreenshots?: boolean;
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

/**
 * Spec 092: every network the wallet cannot reach, in the core's order, each
 * with what was last read there and its RPC fix.
 */
export interface UnreachableModel {
	/** The home's own line ("Can't reach 3 networks right now"), live. */
	title: string;
	/** Absent once every network has come back. */
	summary?: string;
	rows: {
		id: string;
		chainId: number;
		mark: ChainMarkModel;
		name: string;
		/** "Last seen $1,234.50", "Not read yet", … */
		line: string;
		/**
		 * "Fix" — the network's RPC editor. Absent when the core says the RPC is
		 * not what failed (`UnreachableNetwork.rpc_fixable`, PR 3 note 4: a token
		 * list that could not be loaded): the row then offers nothing to press.
		 */
		action?: string;
	}[];
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
	done: {
		id: string;
		mark: ChainMarkModel;
		name: string;
		amount: string;
		/** Owner decision 2026-10-28: "Using public nodes" — its own node is not answering. */
		note?: string;
	}[];
	/** Spec 038 #E8: the tokens the hero's notice is about, by name and network. */
	sectionUnpriced: string;
	unpriced: { id: string; mark: ChainMarkModel; name: string; detail: string }[];
}

/** SR4: fund this chain's bundler treasury. */
/**
 * The relay cannot serve this chain (spec 098 §2). Not the funding sheet:
 * gas would not help a relay that cannot reach the network at all.
 */
export interface RelayUnreachableModel {
	title: string;
	lead: string;
	mark: ChainMarkModel;
	name: string;
	/** Where to change it — the network's RPC, or the relay endpoint. */
	hint?: string;
	/** Present on a network Vela ships: the operator's to fix, so report it. */
	report?: { label: string };
	primary: string;
}

export interface RelayerModel {
	title: string;
	lead: string;
	/** What the treasury has against what it needs, in its coin (spec 098 §4). Live only. */
	balanceLine?: string;
	/** "Checking every few seconds…" — the sheet closes by itself (098 §4). Live only. */
	watching?: string;
	/** Under the code: scan it from another wallet. */
	scanHint?: string;
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

// ---------------------------------------------------------------------------
// Spec 102 — where you review and sign, and the pages you trust
// ---------------------------------------------------------------------------

/**
 * One line about a signing page's integrity, in words: "Version 0ba8ee8c ·
 * matches Vela's published build list · checked 14:32", or why it will not be
 * opened. The words and the state are the core's (`IntegrityLine`); the tone
 * is only how loud the line is drawn.
 *
 * `ok` — it matches the published list, or the person trusts it here;
 * `checking` — the check is running: no verdict yet, so neither a warning
 * nor a pass (a clock, quietly);
 * `warn` — a caution, never a refusal: checking is off, or a self-hosted
 * build asks to be trusted (a question, with its answer beside it);
 * `error` — refused: it will not open.
 */
export interface IntegrityLineModel {
	text: string;
	tone: 'ok' | 'checking' | 'warn' | 'error';
}

/** One choice of "Where you review and sign" (spec 102 R1, R2). */
export interface VenueRowModel {
	/** `in_vela`, or the page's address. */
	id: string;
	/** The venue as the core names it — what choosing this row sends back. */
	venue: SigningVenue;
	icon: UtilityIconId;
	/** "In Vela" / "On a trusted page". */
	title: string;
	/** What it is: "Vela's own signing sheet" / "A zero-dependency page shows exactly what you sign". */
	body: string;
	/**
	 * A page's name ("Vela's official signing page", the person's label for
	 * it, else "Self-hosted · {{domain}}") and host — drawn beside the name
	 * only where the name does not already say it (`hostShown`): a row never
	 * names the same address twice.
	 */
	page?: { name: string; host: string; official: boolean; hostShown: boolean };
	/** "Keys on getvela.app" — whose keys this choice can use (R1), drawn on every row. */
	keysOn: string;
	/** A page's integrity line, where the shell checks pages. */
	integrity?: IntegrityLineModel;
	/** Where this account reviews and signs now. */
	active: boolean;
	/** The core's reason this choice cannot reach the account's keys, in words — drawn disabled. */
	blocked?: string;
}

/** The account's "Where you review and sign" (spec 102, D1: per account, on this device). */
export interface VenueModel {
	title: string;
	subtitle: string;
	/** The settings row's value: where this account signs now, in two words. */
	value: string;
	/** The active page's label and host, or why nothing here can sign. */
	note?: string;
	/**
	 * The settings row's one line under its title: "On a trusted page ·
	 * sign.getvela.app", "In Vela", or why nothing here can sign.
	 */
	summary: string;
	/** "Keys on {{domain}}" — the account's signing domain. */
	domainLine: string;
	rows: VenueRowModel[];
	/**
	 * The rows are a statement, not a choice: where the account signs is
	 * marked and nothing is offered (a board of a shell that opens no page).
	 * Absent — every shell today, the web and the extension included since
	 * 2026-10-30 — every reachable row is a choice.
	 */
	readOnly?: boolean;
	/**
	 * What a signing page protects against, and what it does not (owner,
	 * 2026-10-30) — said under the choice, in two lines: a tampered app
	 * interface cannot make the person sign something else there; a fully
	 * compromised wallet on the same passkey domain can still ask the passkey
	 * itself.
	 */
	pageNotes?: string[];
}

/** One row of Settings → Signing pages. */
export interface SigningPageRowModel {
	url: string;
	/** "Vela's official signing page", the person's label for it, or "Self-hosted · {{domain}}". */
	name: string;
	/** Its host — absent when the name already says it ("Self-hosted · sign.example.com"). */
	host?: string;
	/**
	 * "Keys on {{domain}}" — which accounts it can sign for, seen before
	 * choosing it (R1). Absent when that domain is the page's own host: the
	 * address already says it.
	 */
	keysOn?: string;
	official: boolean;
	integrity?: IntegrityLineModel;
	/**
	 * Its check asks to trust a version it does not know (a self-hosted page
	 * only): "Trust this version", and the version that answer stores.
	 */
	trust?: { label: string; version: string };
}

/** Settings → Signing pages (spec 102): the official page first, then the self-hosted ones. */
export interface SigningPagesModel {
	title: string;
	subtitle: string;
	rows: SigningPageRowModel[];
	/** "Add a page": the address field (its hint is the refusal, when there is one) and its button. */
	add: UrlFieldModel;
	addAction: string;
	renameLabel: string;
	removeLabel: string;
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
	/** Under the account's keys, a Vela account's or a multisig's: it is a Safe. */
	safeWallet: SafeWalletLineModel;
	/** Spec 102: where this account reviews and signs — a row under the account, and its page. */
	venue?: VenueModel;
	/** Spec 102: Settings → Signing pages. Absent where no page is ever opened (the web). */
	signingPages?: SigningPagesModel;
	sections: SettingsSectionModel[];
	appearance: { theme: SegmentedModel; textScale: TextScaleModel };
	signOut: { label: string };
	erase: { title: string; subtitle: string };
	/** Pages, all pre-built so the state switcher is a pure choice. */
	networks: {
		title: string;
		subtitle: string;
		rows: NetworkRowModel[];
		/** Spec 105: live only — the space the rows are. */
		space?: NetworkSpaceHeadingModel;
		addLabel: string;
		/** The custom row's delete control (spec 028 Phase 8) — it used to borrow `addLabel`. */
		removeLabel: string;
		/** The question the delete control asks first (spec 072); its title is the network's name. */
		removeSheet: ConfirmSheetModel;
	};
	networkDetail: NetworkDetailModel;
	addNetwork: AddNetworkModel;
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
 * The Ethereum copy's row (spec 062). One line; the words, the tone and the
 * tap are the core's (`registry_backup::BackupRow`). The row is a button only
 * while there is something to do — a link is not a verdict, so the route
 * checks again before it opens the sheet.
 */
export interface EthereumBackupRowModel {
	title: string;
	subtitle: string;
	/**
	 * The core's tone (`registry_backup::BackupTone`). Never a caution: a
	 * copy is optional and costs a fee, so "not copied yet" is a state, not a
	 * defect.
	 */
	tone: 'neutral' | 'positive';
	/**
	 * What a tap does (`registry_backup::BackupAction`): `copy` opens the
	 * signing sheet, `retry` asks the chain again (only "couldn't check" —
	 * the founder's ruling of 2026-09-23), `none` is a statement and takes no
	 * tap at all.
	 */
	action: 'none' | 'copy' | 'retry';
	/**
	 * The paragraph under the row: what the copy makes public (the wallet's
	 * name, each key's name, public key, credential ID and authenticator
	 * model) and that it costs a network fee. The core names it per state
	 * (`BackupRow.explain_key`) and names none for a record that can never be
	 * copied — absent, no paragraph is drawn and no room is kept for one.
	 */
	explain?: string;
}

/**
 * The keys that control this wallet (spec 062) — drawn above the backup, because
 * a person offered "back up your keys" is owed the sight of them first.
 *
 * `rows` is empty only before the device's record is read; while the registry
 * is being asked they are that record's (`asking`). `note` is set when the
 * registry did not answer and the rows are only what this device remembers.
 */
export interface WalletKeysModel {
	title: string;
	subtitle: string;
	/**
	 * "3" — how many keys, beside the title. The device's own record says it,
	 * so it is there while the registry is still being asked; empty only before
	 * that record is read.
	 */
	count: string;
	/** Before the device's record is read: one row's shape, nothing else. */
	loading: boolean;
	/**
	 * The registry is still being asked (device walk, item 13.1). The rows are
	 * the device's record and stand where they will stand: a key's own name
	 * and short id are drawn; who holds it, its mark and the badges nobody has
	 * vouched for yet are quiet room of the same size — never a word the
	 * registry may contradict a moment later.
	 */
	asking: boolean;
	note?: string;
	/**
	 * Spec 102 (P2-10): "Keys on sign.example.com" — for an account on its own
	 * signing domain only, whose keys answer nowhere but its page.
	 */
	domain?: string;
	rows: WalletKeyRowModel[];
	/**
	 * The wallet record's standing on Ethereum, and the paragraph under it;
	 * absent = nothing to draw.
	 */
	backup?: EthereumBackupRowModel;
	/**
	 * Every second line the copy's row can say, in this language. The line
	 * keeps the room of the tallest, so the row is one height through
	 * "Checking…" and whatever the chain answers (ru: "couldn't check" is two
	 * lines on a phone where "checking" is one).
	 */
	backupLines: string[];
	copy: { action: string; done: string };
}

export interface WalletKeyPillModel {
	text: string;
	tone: 'signs_here' | 'verified' | 'synced' | 'local';
}

export interface WalletKeyRowModel {
	/**
	 * The key's own label, or "Key n" by its place in the founding order when
	 * it has none (the core's `label_key` / `label_n`, r10 follow-up #5b).
	 * Never empty and never the account's name — also while the registry is
	 * asked: a key's place is a fact no answer contradicts, and a registry
	 * that does name the key replaces it in the same line.
	 */
	name: string;
	/** Who holds it when the core's catalog knows; else the method's generic line. */
	holderFallback: string;
	/** `197d…647b` — the public key, shortened: what tells two unnamed keys apart. */
	fingerprint: string;
	/**
	 * "Verify to use", "Cloud-synced" / "Device-bound" — drawn as pills, the
	 * explorer's way. First, and the only filled one, `signs_here`: the key this
	 * device signs with (founder, 2026-09-26: it must stand out).
	 */
	pills: WalletKeyPillModel[];
	/**
	 * The fullest sets of badges this row can end with — one per answer to
	 * "synced?" — and the room the row keeps for them whatever it is showing:
	 * three badges wrap to two lines on a phone, and a row that grew by a line
	 * when its badges arrived moved everything under it. While the registry is
	 * being asked, the first set is what is drawn, quietly.
	 */
	pillRoom: WalletKeyPillModel[][];
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
		/** ob-14: the switcher's remove and its confirm, here too (live only). */
		remove?: string;
		removeNamed?: string;
		removeBody?: string;
		removeCancel?: string;
		/** §18 item 2: a stored record this browser cannot read, said under the list. */
		unreadable?: string;
		signOutLabel: string;
		signOutNote: string;
		erase: { title: string; subtitle: string; action: string };
		/** Live only (spec 062): the keys that control the wallet, and their Ethereum backup. */
		keys?: WalletKeysModel;
		/** Spec 102: where this account reviews and signs, under its keys. */
		venue?: VenueModel;
	};
	/** Spec 102: Settings → Signing pages (DST9). Absent where no page is ever opened (the web). */
	signingPages?: SigningPagesModel;
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
		/** Spec 105: live only — the space the rows are. */
		space?: NetworkSpaceHeadingModel;
		detail: NetworkDetailModel;
	};
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
	/** The phone's Community group, as a panel: the same three link rows. */
	community: { title: string; rows: SettingsRowModel[] };
	/** The panel "Share anonymous usage statistics" stands in, beside the AI agent's. */
	privacy: { title: string };
	/** Under the account's keys, as on the phone. */
	safeWallet: SafeWalletLineModel;
	about: AboutModel;
	addNetwork: AddNetworkModel;
	rpcFix: RpcFixModel;
	/** Open dropdown, when the state has one (DST3). */
	dropdown?: DropdownModel;
	/** DSR1 renders the wallet page behind its dialog, banner and all. */
	rpcBanner?: RpcBannerModel;
	backdropTitle: string;
}
