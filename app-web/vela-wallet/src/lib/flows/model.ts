/**
 * Wallet-flow view models (spec 021).
 *
 * The same contract spec 015's `wallet/model.ts` established, extended to the
 * four journeys that live off the wallet home: Receive, Send, Activity, Assets.
 * Components consume ONLY these display-ready shapes — no service types, no
 * formatting, no fetching. Every string arrives resolved and every number
 * arrives as text, so the later "real data" feature replaces the fixture layer
 * and nothing else.
 *
 * Shapes that spec 015 already defined (`ActivityRowModel`, `AssetRowModel`,
 * `ChainRowModel`, …) are imported rather than restated: the send token picker
 * and the assets list render the SAME row as the wallet home does, and a
 * parallel type would be the first step towards a parallel component.
 */

import type { QrCode } from '$lib/wallet/qr';
import type {
	ActivityGroupModel,
	ActivityRowModel,
	AssetRowModel,
	ChainRowModel
} from '../wallet/model';

export type { ActivityGroupModel, ActivityRowModel, AssetRowModel, ChainRowModel };

/** Mobile gallery ids — spec.md's state matrix, stable across all four clients. */
export type FlowStateId =
	| 'r1'
	| 'r2'
	| 'r2x'
	| 'r3'
	| 'r4'
	| 's1'
	| 'a1'
	| 'a2'
	| 'a3'
	| 't1'
	| 't2'
	| 't3'
	| 't3b'
	| 't4'
	| 't5'
	| 't5b'
	| 'sd1'
	| 'sd1b'
	| 'sd2'
	| 'sd2b'
	| 'sd2c'
	| 'sd2d'
	| 'sd2e'
	| 'sd2f'
	| 'sd3'
	| 'sd3b'
	| 'sd3c'
	| 'sd4a'
	| 'sd4b'
	| 'sd4c';

/** Desktop gallery ids — the same content in the third column. */
export type DesktopFlowStateId =
	| 'dr1'
	| 'dr2'
	| 'dr3'
	| 'ds1'
	| 'da1'
	| 'da2'
	| 'da3'
	| 'dt1'
	| 'dt3'
	| 'dt3b'
	| 'dt4'
	| 'dsd1'
	| 'dsd2'
	| 'dsd2b'
	| 'dsd3'
	| 'dsd4'
	/**
	 * The three sub-pickers the desktop send form can open. `docs/design/wallet-2/`
	 * draws no panel for them, but DSD2L draws the affordances that ask for
	 * them — a person icon on the recipient field, a chevron on the fee row,
	 * and (on DSD2bL) an import pill. A chevron that leads nowhere is a defect,
	 * so the same bodies the phone raises as sheets become panels here.
	 */
	| 'dsd2c'
	| 'dsd2e'
	| 'dsd2f';

/* ------------------------------------------------------------------ chrome */

/** A screen's own top bar: back, title, and at most one trailing text action. */
export interface FlowHeaderModel {
	title: string;
	backLabel: string;
	/** e.g. T1's 添加. Absent on screens whose header carries no action. */
	action?: string;
	/** The network pill, where the screen filters by chain (A1, T1, SD1). */
	pill?: { dots: string[]; label: string };
}

/* ------------------------------------------------------------------ shared */

/** A token's circular mark: three-letter glyph plus its chain dot. */
export interface TokenMarkModel {
	ticker: string;
	badgeColor: string;
	/** Live marks only (spec 028 Phase 9, T492): logo candidates, tried in order; the glyph shows otherwise. */
	logoUrls?: string[];
	/** Live marks only: the badge chain's logo over the dot. */
	badgeLogoUrl?: string;
	/** Live marks only: no badge — a native coin on its own chain, or a chain drawn as itself. */
	badgeHidden?: boolean;
}

/**
 * A label/value row. The single label-value primitive for the whole feature —
 * A2's transaction facts, SD3's confirmation summary, T2's token facts and
 * T3b's chain facts are the same row with different leading art.
 */
export interface FactRowModel {
	label: string;
	value: string;
	/** Leading art on the value side: a chain dot, a token mark, or an identicon. */
	lead?:
		| { kind: 'dot'; color: string }
		| { kind: 'token'; mark: TokenMarkModel }
		/** `address` is the seed; present, the artwork opens the identicon viewer on it. */
		| { kind: 'identicon'; svg: string; address?: string };
	/** Renders the value in the mono face (addresses, hashes). */
	mono?: boolean;
	/** Shows a copy affordance and its accessible name. */
	copy?: string;
	/** The whole text the affordance copies, when `value` is a shortened form. */
	copyValue?: string;
}

