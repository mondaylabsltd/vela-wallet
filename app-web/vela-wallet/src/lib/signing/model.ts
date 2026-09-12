/**
 * Signing view models (spec 022, data-model.md §3) — the universal renderer.
 *
 * A scenario is a header, an ORDERED list of blocks, and a fixed footer. Every
 * one of the 33 CS mocks is expressible that way, and nothing in the renderer
 * knows what "a swap" is: the six-rung ERC-7730 degradation ladder is made
 * structural, so a deeper rung simply emits more warning blocks and fewer
 * decoded ones instead of forking the layout.
 */

export type SigningStateId =
	| 'cs1'
	| 'cs2'
	| 'cs3'
	| 'cs4'
	| 'cs5'
	| 'cs6'
	| 'cs7'
	| 'cs8'
	| 'cs9'
	| 'cs10'
	| 'cs11'
	| 'cs12'
	| 'cs13'
	| 'cs14'
	| 'cs15'
	| 'cs16'
	| 'cs17'
	| 'cs18'
	| 'cs19'
	| 'cs20'
	| 'cs21'
	| 'cs22'
	| 'cs23'
	| 'cs24'
	| 'cs25'
	| 'cs26'
	| 'cs27'
	| 'cs28'
	| 'cs29'
	| 'cs30'
	| 'cs31'
	| 'cs32'
	| 'cs33'
	/** Spec 032 phase 39: a cap being TYPED, and the same field refused. */
	| 'cs34'
	| 'cs35';

/** Semantic weight. `accent` is the intent sentence; the rest colour warnings. */
export type Tone = 'neutral' | 'accent' | 'success' | 'caution' | 'danger';

export interface TokenMark {
	letter: string;
	tint: string;
}

export interface AmountLine {
	/** Rendered ahead of the value and coloured with it: "−", "+", or "". */
	sign: string;
	value: string;
	symbol: string;
	token?: TokenMark;
	fiat?: string;
	/** "支付" / "最少收到" / "存入资产" — the line's own small label. */
	caption?: string;
	tone: Tone;
}

export interface KeyValueRow {
	label: string;
	value: string;
	valueTone?: Tone;
	mono?: boolean;
}

/**
 * The typed cap, when `Custom` is the chosen chip.
 *
 * The value is the CORE's `custom_text` rather than a local echo, so a
 * keystroke the machine rejected never appears as though it had been taken.
 */
export interface AllowanceInput {
	value: string;
	/** The coin the number counts in — the field's own label. */
	symbol: string;
	placeholder: string;
	/** The core's verdict on what is typed so far. */
	error?: string;
}

export interface AllowanceChip {
	id: string;
	label: string;
	state: 'idle' | 'selected' | 'disabled';
}

export interface PartyBadge {
	text: string;
	tone: Tone;
}

export interface BalanceRow {
	symbol: string;
	delta: string;
	tone: Tone;
}

export type Block =
	/** The eyebrow above the hero — "发送", "授权", "盲签". */
	| { kind: 'intent'; text: string; tone: Tone }
	/** The hero number. `card` boxes it in its tone (cs28's burn intercept). */
	| {
			kind: 'amount';
			line: AmountLine;
			card?: boolean;
			/** Second line inside the card: "发送到代币自身合约". */
			note?: string;
	  }
	/** Two amount lines with the ↓ badge between them. */
	| { kind: 'swap'; pay: AmountLine; receive: AmountLine }
	| { kind: 'nft'; id: string; collection: string }
	/** The one-sentence plain-language summary. */
	| { kind: 'sentence'; text: string; tone: Tone }
	| {
			kind: 'allowance';
			label: string;
			value: string;
			valueTone: Tone;
			chips: AllowanceChip[];
			note?: string;
			resultingTotal?: KeyValueRow;
			/** Drawn under the chips: the field a custom cap is typed into. */
			custom?: AllowanceInput;
	  }
	| { kind: 'party'; label: string; name: string; address?: string; badge?: PartyBadge }
	| { kind: 'rows'; rows: KeyValueRow[] }
	| { kind: 'warning'; tone: 'caution' | 'danger'; text: string }
	| { kind: 'positive'; text: string }
	/** Message, hex, typed-data JSON or calldata — always monospace. */
	| { kind: 'code'; lines: string[]; note?: string }
	/** A batch step or a Safe inner call. */
	| { kind: 'card'; title?: string; rows: KeyValueRow[]; tone: Tone }
	| {
			kind: 'balances';
			title: string;
			rows: BalanceRow[];
			note?: string;
			noteTone?: Tone;
	  };

export interface TechIdentity {
	role: string;
	name: string;
	address: string;
	mark?: TokenMark;
}

export interface TechModel {
	title: string;
	/** Byte count shown on the collapsed row when there is one ("· 412 字节"). */
	summary?: string;
	fn?: { label: string; signature: string };
	params: KeyValueRow[];
	identities: TechIdentity[];
	simResult?: KeyValueRow;
	raw?: { label: string; hex: string };
	copyLabel: string;
	explorerLabel: string;
}

export interface FeeTokenOption {
	id: string;
	mark: TokenMark;
	name: string;
	balance: string;
	fee: string;
	selected: boolean;
}

export type FeeModel =
	| {
			kind: 'onchain';
			label: string;
			value: string;
			/** Present only while the selector is open (cs33). */
			selector?: { title: string; options: FeeTokenOption[] };
	  }
	/** Off-chain signature: the ✓ line, in place of a fee row. */
	| { kind: 'offchain'; note: string }
	/** Nothing at all — cs20–cs22, where there is no fee and no reassurance. */
	| { kind: 'hidden' };

export interface SigningModel {
	id: SigningStateId;
	dapp: { name: string; host: string; letter: string; tint: string };
	network: { name: string; dot: string };
	blocks: Block[];
	tech: TechModel;
	/** cs29 ships the disclosure open — the whole point of that mock. */
	techOpen: boolean;
	fee: FeeModel;
	signer: {
		label: string;
		name: string;
		identiconSvg: string;
		/** The signing account's address — the identicon viewer's seed. Live only. */
		address?: string;
	};
	/**
	 * The slide. There is no reject button anywhere in this vocabulary:
	 * closing the sheet is the rejection (product contract, SPEC 签名).
	 */
	confirm: { hint: string; action: string; enabled: boolean };
	/** Desktop third-column heading — "签名请求". */
	panelTitle: string;
}
