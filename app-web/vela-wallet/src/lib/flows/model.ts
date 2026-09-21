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
	| 'sd2g'
	| 'sd2h'
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
	/**
	 * One calm sentence under the row, saying why its value is what it is. The
	 * send confirm uses it for a speed taken because it was free (issue 686),
	 * so the tier and its reason reach the last screen before a signature
	 * together. Drawn by the screens that set it; the row itself ignores it.
	 */
	note?: string;
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
	/**
	 * What the list says when it has no rows to show (issue 209): "no tokens
	 * with balance" for an account that holds nothing, "no matching tokens"
	 * when a filter or a search hid them. Live only — the drawn picker always
	 * has rows.
	 */
	empty?: string;
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
	/** Who the row pays, as it reads at rest: a name when there is one, else the short address. */
	name: string;
	/**
	 * The short address UNDER a name. A row that arrived from a spreadsheet or
	 * the book carries a name, and the editable card used to drop it and show
	 * a clipped address instead — so a payroll of forty people read as forty
	 * hex strings. The name is what the person recognises; the address is what
	 * is paid, so a named row shows both.
	 */
	addressShort?: string;
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
	/** Live rows only: what an empty address field asks for. */
	addressPlaceholder?: string;
	pickLabel?: string;
	removeLabel: string;
	/**
	 * Live rows only: what the core says is wrong with a field that has
	 * something IN it (`split_row_issues`, state `invalid`) — "Invalid address",
	 * "Not a valid amount". An empty field gets no note: the row is unfinished,
	 * not mistaken, and its prompt already says what it wants.
	 */
	addressNote?: string;
	amountNote?: string;
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
	/** The coin half — "0.0021 ETH" — or the "…" / "—" that stands in for it. */
	value: string;
	/**
	 * What that costs — "≈ $0.55" — when anything can say (issue 231). Its own
	 * field so a narrow row can drop it onto a second line WHOLE: as one string
	 * the row broke wherever the text ran out, which was inside "Network fee".
	 */
	valueFiat?: string;
	openLabel: string;
	/**
	 * The refresh affordance's accessible name (spec 068). The fee is the one
	 * figure on this screen that moves on its own, and until now a person
	 * could neither re-read it nor be told it had gone quiet.
	 */
	refreshLabel: string;
	/** A refresh is out. The control says so, so a tap is never ambiguous. */
	refreshing?: boolean;
	/**
	 * The quote's 30s TTL elapsed (`FeeView.stale`, which had no consumer in
	 * this shell at all). Deliberately CALM wording and muted tone: a figure
	 * that is merely old is not a fault, and dressing it as one would teach
	 * people to fear a fee row that is doing its job.
	 */
	staleNote?: string;
}

/** One row of the folded speed control (spec 068). */
export interface FeeSpeedOptionModel {
	/** The wire tier name — `fast` / `standard` / `slow`. */
	id: string;
	/**
	 * The SPEED itself — 超快 / 标准 / 较慢. Never a number: the control's own
	 * label asks about speed, so the answer has to be one. The gas price below
	 * is supporting information BESIDE the name, not the name.
	 */
	label: string;
	/**
	 * What that speed buys, one short line under the name — "Lowest fee, if
	 * you can wait". The owner's ruling (spec 068): under a heading that asks
	 * about speed every option has to BE a speed, so the advantage cannot live
	 * in the name. It lives here, and it is the reason the slow tier reads as
	 * a choice rather than a defect.
	 */
	detail?: string;
	/**
	 * This option's OWN fee, priced at this option's tier — "0.0021 ETH", or
	 * the "…" / "—" that stands in for it. The whole point of opening the
	 * control is seeing the trade, and a row of names without prices asks
	 * somebody to choose blind.
	 */
	value: string;
	/** What that costs, when anything can price it — "≈ $0.55". */
	valueFiat?: string;
	/**
	 * What this speed actually BUYS: the effective gas price the chain will
	 * charge at this tier — "300 gwei", "3,000 wei" (issue 684).
	 *
	 * The fee beside it stops distinguishing the tiers on any chain whose real
	 * cost is under a cent, because `fee_policy` clamps all three to the $0.01
	 * floor; the owner reported three identical fees on seven networks. The
	 * tiers still buy different inclusion — each signs a different tip — and
	 * this is where that difference becomes visible. Quiet and secondary: the
	 * fee stays the primary figure.
	 *
	 * Absent when there is no honest number — a chain with no priority fee at
	 * all (Tempo), or a quote that did not report one. Never a stand-in 0,
	 * which would claim the tiers are equal. Absent on EVERY row while any
	 * tier is still being measured, because the set is formatted together and
	 * a partial set would re-shape as the rest arrived.
	 *
	 * A range since issue 685 — `0.02011 ~ 0.03016 gwei`, what the speed bids
	 * now and how high it will go (its cap) — or one figure where the two
	 * ends are the same. Already formatted: unit, precision and glyph are
	 * decided over the whole set in `gas-price.ts`, never per option.
	 */
	gasPrice?: string;
	selected: boolean;
}