export type StatusTone = 'success' | 'warning' | 'error' | 'info';

export interface StatusChipModel {
	text: string;
	tone: StatusTone;
}

/* ----------------------------------------------------------------- receive */

/** One row of R1: a network, the address on it, and the two things you do. */
export interface NetworkRowModel {
	name: string;
	code: string;
	badgeColor: string;
	/** Live rows: the chain, so a tapped row can name the code it opens. */
	chainId?: number;
	/** Live rows: the chain's logo over the lettered badge. */
	logoUrl?: string;
	addressDisplay: string;
	/** Live rows: the whole address the copy button writes. */
	addressFull?: string;
	copyLabel: string;
	qrLabel: string;
}

export interface ReceiveListModel {
	header: FlowHeaderModel;
	/** "One address across all 8 networks". */
	subtitle: string;
	searchPlaceholder: string;
	/** Shown in place of the rows when the search matches nothing. */
	emptyText: string;
	rows: NetworkRowModel[];
}

/** The account card that sits above every QR: who this address belongs to. */
export interface AddressCardModel {
	name: string;
	identiconSvg: string;
	/** The full address, pre-split into the two lines the mocks wrap it into. */
	lines: [string, string];
	copyLabel: string;
}

export interface ReceiveQrModel {
	/** "Use this address to receive assets on Ethereum" / "… to receive USDT …". */
	title: string;
	closeLabel: string;
	/** R3 only: the token's contract, shown above the account card. `copyValue` is the whole address. */
	contract?: { label: string; value: string; copyLabel: string; copyValue?: string };
	account: AddressCardModel;
	/**
	 * The code to draw (spec 028). Absent = the drawn placeholder, which is what
	 * the galleries carry; a live screen always supplies a real one.
	 */
	code?: QrCode;
	/** The mark drawn in the middle of the code — the token, or the network. */
	centre: TokenMarkModel;
	warning: string;
	saveImage: string;
	/** Live only: what 保存图片 produces — R4, about this network or token (T488). */
	share?: ShareCardModel;
	viewOnExplorer: string;
	/** Where "view on explorer" leads — live only; absent, the control is drawn inert. */
	explorerUrl?: string;
}

/** R4 — the image "Save image" produces, not a screen someone navigates to. */
export interface ShareCardModel {
	headline: string;
	/** As above: absent in the gallery, real everywhere a person can save it. */
	code?: QrCode;
	name: string;
	lines: [string, string];
	networkNote: string;
	networkMark: TokenMarkModel;
	identiconSvg: string;
	wordmark: string;
}

/* -------------------------------------------------------------------- scan */

export interface ScanModel {
	title: string;
	hint: string;
	closeLabel: string;
	/** The three tools under the frame. Desktop drops the torch. */
	tools: { id: 'gallery' | 'torch' | 'flip'; label: string }[];
}

/* ---------------------------------------------------------------- activity */

export interface HistoryModel {
	header: FlowHeaderModel;
	/** 'rows' renders the groups, 'empty' the filtered-empty line, 'loading' skeletons. */
	mode: 'rows' | 'empty' | 'loading';
	emptyText: string;
	groups: ActivityGroupModel[];
}

/** A2 / A3 — one transaction, opened from a history row. */
export interface TxDetailModel {
	/** "Received USDT" / "Sent POL". */
	title: string;
	status: StatusChipModel;
	closeLabel: string;
	amount: string;
	fiat: string;
	positive: boolean;
	facts: FactRowModel[];
	viewOnExplorer: string;
	/** Where "view on explorer" leads — live only; absent, the control is drawn inert. */
	explorerUrl?: string;
	/**
	 * "Delete record" — the feed's tombstone (spec 028 Phase 8). Absent in the
	 * drawn fixtures, where the detail is a picture; present on a live row.
	 */
	deleteLabel?: string;
	/** Spec 038 #D2 — a folded batch row: its parts, under the facts. */
	breakdownTitle?: string;
	breakdown?: BreakdownRowModel[];
}

/* ------------------------------------------------------------------ assets */

export interface AssetsModel {
	header: FlowHeaderModel;
	searchPlaceholder: string;
	rows: AssetRowModel[];
	/** T1's trailing link under the list. */
	addByAddress: string;
	/** T4: the guided-empty body replaces the rows entirely. */
	empty?: {
		title: string;
		caption: string;
		cta: string;
		hintTitle: string;
		hintBody: string;
	};
}

