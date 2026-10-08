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
	| 'cs35'
	/** The wallet's own key backup (first-party): its headline, no requester. */
	| 'cs36';

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
			/** The batch leg this card caps; absent = the single approval. The
			 *  sheet routes its chips and field to that leg's own events. */
			leg?: number;
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
	/**
	 * The coin's real mark — the send form's fee-coin sheet's, by the same
	 * rule (`tokenMarkFor`): its logo over its drawn ticker, the chain badge
	 * unless it is the chain's own coin. A first letter on a disc drew USDC
	 * and USDT as the same "U".
	 */
	mark: TokenMarkModel;
	name: string;
	balance: string;
	fee: string;
	selected: boolean;
	/** The core's verdict: this coin cannot pay. Drawn, never pickable (issue 211). */
	insufficient?: boolean;
	/**
	 * Issue 408: why a greyed coin cannot pay, drawn under its row — the
	 * core's shortfall, need and have in the coin's own unit.
	 */
	reason?: string;
}

import type { FeeSpeedModel, ReceiptStage, TokenMarkModel } from '$lib/flows/model';

export type FeeModel =
	| {
			kind: 'onchain';
			label: string;
			value: string;
			/** Present only while the selector is open (cs33). */
			selector?: { title: string; options: FeeTokenOption[] };
			/**
			 * The row has something to DO when it is pressed: ask a failed quote
			 * again, or open the list of coins that can pay. On a chain with one
			 * fee coin and a quote in hand there is nothing to choose — so the row
			 * is a fact, not a control, and is drawn without the chevron and
			 * without the pointer it cannot honour. Android's `tappable`, same
			 * rule (`SigningLive.kt`); the desktop and iOS say the same thing in
			 * their handlers and only forget to say it in the drawing.
			 */
			tappable?: boolean;
			/**
			 * The speed control under the fee (spec 069) — the send form's, so
			 * a dApp transaction is priced at a speed the person can see and
			 * change for this one request. Absent in the gallery.
			 */
			speed?: FeeSpeedModel;
			/**
			 * Why the slide is shut: the coin that pays is not there (issue 262 —
			 * 0 ETH on mainnet, quoted in ETH, signed and never bundled) — or,
			 * spec 079, why there is no fee at all: the relay could not be
			 * reached, and the sheet will ask again by itself.
			 */
			warning?: string;
			/**
			 * Spec 082 G47: the cause line is hidden while a re-quote runs (it
			 * said why the LAST ask failed, under "estimating"), but its height
			 * is kept, so the speed row and the slide below do not jump each
			 * time the sheet asks again. The held words, drawn invisibly: the
			 * reserved line is exactly as tall as the one it stands in for.
			 */
			warningReserved?: string;
			/**
			 * Spec 079: the send form's refresh control — its accessible name.
			 * The owner: "似乎没有刷新网络费的按钮呀". Live only.
			 */
			refreshLabel?: string;
			/** A measurement is out: the control turns and refuses a second tap. */
			refreshing?: boolean;
			/**
			 * The quote's TTL elapsed (`FeeView.stale`) — the send form's calm
			 * note, beside the control that fixes it. Never a warning tone.
			 */
			staleNote?: string;
			/**
			 * The chevron: only where a tap opens the list of coins. A failed
			 * quote with one coin is still tapped to ask again, but opens nothing.
			 */
			chevron?: boolean;
	  }
	/** Off-chain signature: the ✓ line, in place of a fee row. */
	| { kind: 'offchain'; note: string }
	/** Nothing at all — cs20–cs22, where there is no fee and no reassurance. */
	| { kind: 'hidden' };

/** Spec 079: where an approved request stands, drawn with the send receipt's `StatusHero`. */
export interface SigningStatus {
	stage: ReceiptStage;
	title: string;
	/** The request in one line, then what the person may do about the wait. */
	captions: string[];
	/**
	 * The ✕ may close the sheet now — the operation carrying on, the page still
	 * answered. Shut while the passkey prompt is up or has not yet produced a
	 * signature: closed then, a cancelled prompt would leave the page unanswered.
	 */
	closable: boolean;
	/**
	 * Spec 096 F8: the failure's own way out, labelled — Close (the page is
	 * answered the failure now) and, when nothing was sent and it was no
	 * refusal, Try again (`retry`, back to review). Absent while it is no
	 * failure.
	 */
	actions?: { close: string; retry?: string };
}

export interface SigningModel {
	id: SigningStateId;
	dapp: {
		name: string;
		/**
		 * The observed origin's host. EMPTY for the wallet's own request (there
		 * is no site), and when the name IS the host (spec 079: said once).
		 */
		host: string;
		letter: string;
		tint: string;
		/**
		 * The site's own icon, tried in order over the letter (founder ruling
		 * 2026-09-19, superseding spec 022's "never fetch"): the letter is what
		 * shows until one lands and what stays when none exists.
		 */
		iconUrls?: string[];
	};
	network: {
		name: string;
		/** The drawn fallback beneath the logo — a real colour, never a keyword. */
		dot: string;
		/** The chain's logo from the chain-data endpoint; the dot shows until it lands. */
		logoUrl?: string;
	};
	/**
	 * The wallet asking ITSELF — the key backup, which the core marks
	 * `first_party` (never the origin or the bytes: a site can send the same
	 * register() call). There is no requester to show, so the header is this
	 * one line, the request's intent in the sheet's title type, and the ✕ —
	 * no mark, no "Vela Wallet", no network chip (the rows say the network).
	 * `dapp` and `network` are not drawn then, and the intent is not repeated
	 * in `blocks`. Absent for every request a site makes.
	 */
	headline?: { text: string; tone: Tone };
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
	 * The slide. There is no reject BUTTON anywhere in this vocabulary; the
	 * header's quiet ✕ is the refusal, and since spec 079 nothing else closes
	 * the sheet (owner ruling: no swipe, scrim or Escape rejection).
	 */
	confirm: {
		/** The action alone ("Confirm send"): the control's whole label (issue 461). */
		action: string;
		enabled: boolean;
		/** Spec 099 R7: why the slide is shut, in the core's words. Live only. */
		note?: string;
	};
	/** Spec 079: the ✕'s accessible name — the sheet's one explicit close. Live only. */
	closeLabel?: string;
	/**
	 * Spec 079 (F11): the person has approved — the sheet is a status now, not
	 * a form. No fee controls, no slide (never a greyed one): the request's one
	 * line and where it stands. Absent while the request is still a request.
	 */
	status?: SigningStatus;
	/**
	 * Spec 081: the request was refused outright (it would have changed who
	 * controls the account). There is no fee to show and nothing to slide —
	 * the only thing the sheet offers is the way out, labelled with this word.
	 */
	dismissOnly?: string;
	/** Desktop third-column heading — "签名请求". */
	panelTitle: string;
}
