/**
 * Wallet view models (spec 015, data-model.md).
 *
 * Components consume ONLY these display-ready shapes — no service types, no
 * formatting, no fetching (spec FR-005 / SC-005). A later "real data" feature
 * replaces the fixture layer that builds them and nothing else.
 */

export type MobileStateId = 'h1' | 'h1s' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' | 'h7' | 'h7x' | 'h8';

export type DesktopStateId = 'd1' | 'd2' | 'd3';

export type NetworkPillModel =
	{ kind: 'all'; dots: string[]; label: string } | { kind: 'single'; dot: string; label: string };

export interface WalletHeaderModel {
	name: string;
	addressDisplay: string;
	/**
	 * The seed the identicon was drawn from — the account's full address.
	 * The viewer shows it beside the artwork, which is the whole point of the
	 * viewer: a fingerprint is only legible next to the thing it fingerprints.
	 */
	addressFull: string;
	/** Inline SVG markup from vela-core (circular variant, no ids). */
	identiconSvg: string;
}

export type BalanceStateKind = 'normal' | 'zero-live' | 'loading' | 'hidden';

export interface BalanceModel {
	label: string;
	currency: string;
	state: BalanceStateKind;
	/** e.g. "$1,383" — absent while loading. */
	integer?: string;
	/** e.g. "28" — rendered de-emphasised after the separator. */
	decimals?: string;
	/**
	 * The mark between the two, from the person's number preset (spec 028
	 * Phase 9, T480). Absent — the fixtures — the display draws `.`; live it
	 * is the preset's, so `1.575,55` never reads `1.575.55`.
	 */
	decimalMark?: string;
	liveText?: string;
	status?: { kind: 'warning' | 'refreshing'; text: string };
	a11yHide: string;
	a11yShow: string;
}

export type ActivityKind = 'sent' | 'received' | 'dapp';

export interface ActivityRowModel {
	/** Live rows only: the feed item's id, so a tap can name what it hit. */
	id?: string;
	kind: ActivityKind;
	title: string;
	subtitle: string;
	amount: string;
	unit: string;
	positive: boolean;
	masked: boolean;
	badgeColor: string;
	/** Live rows only: the chain's logo over the badge dot. */
	badgeLogoUrl?: string;
}

export interface ActivityGroupModel {
	label: string;
	rows: ActivityRowModel[];
}

export type AssetFiatModel =
	| { kind: 'value'; text: string }
	| { kind: 'no-price'; text: string }
	| { kind: 'masked' }
	/**
	 * Spec 021 SD2d: the row has no fiat line at all. Distinct from `masked`,
	 * which HIDES a figure that exists — a sweep row is an editable amount, and
	 * dots under it would read as a concealed second number.
	 */
	| { kind: 'none' };

export interface AssetRowModel {
	/** Live rows only: the held token's key, so a tap can name what it hit. */
	id?: string;
	ticker: string;
	chain: string;
	badgeColor: string;
	/** Live rows only: logo candidates, tried in order; the glyph shows otherwise. */
	logoUrls?: string[];
	/** Live rows only: the badge chain's logo over the dot. */
	badgeLogoUrl?: string;
	/** Live rows only: no badge — a native coin on its own chain wears one logo, not two. */
	badgeHidden?: boolean;
	balance: string;
	fiat: AssetFiatModel;
	masked: boolean;
}

export interface SectionModel {
	title: string;
	action: string;
	/** 'rows' renders content; 'empty' the empty state; 'loading' skeletons. */
	mode: 'rows' | 'empty' | 'loading';
	empty?: { title: string; caption: string };
}

export interface ChainRowModel {
	name: string;
	dot: string;
	count: number;
	selected: boolean;
	/** Live rows only: the chain this row filters to; `null` is 全部. */
	chainId?: number | null;
	/** Live rows only: the chain's logo; the dot shows until it loads, and if it never does. */
	logoUrl?: string;
}

export interface TabsModel {
	wallet: string;
	contacts: string;
	explore: string;
	settings: string;
}

export interface WalletHomeModel {
	state: MobileStateId;
	header: WalletHeaderModel;
	pill: NetworkPillModel;
	balance: BalanceModel;
	actions: { receive: string; send: string; scan: string };
	activitySection: SectionModel;
	activityGroups: ActivityGroupModel[];
	assetsSection: SectionModel;
	assetRows: AssetRowModel[];
	tabs: TabsModel;
	sheet?: { title: string; rows: ChainRowModel[] };
	/** 1 or 1.35 — multiplies the text tokens via `--text-scale` (FR-011). */
	textScale: number;
}

export interface ReceivePanelModel {
	kind: 'receive';
	title: string;
	token: { ticker: string; badgeColor: string; detail: string };
	qrCaption: string;
	addressLabel: string;
	addressFull: string;
	copyAddress: string;
	warningTitle: string;
	warningReminder: string;
	networksLine: string;
}

export interface AssetDetailPanelModel {
	kind: 'asset-detail';
	/** Live only: the held token's key, so its two doors can name it. */
	id?: string;
	title: string;
	token: {
		ticker: string;
		badgeColor: string;
		balance: string;
		fiatLine: string;
		/** Live only — see `AssetRowModel`. */
		logoUrls?: string[];
		badgeLogoUrl?: string;
		badgeHidden?: boolean;
	};
	send: string;
	receive: string;
	/** `copy` names the copy affordance; `copyValue` is the whole text when `value` is shortened. */
	facts: { label: string; value: string; copy?: string; copyValue?: string }[];
	viewOnExplorer: string;
	/** Where "view on explorer" leads — live only; absent, the control is drawn inert. */
	explorerUrl?: string;
	transactionsTitle: string;
	rows: ActivityRowModel[];
}

export type PanelModel = ReceivePanelModel | AssetDetailPanelModel;

export interface SidebarModel {
	header: WalletHeaderModel;
	nav: { id: 'wallet' | 'contacts' | 'explore' | 'settings'; label: string; selected: boolean }[];
	networksTitle: string;
	networks: ChainRowModel[];
}

export type PanelId = 'none' | 'receive' | 'asset-detail';

export interface WalletDesktopModel {
	state: DesktopStateId;
	sidebar: SidebarModel;
	balance: BalanceModel;
	actions: { receive: string; send: string; scan: string };
	activitySection: SectionModel;
	activityGroups: ActivityGroupModel[];
	assetsSection: SectionModel;
	assetRows: AssetRowModel[];
	/** Both panel contents ship so the gallery can open them interactively. */
	panels: { receive: ReceivePanelModel; assetDetail: AssetDetailPanelModel };
	/** Which panel the state opens with (D1 none, D2 receive, D3 asset detail). */
	initialPanel: PanelId;
	closeLabel: string;
}