/** T2 — one token, opened from an assets row. */
export interface TokenDetailModel {
	mark: TokenMarkModel;
	symbol: string;
	chain: string;
	closeLabel: string;
	balance: string;
	fiat: string;
	receive: string;
	send: string;
	facts: FactRowModel[];
	transactionsTitle: string;
	rows: ActivityRowModel[];
	/** Live only (spec 038 #E2): per row, the history index that opens its detail. */
	rowTargets?: (number | undefined)[];
	viewOnExplorer: string;
	/** Where "view on explorer" leads — live only; absent, the control is drawn inert. */
	explorerUrl?: string;
}

/* -------------------------------------------------------------- add token  */

export type AddTokenTab = 'erc20' | 'native';

/** The result card under the input: what the address or query resolved to. */
export type AddTokenResult =
	| { kind: 'none' }
	| { kind: 'searching'; text: string }
	| { kind: 'token'; mark: TokenMarkModel; name: string; detail: string; chip?: StatusChipModel }
	| {
			kind: 'network';
			mark: TokenMarkModel;
			name: string;
			chip: StatusChipModel;
			/** Chain ID / native coin, as label-value rows. */
			facts: FactRowModel[];
			/** T5b's "deploy the missing contracts" link, on the incompatible chip. */
			link?: string;
	  }
	| { kind: 'not-found'; text: string }
	/**
	 * T3b live (spec 028 Phase 10): the chain index's matches for what was
	 * typed, before one is chosen and probed. The drawn sheet shows one card;
	 * a registry of two thousand chains needs the list in between.
	 */
	| {
			kind: 'suggestions';
			rows: { id: string; mark: TokenMarkModel; name: string; meta: string }[];
	  };

export interface AddTokenModel {
	title: string;
	closeLabel: string;
	tab: AddTokenTab;
	tabs: { erc20: string; native: string };
	/** ERC-20 only: the network the contract is looked up on. */
	network?: { mark: TokenMarkModel; name: string; pickLabel: string };
	fieldLabel: string;
	fieldValue: string;
	fieldPlaceholder: string;
	/** Draws the field in its error state and prints this under it. */
	fieldError?: string;
	result: AddTokenResult;
	cta: string;
	ctaDisabled: boolean;
}

/* -------------------------------------------------------------------- send */

/** SD1 / SD1b — pick the token, or several of them. */
export interface SendPickModel {
	header: FlowHeaderModel;
	searchPlaceholder: string;
	filters: { id: string; label: string; selected: boolean }[];
	/** SD1b: the chain lock, once the first token pins the network. */
	notice?: { mark: TokenMarkModel; text: string };
	rows: AssetRowModel[];
	/** SD1b: which rows are chosen, and which are off-network and greyed. */
	selection?: { selected: boolean[]; dimmed: boolean[]; selectAll: string };
	/** SD1's "send several tokens" ghost CTA, or SD1b's accent one. */
	cta: { label: string; accent: boolean };
}

/** The token card at the top of the send form. */
export interface SendTokenCardModel {
	mark: TokenMarkModel;
	symbol: string;
	/** "Ethereum · Balance 53.4836". */
	detail: string;
	max?: string;
}

/** SD2b's recipient card: who, how much, and a way to drop them. */
export interface RecipientCardModel {
	/** Live rows only: the core's draft id, so a per-row pick can name its target. */
	id?: string;
	ordinal: string;
	name: string;
	/** The seed of the artwork — what the identicon viewer shows beside it. */
	address: string;
	identiconSvg: string;
	amount: string;
	/**
	 * Live rows only (spec 028 Phase 10): the figure as typed, in token units,
	 * for the editable card. `amount` above stays the worded "5 USDT".
	 */
	amountValue?: string;
	/** Live rows only: the field names the editable card announces. */
	addressLabel?: string;
	pickLabel?: string;
	removeLabel: string;
	/**
	 * Live rows only (issue 203): this row pays an address an EARLIER row
	 * already pays, and this sentence names which one. The core decides it
	 * (`SendView.split_duplicates`); nothing here compares addresses.
	 */
	duplicateNote?: string;
}

/** SD2d's sweep row: one token, its amount, and a Max. */
export interface SweepRowModel {
	mark: TokenMarkModel;
	symbol: string;
	balanceLabel: string;
	amount: string;
	max: string;
}

export interface FeeRowModel {
	label: string;
	mark: TokenMarkModel;
	value: string;
	openLabel: string;
}

export type SendFormMode = 'single' | 'split' | 'sweep';