/**
 * The speed control (spec 068), folded away until the person opens it.
 *
 * Folded, it shows THEIR default (the `fee_tier_pref` core's committed tier),
 * never a hardcoded one, so this row and the Settings row can never disagree.
 * A pick here is ONE-SHOT: it prices and submits this send and never rewrites
 * the stored preference — which is what `onceNote` says out loud.
 */
export interface FeeSpeedModel {
	label: string;
	/** The folded summary: the tier in force for THIS send. */
	value: string;
	open: boolean;
	onceNote: string;
	/**
	 * Why the tier in force is the fastest one when the person's default is
	 * slower (issue 686): on this network it costs no more, so this send takes
	 * it. One calm line under the folded summary, present only while that is
	 * what happened — the screen must never say "Fast" over a Settings row that
	 * says "Slow" without saying why.
	 */
	freeNote?: string;
	/**
	 * This network has one speed (issue 686): every tier settled at the same
	 * fee with no gas-price range to tell them apart, so the options are
	 * replaced by this one statement. Absent whenever anything differs, or
	 * while any tier is still being measured.
	 */
	singleNote?: string;
	/**
	 * Names {@link FeeSpeedOptionModel.gasPrice}, drawn beside it. Under a fee
	 * and in the same numeric face, an unnamed "3,244 wei" reads as a second
	 * amount being charged — and on a floor-clamped chain one that disagrees
	 * with the fee about which tier is dearer. A named figure can at least be
	 * looked up; a bare one leaves the person guessing.
	 */
	gasPriceLabel: string;
	/**
	 * Whether the options carry a gas-price line at all. True while any tier
	 * is still being measured (the line is reserved, empty) and whenever any
	 * tier has a figure, so the rows keep one height instead of losing a line
	 * and regaining it each time a tier re-measures.
	 */
	gasPriceLine: boolean;
	options: FeeSpeedOptionModel[];
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
	amount?: {
		/** What has been typed — `''` when nothing has, never a stand-in "0". */
		value: string;
		/** What the empty field shows instead. */
		placeholder: string;
		/**
		 * The figure's unit, drawn beside it (issue 231): a currency symbol
		 * leads, a currency code or a token symbol follows. At most one is set;
		 * neither is, only when there is no unit to name.
		 */
		adornment: { prefix?: string; suffix?: string };
		fiat: string;
		/** The unit the figure is TYPED in — the entry field's accessible name. */
		denomLabel: string;
		/**
		 * The ⇄ row (spec 021 component 8, E07 FR-3/FR-4), when the core offers
		 * it: `SendView.denom_toggle_shown` decides that it is there at all,
		 * `denom_toggle_enabled` whether pressing it would change anything.
		 * Absent ⇒ no swap on offer and the line beneath is plain text — an
		 * affordance appears only where something can act on it.
		 */
		denomToggle?: { enabled: boolean };
	};
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
	/**
	 * split and sweep: the total line above the fee. `detail` is the same sum
	 * in the display currency; `over` is the core's `split_over_balance` — the
	 * total is more than the account holds, and the figure says so in the
	 * refusal colour BEFORE Continue is pressed rather than after.
	 */
	summary?: {
		label: string;
		value: string;
		detail?: string;
		over?: boolean;
		/** split only: "2.25 ETH left" — the core's `split_remaining`. */
		remaining?: string;
	};
	/**
	 * split only: one amount into every row that has none — offered when a row
	 * has a figure and another does not. The common split is the same amount to
	 * everyone (a bonus, a refund, a group from the book arrives with no amounts
	 * at all), and typing it forty times is the form failing at its one job.
	 */
	fillEmpty?: { label: string; amount: string };
	fee: FeeRowModel;
	/**
	 * The speed control under the fee row (spec 068). Absent in surfaces that
	 * only draw a fee — the gallery's sweep board, say — so nothing has to
	 * invent a tier it cannot honour.
	 */
	speed?: FeeSpeedModel;
	/**
	 * The core's last refusal, in the corpus's words (spec 038 #D4): an
	 * estimate that failed, an address that is not one. Live only; the
	 * phone raised these as native alerts, this shell had logged them.
	 */
	alert?: string;
	/**
	 * split only: why Continue is dark when nothing has been refused — WHICH
	 * recipient is still missing its address or its amount (the core's
	 * `split_row_issues`). Not an alert: nothing went wrong, something is
	 * unfinished.
	 */
	hint?: string;
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

/**
 * A line of the sheet the parser refused, as the person wrote it: there is no
 * address to draw and nobody to name, only the text to find it by and the
 * reason. These were counted ("2 rows skipped") and never shown.
 */
export interface BatchRefusedModel {
	kind: 'refused';
	text: string;
	note: string;
}

/** One parsed line of SD2c: who is paid, what they get, and why not when they do not. */
export interface BatchRowModel {
	kind: 'row';
	ok: boolean;
	/** The sheet's own name for the person, when it has a name column. */
	name?: string;
	/** Short form for the row; `addressFull` is the identicon's seed and the hover text. */
	address: string;
	addressFull: string;
	identiconSvg: string;
	/** What is SENT — "6.642752 BNB" — or a dash when nothing can be. */
	amount: string;
	/** The figure as the sheet wrote it, "5000 USD": fiat mode only. */
	source?: string;
	/** Why the row is skipped, in the corpus's words. */
	note?: string;
}

/** SD2c — the recipient importer. */
export interface BatchImportModel {
	title: string;
	closeLabel: string;
	/** "How the amounts are written" — the toggle was a choice with no question. */
	unitCaption: string;
	units: { fiat: string; token: string };
	unit: 'fiat' | 'token';
	pasteValue: string;
	pastePlaceholder: string;
	/**
	 * The two ways a list arrives without being pasted (issue 205). Each says
	 * what state it is in — a pick that is still reading, a template that was
	 * saved, a file that could not be read — where before two grey words did
	 * the same thing whatever happened.
	 */
	tools: {
		file: { label: string; busy: boolean };
		template: { label: string; saved: boolean };
		/** "xlsx · csv · txt" — extensions, not words. */
		formats: string;
		/** The picked file, by name: a workbook leaves the paste box empty. */
		fileName?: string;
		error?: string;
	};
	/**
	 * Fiat mode only. In token mode no conversion happens, and a rate row there
	 * is a control for something that is not going on — with a sentence under
	 * it ("read as USD…") that is false.
	 */
	rate?: {
		section: string;
		/** "1 BNB" — the left side of the equation. */
		lead: string;
		/**
		 * "≈" while the figure mirrors the market, "=" once the person has
		 * typed their own: a pinned rate is exact, a fetched one is not
		 * (issue 206).
		 */
		sign: '≈' | '=';
		/** The figure, in the person's decimal mark. Empty while unknown. */
		value: string;
		/** "USD" — the unit the figure is in, which the row never said. */
		code: string;
		/** Live only: the figure can be typed in place (spec 038 E6). */
		editable: boolean;
		edited: boolean;
		reset: string;
		hint: string;
		/** `warning`: the rate is unknown and nothing converts until one is typed. */
		hintTone: 'plain' | 'warning';
	};
	/**
	 * Token mode's one sentence, where the rate would be: the sheet's figures
	 * ARE the amounts. The template's 5000 is five thousand coins there.
	 */
	unitHint?: string;
	/** Absent until something has been pasted or picked. Source order, refused lines among the rest. */
	preview?: { label: string; rows: (BatchRowModel | BatchRefusedModel)[] };
	/** Skipped rows, rows past the cap — shown together, never one hiding the other. */
	notices: string[];
	/**
	 * What the import adds up to, against what the account holds. This is the
	 * line issue 204 was missing: fifty-nine rows summed to more than the
	 * balance, the core refused, and the screen showed a dark button.
	 */
	total?: {
		label: string;
		value: string;
		/** The sheet's own figures summed, "13,000 USD": fiat mode only. */
		detail?: string;
		balance: string;
		over: boolean;
		/** The refusal, when `over`. */
		overText?: string;
	};
	/**
	 * The form already has people on it: what this import will do to them, and
	 * the way to choose the other. Adding is the default; the core's seed had
	 * always replaced, and no screen had said either.
	 */
	merge?: { note: string; action: string };
	cta: string;
	ctaDisabled: boolean;
}

/** SD3 — the confirmation. */
export interface SendConfirmModel {
	header: FlowHeaderModel;
	/**
	 * The coin being sent, drawn above the figure (founder, 2026-09-17).
	 * The confirm page named the asset in words only while every row beneath
	 * it carried art — the one screen where "which coin is this?" must be
	 * answerable at a glance. Absent on a sweep: several coins, no one mark.
	 */
	mark?: TokenMarkModel;
	/** "120 USDT" / "3 assets". */
	amount: string;
	/** "≈ $120.00" / "Total ≈ $200.90 · Ethereum". */
	subline: string;
	facts: FactRowModel[];
	/** SD3b's recipient list / SD3c's asset list, as a second card. */
	breakdown?: BreakdownRowModel[];
	/**
	 * "First time sending here" — the anti-poisoning tell, on the page that
	 * signs. The core resolves it only while this page is up (`confirm_probes`),
	 * so the form never has it to show. Absent on a split.
	 */
	recipientTag?: string;
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
	/**
	 * The short address under a NAME. On the page that signs, a name is a
	 * claim and the address is what is paid: a row from a spreadsheet can call
	 * any address "Alice", so the confirm never shows the one without the other.
	 */
	detail?: string;
	/**
	 * The label IS an address (nobody named it), so it is set in mono. Said by
	 * whoever built the row — guessing it from "has an address, has no detail"
	 * put every named row of the receipt and the detail page in mono too.
	 */
	mono?: boolean;
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
		/** "~{{remaining}}s remaining" — inside the typical time; the screen fills the number. */
		remainingTemplate: string;
		/** "{{elapsed}}s elapsed — almost there" — past it, where "almost" is true. */
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