export interface SendFormModel {
	header: FlowHeaderModel;
	mode: SendFormMode;
	/** 'single' and 'split' send one token; 'sweep' sends the list below. */
	token?: SendTokenCardModel;
	/** sweep only: "3 tokens · Ethereum" plus the per-token rows. */
	sweepSummary?: string;
	sweepRows?: SweepRowModel[];
	/** single only: the big enterable amount. */
	amount?: { value: string; fiat: string; denomLabel: string };
	/** single and sweep: one recipient field. */
	recipient?: {
		label: string;
		lines: [string, string];
		/** The whole address, when there is one: the identicon viewer's seed. */
		address?: string;
		identiconSvg: string;
		pickLabel: string;
		/** sweep shows a scan button beside the picker; single does not. */
		scanLabel?: string;
		/** sweep's "every token goes to the same address". */
		note?: string;
	};
	/** single: the "+ add recipient" that turns this into a split. */
	addRecipient?: string;
	/** split only. */
	recipients?: RecipientCardModel[];
	/** split only: add / from contacts / import, as ghost pills. */
	recipientActions?: { id: 'add' | 'contacts' | 'import'; label: string }[];
	/** split and sweep: the total line above the fee. */
	summary?: { label: string; value: string };
	fee: FeeRowModel;
	/**
	 * The core's last refusal, in the corpus's words (spec 038 #D4): an
	 * estimate that failed, an address that is not one. Live only; the
	 * phone raised these as native alerts, this shell had logged them.
	 */
	alert?: string;
	cta: string;
}

/** SD2e — the contact picker sheet. */
export interface ContactPickModel {
	title: string;
	closeLabel: string;
	searchPlaceholder: string;
	scanRow: string;
	groupsTitle: string;
	groups: { name: string; count: string; colors: [string, string] }[];
	contactsTitle: string;
	contacts: {
		name: string;
		group?: string;
		addressDisplay: string;
		/** The seed of the artwork — what the identicon viewer shows beside it. */
		addressFull: string;
		identiconSvg: string;
	}[];
}

/** SD2f — the fee-token sheet. */
export interface FeeTokenPickModel {
	title: string;
	closeLabel: string;
	hint: string;
	estimateLabel: string;
	rows: {
		mark: TokenMarkModel;
		symbol: string;
		balanceLabel: string;
		fee: string;
		selected: boolean;
		/**
		 * The core's `insufficient`: this coin cannot cover the fee, so the
		 * row is shown for context and answers to nothing (invariant ⑧). The
		 * native row is always offered, balance or not — which is how a send
		 * came to be paid in a coin the account did not hold (issue 211).
		 */
		insufficient?: boolean;
		/** Why it cannot be chosen, when it cannot. */
		insufficientNote?: string;
	}[];
}

/** SD2c — the recipient importer. */
export interface BatchImportModel {
	title: string;
	closeLabel: string;
	units: { fiat: string; token: string };
	unit: 'fiat' | 'token';
	pasteValue: string;
	pastePlaceholder: string;
	importFile: string;
	template: string;
	rateSection: string;
	rateLabel: string;
	rateValue: string;
	/** Live only (spec 038 #E6): the rate as typed, editable in place. */
	rateInput?: string;
	/** The person overrode the fetched rate; the reset control shows. */
	rateEdited?: boolean;
	/** "Auto" — the reset control's word. */
	rateReset?: string;
	rateHint: string;
	parsedLabel: string;
	rows: { ok: boolean; address: string; conversion: string }[];
	rejectedText?: string;
	cta: string;
	ctaDisabled: boolean;
}

/** SD3 — the confirmation. */
export interface SendConfirmModel {
	header: FlowHeaderModel;
	/** "120 USDT" / "3 assets". */
	amount: string;
	/** "≈ $120.00" / "Total ≈ $200.90 · Ethereum". */
	subline: string;
	facts: FactRowModel[];
	/** SD3b's recipient list / SD3c's asset list, as a second card. */
	breakdown?: BreakdownRowModel[];
	/** The core's last refusal, worded — see `SendFormModel.alert`. */
	alert?: string;
	cta: string;
}

/**
 * One part of a batch: a recipient with an avatar (a split) or an asset with
 * its mark (a sweep). The same row on the confirm, the receipt and the
 * transaction detail (spec 038 #D2), so what was signed, what is landing and
 * what landed read as one thing.
 */
export interface BreakdownRowModel {
	lead?: TokenMarkModel;
	identiconSvg?: string;
	/** With `identiconSvg`: its seed, for the viewer. */
	address?: string;
	label: string;
	value: string;
	/**
	 * A sentence under the label. The split's confirm uses it to repeat the
	 * form's duplicate-payee warning (issue 203) on the last screen before a
	 * signature; the receipt and the detail leave it unset — there, the batch
	 * is already history and nothing can be edited.
	 */
	note?: string;
}

export type ReceiptStage = 'submitting' | 'submitted' | 'confirmed' | 'failed';

/** SD4 — the receipt, in whichever of its states the transaction is in. */
export interface SendReceiptModel {
	header: FlowHeaderModel;
	stage: ReceiptStage;
	title: string;
	/** Up to two lines under the title. */
	captions: string[];
	/** submitted / confirmed: the hash and its copy affordance. */
	hash?: { label: string; value: string; copyLabel: string };
	viewOnExplorer?: string;
	/**
	 * Spec 038 #D3 — live, while submitted: when the relay accepted the op and
	 * how long this chain usually takes, so the screen can count rather than
	 * spin. The screen owns the clock; the sentence is the corpus's.
	 */
	eta?: {
		submittedAtMs: number;
		typicalS: number;
		/** "Gnosis typically confirms in ~15s" — already filled. */
		typicalLine: string;
		/** "{{elapsed}}s elapsed — almost there" — the screen fills the number. */
		elapsedTemplate: string;
		/** Past twice the typical time. */
		slowLine: string;
	};
	/** Spec 038 #D2 — a split: "N recipients", then every one of them. */
	breakdownTitle?: string;
	breakdown?: BreakdownRowModel[];
	/** The single bottom button: "Close · keep running" or "Done". */
	cta: string;
	ctaAccent: boolean;
}

/* ------------------------------------------------------------- the screens */

/**
 * One mobile state: the screen under it, and the sheet over it.
 *
 * Sheets are modelled as an overlay on a base screen rather than as states of
 * their own because that is what they are — A2 is the history with a
 * transaction over it, and the history behind it is still the history.
 */
export interface FlowScreenModel {
	state: FlowStateId;
	base: FlowBaseModel;
	sheet?: FlowSheetModel;
	/** 1 or 1.35 — multiplies the text tokens via `--text-scale`. */
	textScale: number;
}

export type FlowBaseModel =
	| { kind: 'receive-list'; model: ReceiveListModel }
	| { kind: 'share-card'; model: ShareCardModel }
	| { kind: 'scan'; model: ScanModel }
	| { kind: 'history'; model: HistoryModel }
	| { kind: 'assets'; model: AssetsModel }
	| { kind: 'send-pick'; model: SendPickModel }
	| { kind: 'send-form'; model: SendFormModel }
	| { kind: 'send-confirm'; model: SendConfirmModel }
	| { kind: 'send-receipt'; model: SendReceiptModel };

export type FlowSheetModel =
	| { kind: 'receive-qr'; model: ReceiveQrModel }
	| { kind: 'tx-detail'; model: TxDetailModel }
	| { kind: 'token-detail'; model: TokenDetailModel }
	| { kind: 'add-token'; model: AddTokenModel }
	| { kind: 'contact-pick'; model: ContactPickModel }
	| { kind: 'fee-token'; model: FeeTokenPickModel }
	| { kind: 'batch-import'; model: BatchImportModel };

/**
 * One desktop state: what the third column holds.
 *
 * The panel title and its back affordance live here rather than in each body
 * model because the panel is the chrome and the bodies are interchangeable —
 * the same `send-form` body appears under 转账 whether it was reached from the
 * dock or from a token detail.
 */
export interface DesktopFlowModel {
	state: DesktopFlowStateId;
	title: string;
	/** Present once the panel is more than one level deep. */
	backLabel?: string;
	closeLabel: string;
	body: DesktopFlowBody;
}

export type DesktopFlowBody =
	| { kind: 'receive-list'; model: ReceiveListModel }
	| { kind: 'receive-qr'; model: ReceiveQrModel }
	| { kind: 'history'; model: HistoryModel }
	| { kind: 'tx-detail'; model: TxDetailModel }
	| { kind: 'assets'; model: AssetsModel }
	| { kind: 'add-token'; model: AddTokenModel }
	| { kind: 'send-pick'; model: SendPickModel }
	| { kind: 'send-form'; model: SendFormModel }
	| { kind: 'send-confirm'; model: SendConfirmModel }
	| { kind: 'send-receipt'; model: SendReceiptModel }
	| { kind: 'contact-pick'; model: ContactPickModel }
	| { kind: 'fee-token'; model: FeeTokenPickModel }
	| { kind: 'batch-import'; model: BatchImportModel };
