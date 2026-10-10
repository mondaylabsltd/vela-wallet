package app.getvela.wallet.feature.signing

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/**
 * Signing view models (spec 022, data-model.md §3 — the Android port of the
 * web's `src/lib/signing/model.ts`): the universal renderer.
 *
 * A scenario is a header, an ORDERED list of blocks, and a fixed footer. Every
 * one of the 33 CS mocks is expressible that way, and nothing in the renderer
 * knows what "a swap" is: the six-rung ERC-7730 degradation ladder is made
 * structural, so a deeper rung emits more warning blocks and fewer decoded
 * ones instead of forking the layout.
 */

enum class SigningScreenState {
    CS1, CS2, CS3, CS4, CS5, CS6, CS7, CS8, CS9, CS10, CS11,
    CS12, CS13, CS14, CS15, CS16, CS17, CS18, CS19, CS20, CS21, CS22,
    CS23, CS24, CS25, CS26, CS27, CS28, CS29, CS30, CS31, CS32, CS33,

    /**
     * The wallet's own request (the key backup to Ethereum, the core's
     * `first_party`): no requester header, its intent and the ✕ in one row,
     * the rows Network / Address / Public keys, the speed control open with
     * a tier still measuring. The canon's cs34/cs35 (a cap being typed) are
     * not drawn on this platform yet; the number stays the canon's.
     */
    CS36,

    /**
     * Spec 102 D4: the hand-off card — an account that reviews and signs on
     * the official page, whose check matches the published build list (CS37);
     * the same request when the page could not be checked, so Open stays shut
     * (CS38); and the page open, waiting for its answer (CS39).
     */
    CS37, CS38, CS39,

    /**
     * Spec 102 core round, the card on its own — what a send's hand-off
     * raises when no send confirm is on screen: its fee + speed row
     * (`handoffFeeRow`), the key named by its
     * place (D-17), the page matching (CS40); the same card while a check
     * older than a day runs again, Open off (CS41); and a dApp request on a
     * self-hosted page whose build is new to Vela — the line asks, "Trust
     * this version" answers, Open off until it is trusted (CS42).
     */
    CS40, CS41, CS42,

    /**
     * Spec 102 integration: a key ceremony waiting on a self-hosted page — what
     * it is doing there (its own title, as the card's line under "Waiting
     * for the signing page…") and its key row, as the page draws its own: a key being made
     * there ("New key on | Phone or tablet", CS43) and a sign-in ("Confirm
     * with | This device", CS44). The card a create or a sign-in raises on
     * its own (`TrustedSignerWaitingSheet`).
     */
    CS43, CS44,

    /**
     * The correctness batch — boards drawn through the LIVE builders
     * (`SigningLive.feeModel`, the core's confirm-block keys, `aftercareReceipt`):
     * the confirm held while the account's previous transaction on this
     * network is in flight (CS45); a fee coin the machine switched, measured
     * again with its own fee leg (CS46, provisional) and settled (CS47); a fee
     * whose account read failed — the chain's nodes out of reach (CS48) vs a
     * fault inside the app (CS49), the row and the footer naming the same
     * cause; and a relay refusal told by its reason, another transaction of
     * the account having gone first (CS50).
     *
     * The integration round (PR 2 notes 1, 9), each fee view written by the
     * real fee machine (`FeeBoards`): CS48/CS49 retried by the core itself —
     * the reason on the row, "Retrying…" under the confirm, nothing asking
     * for a tap; its re-ask out, the reason kept beside the turning sign
     * (CS51); a failure only a tap fixes — "Tap to retry" on the row, "Tap it
     * to retry" under the confirm (CS52); and the sheet's own failure told by
     * the relay's reason (`failure_refusal_key`): another of the account's
     * operations holds the nonce — since the polish round "Not sent yet",
     * calmly, with Try again (CS53, `failure_not_sent`).
     *
     * The polish round: a fee the relay answered would fail in the coin
     * chosen — "Pay with another coin" on the row and "This would fail if
     * sent as it is." under the confirm (CS54), the coins its tap opens
     * (CS56), and the dash, no control, when no other coin is left (CS55).
     */
    CS45, CS46, CS47, CS48, CS49, CS50, CS51, CS52, CS53, CS54, CS55, CS56,

    /**
     * Nothing jumps when the simulation's verdict lands (the 102 device run):
     * CS1's transfer while its simulation is out — the verdict's place kept,
     * nothing in it (CS57) — and the same sheet once a node that cannot
     * simulate has answered: 「Vela 未能检查这笔交易的结果」 in that place
     * (CS58). The two are the same height, and the confirm is where it was.
     */
    CS57, CS58,

    /**
     * The verdict's place holds EVERY verdict a sheet can end on (the
     * integration's note 12 — it held "could not check" alone, and a taller
     * one still pushed the sheet up): CS57's waiting sheet once the node
     * says the call is expected to fail, with its reason (CS61); once it
     * says nothing of theirs moves (CS62); once it shows the one balance a
     * send moves (CS63), and the two a swap does (CS64). Each is the same
     * height as CS57, and the confirm is where it was. Through the live
     * builders.
     */
    CS61, CS62, CS63, CS64,

    /**
     * A verdict TALLER than the place (PR 3 final note F2): three balance
     * rows (CS65), and a received token nothing could verify, with its
     * warning under the rows (CS66). The place scrolls inside itself — the
     * sheet is CS57's height and the confirm is where it was.
     */
    CS65, CS66,

    /**
     * The fee's worth waits for the display currency like every fiat figure
     * (the core's withhold rule): CS1's transfer with a settled fee while the
     * currency is on its way — the fee in its coin, no "≈ $" beside it (CS59)
     * — and the SAME sheet once it commits, the worth in the person's money
     * on the same one line (CS60). Through the live [SigningLive.feeModel].
     */
    CS59, CS60,
}

/**
 * What a list of blocks SAYS: a held place counts as the block that took it,
 * and as nothing while it is still only room.
 */
fun List<SigningBlock>.said(): List<SigningBlock> = flatMap { block ->
    if (block is SigningBlock.Held) listOfNotNull(block.shown) else listOf(block)
}

/** Semantic weight. `Accent` is the intent sentence; the rest colour warnings. */
enum class SigningTone { Neutral, Accent, Success, Caution, Danger }

@Immutable
data class AmountLine(
    /** Rendered ahead of the value and coloured with it: "−", "+", or "". */
    val sign: String,
    val value: String,
    val symbol: String,
    /**
     * The coin's mark — the send flow's token mark (`WalletLive.mark` on the
     * request's chain): its logo over its ticker's letters. Never a first
     * letter on a tinted disc (USDC and USDT were both "U").
     */
    val token: app.getvela.wallet.feature.flows.TokenMarkModel? = null,
    val fiat: String? = null,
    /** "支付" / "最少收到" / "存入资产" — the line's own small label. */
    val caption: String? = null,
    val tone: SigningTone = SigningTone.Neutral,
)

@Immutable
data class SigningRow(
    val label: String,
    val value: String,
    val valueTone: SigningTone = SigningTone.Neutral,
    val mono: Boolean = false,
)

@Immutable
data class AllowanceChip(val id: String, val label: String, val state: ChipState) {
    enum class ChipState { Idle, Selected, Disabled }
}

/** Spec 044: the custom-amount field the guard's editor opens. */
@Immutable
data class AllowanceInput(val value: String, val symbol: String, val placeholder: String, val error: String? = null)

@Immutable
data class PartyBadge(val text: String, val tone: SigningTone)

@Immutable
data class BalanceDeltaRow(val symbol: String, val delta: String, val tone: SigningTone)

@Immutable
sealed interface SigningBlock {
    /** The eyebrow above the hero — "发送", "授权", "盲签". */
    data class Intent(val text: String, val tone: SigningTone) : SigningBlock

    /** The hero number. `card` boxes it in its tone (cs28's burn intercept). */
    data class Amount(
        val line: AmountLine,
        val card: Boolean = false,
        val note: String? = null,
    ) : SigningBlock

    /** Two amount lines with the ↓ badge between them. */
    data class Swap(val pay: AmountLine, val receive: AmountLine) : SigningBlock

    data class Nft(val id: String, val collection: String) : SigningBlock

    /** The one-sentence plain-language summary. */
    data class Sentence(val text: String, val tone: SigningTone) : SigningBlock

    data class Allowance(
        val label: String,
        val value: String,
        val valueTone: SigningTone,
        val chips: List<AllowanceChip>,
        val note: String? = null,
        val resultingTotal: SigningRow? = null,
        /** Spec 044: present while the Custom chip is selected. */
        val custom: AllowanceInput? = null,
        /** The batch leg this card caps; `null` = the single approval. The
         *  sheet routes its chips and field to the leg's own events. */
        val leg: Int? = null,
    ) : SigningBlock

    data class Party(
        val label: String,
        val name: String,
        val address: String? = null,
        val badge: PartyBadge? = null,
    ) : SigningBlock

    data class Rows(val rows: List<SigningRow>) : SigningBlock

    data class Warning(val tone: SigningTone, val text: String) : SigningBlock

    data class Positive(val text: String) : SigningBlock

    /** Message, hex, typed-data JSON or calldata — always monospace. */
    data class Code(val lines: List<String>, val note: String? = null) : SigningBlock

    /** A batch step or a Safe inner call. */
    data class Card(
        val title: String?,
        val rows: List<SigningRow>,
        val tone: SigningTone,
    ) : SigningBlock

    data class Balances(
        val title: String,
        val rows: List<BalanceDeltaRow>,
        val note: String? = null,
        val noteTone: SigningTone = SigningTone.Neutral,
    ) : SigningBlock

    /**
     * A place kept for a block that arrives late — the simulation's verdict.
     *
     * The sheet is bottom-anchored and as tall as its content, so a card
     * that appears a second after the sheet opened pushed everything above
     * it up (the 102 device run: 「Vela 未能检查这笔交易的结果」 on every
     * Gnosis request). The place is there from the first frame — as tall as
     * the TALLEST of [rooms], each drawn unseen and unsaid — and [shown]
     * takes it when it lands, centred in it.
     *
     * [rooms] are the verdicts a sheet can end on (`SigningLive.verdictRooms`):
     * "could not check", "expected to fail" at the longest reason the core
     * prints, "no asset changes", and a balance card of the usual number of
     * moves. It was the "could not check" card alone, which every other
     * verdict is taller than: a swap's two balance rows still pushed the
     * sheet up when they landed.
     *
     * PR 3 final note F2. The place is never BLANK: while the verdict is out
     * a quiet skeleton stands in it, the size of the place, said to a screen
     * reader as [waiting] ("Checking…"). And nothing grows it: a [shown]
     * taller than the place (a third balance row, an unverified token's
     * warning) scrolls INSIDE it, its cut edge faded, instead of pushing the
     * form and the confirm — it grew the sheet by the difference.
     */
    data class Held(
        val rooms: List<SigningBlock>,
        val shown: SigningBlock?,
        /** What the skeleton says to a screen reader while [shown] is still out. */
        val waiting: String = "",
    ) : SigningBlock
}

@Immutable
data class TechIdentity(
    val role: String,
    val name: String,
    val address: String,
    /**
     * What stands beside it: a token's mark (`FactLead.Token`, the send
     * flow's) or a person's identicon (`FactLead.Identicon`, from the
     * address) — the leads a fact row draws, never a letter avatar.
     */
    val lead: app.getvela.wallet.feature.flows.FactLead? = null,
)

@Immutable
data class TechModel(
    val title: String,
    /** Byte count shown on the collapsed row when there is one. */
    val summary: String? = null,
    val functionLabel: String? = null,
    val signature: String? = null,
    val params: List<SigningRow> = emptyList(),
    val identities: List<TechIdentity> = emptyList(),
    val simResult: SigningRow? = null,
    val rawLabel: String? = null,
    val rawHex: String? = null,
    val copyLabel: String,
    val explorerLabel: String,
) {
    /**
     * Nothing to disclose. A refused request nulls every field this card would
     * show (spec 081), and the row still drew itself — expanding it opened an
     * empty panel, which tells a person there is something here and then does
     * not show it. Found on the Xiaomi.
     */
    val isEmpty: Boolean
        get() = summary == null && functionLabel == null && signature == null &&
            params.isEmpty() && identities.isEmpty() && simResult == null &&
            rawLabel == null && rawHex == null
}

@Immutable
data class FeeTokenOption(
    val id: String,
    /**
     * The coin's real mark — the send form's fee-coin sheet's own: its logo
     * on the REQUEST's chain (a native coin wears its chain's), over the drawn
     * ticker when the logo cannot load. Never a first letter (USDC and USDT
     * were both "U").
     */
    val mark: app.getvela.wallet.feature.flows.TokenMarkModel,
    val name: String,
    val balance: String,
    val fee: String,
    val selected: Boolean,
    /** The core's `insufficient`: shown for context, never pickable (invariant ⑧). */
    val disabled: Boolean = false,
    /**
     * Issue #408: why a disabled coin cannot pay, drawn under its row — the
     * core's shortfall, need and have in the coin's own unit. `null` = none.
     */
    val reason: String? = null,
)

@Immutable
sealed interface FeeModel {
    data class OnChain(
        val label: String,
        val value: String,
        /** Present only while the selector is open (cs33). */
        val selectorTitle: String? = null,
        val options: List<FeeTokenOption> = emptyList(),
        /** The speed control under the fee (spec 069) — the send form's own. */
        val speed: app.getvela.wallet.feature.flows.FeeSpeedModel? = null,
        /** The row answers a tap: a failed quote to retry, or more than one coin to choose from. */
        val tappable: Boolean = false,
        /**
         * Issue #262: why the confirm is shut — the paying coin is not there;
         * issue #408: or no coin on offer can pay, said as that.
         */
        val warning: String? = null,
        /** Spec 079: the send form's refresh control, and whether a measurement is out. */
        val refreshLabel: String? = null,
        val refreshing: Boolean = false,
        /**
         * The figure is being measured again — a measurement is out, or the
         * one in hand is another speed's. The line under the card keeps its
         * last words' height meanwhile ([HeldLine]), so the sheet does not move.
         */
        val measuring: Boolean = false,
        /**
         * The last [warning], kept while the fee is measured again: its room
         * only. The verdict is about the last quote, so it is not said —
         * drawn invisible and silent — and it goes when a fee lands with
         * nothing to say. Set by the sheet, never by the builder.
         */
        val heldWarning: String? = null,
        /**
         * Before any [warning] was said: the line the core already knows the
         * first figure will bring — "no coin can pay", when no coin has
         * anything to pay from ([FeeView.nothing_to_pay_from]) — held the
         * same way while it is measured, so its landing does not move the
         * confirm.
         */
        val reserve: String? = null,
        /** The chevron: only where a tap opens the coin list. */
        val chevron: Boolean = true,
        /**
         * PR 2 polish: the row has coins to open, but a tap does not open them
         * now (a failure it retries, or one it cannot help): the chevron's room
         * is kept, unmarked, so the figure does not move when it comes back.
         */
        val chevronRoom: Boolean = false,
        /**
         * The display currency is on its way: the fee is its coin amount
         * alone, and its worth will join the line when the currency commits.
         * The row keeps the room that longer line needs from now, so the
         * confirm under it does not move when it lands.
         */
        val worthRoom: Boolean = false,
    ) : FeeModel

    /** Off-chain signature: the ✓ line, in place of a fee row. */
    data class OffChain(val note: String) : FeeModel

    /** Nothing at all — cs20–cs22, where there is no fee and no reassurance. */
    data object Hidden : FeeModel
}

/**
 * Spec 102 D4: the hand-off card — the account reviews and signs on a page,
 * so the sheet does not repeat the preview. Where (the title), with which key
 * (the "Confirm with | …" row), what is trusted about the page (its integrity
 * line, or why it will not open), and Open — enabled only when the line says
 * the page opens and the request may be confirmed.
 */
@Immutable
data class HandoffModel(
    val title: String,
    /**
     * The key row, drawn like the sheet's "Signing account | name" row:
     * "Confirm with | Savings" / "Confirm with | This device" (the core's
     * `KeyLabel`); `null` when nothing names the key.
     */
    val key: KeyRowModel?,
    /**
     * The page's NAME, as Settings names it (D6): 「Vela 官方签名页」, the
     * person's label, or "Self-hosted · domain".
     */
    val pageName: String,
    /** The page's address, without its scheme — drawn under [pageName] unless the name already says it. */
    val page: String,
    val integrity: app.getvela.wallet.feature.settings.components.IntegrityLineModel,
    val open: String,
    /**
     * The fee this operation was priced at, and its speed — one quiet row
     * under the key row (core round 5, `handoffFeeRow`), no control: the fee
     * was chosen before the hand-off. Drawn only on a screen that shows no
     * fee of its own, so a fee is on screen once — the sheet a send's
     * hand-off raises when no send confirm is on screen. `null`: no row — a
     * message, a fee not settled for the speed in force, or a surface whose
     * own fee row sits right above the card (the dApp sheet's, the send
     * confirm's: each draws the card in its confirm's place).
     */
    val fee: HandoffFeeModel? = null,
    /**
     * "Trust this version" under a self-hosted page's question
     * (`settings.signing.pageTrust`), when its line asks; `null` otherwise.
     */
    val trust: String? = null,
)

/** The hand-off card's fee row: "Network fee  ~0.00012 ETH · ≈$0.31", and the speed's name. */
@Immutable
data class HandoffFeeModel(
    val label: String,
    val value: String,
    val tier: String?,
    /** The fee's worth is withheld until the display currency commits: the row keeps the room the longer line needs. */
    val worthRoom: Boolean = false,
)

/**
 * Spec 102: a key row — "Confirm with | Phone or tablet", "New key on | This
 * device" — the core's `KeyLabel` in the person's words: [label] from its
 * `label_key`, [value] its name or its place's title.
 */
@Immutable
data class KeyRowModel(val label: String, val value: String)

/**
 * The signing page is open (spec 071): the sheet says so instead of
 * offering the confirm, with a way back to the page and a way out. A key
 * ceremony (spec 102) names its key: [key] is "New key on | …" while a key is
 * made, "Confirm with | …" when one signs in or proves; `null` for a
 * signature, whose hand-off card already named it.
 */
@Immutable
data class TrustedSignerWaitModel(
    val title: String,
    val hint: String,
    val reopen: String,
    val cancel: String,
    val key: KeyRowModel? = null,
)

@Immutable
data class SigningScreenModel(
    val state: SigningScreenState,
    val dappName: String,
    val dappHost: String,
    val dappLetter: String,
    val dappTint: Color,
    val networkName: String,
    val networkDot: Color,
    /**
     * The wallet asking ITSELF (the key backup) — the core's `first_party`,
     * never read off the request's bytes or origin. There is no requester to
     * name: no mark, no name, no network chip (its rows say the network). The
     * header is one row, [headline] and the ✕.
     */
    val dappOwn: Boolean = false,
    /**
     * The wallet's own request: what it does ("复制钱包记录"), drawn as the
     * header's title beside the ✕. The intent block it comes from is not
     * repeated below. `null` for a site's request, whose header names the site.
     */
    val headline: String? = null,
    /** The site's own icon, tried in order OVER the letter (founder ruling 2026-09-19). Https only. */
    val dappIconUrls: List<String> = emptyList(),
    /** The chain's logo from the chain-data endpoint; the dot shows until it lands. */
    val networkLogoUrl: String? = null,
    /** Spec 071: the signing page is open; the confirm gives way to this. */
    val trustedSignerWait: TrustedSignerWaitModel? = null,
    /** Spec 102 D4: the account signs on a page — the confirm is this card's Open. */
    val handoff: HandoffModel? = null,
    /** Spec 071: why the last Trusted Signer attempt did not sign. */
    val trustedSignerNotice: String? = null,
    val blocks: List<SigningBlock>,
    val tech: TechModel,
    /** cs29 ships the disclosure open — the whole point of that mock. */
    val techOpen: Boolean,
    /** `null` under a refusal (spec 081): a fee for a transaction nobody will send. */
    val fee: FeeModel?,
    val signerLabel: String,
    val signerName: String,
    val signerSeed: String,
    /**
     * The confirm's words — the action alone ("确认兑换", "签名", "复制钱包记录"),
     * on a tap button (issue #461: the Send screen's Confirm, not a slide).
     * There is no reject BUTTON anywhere in this vocabulary; the header's ✕ is
     * the explicit refusal, and since spec 079 nothing else closes the sheet
     * (owner ruling: no swipe, scrim or Back rejection).
     *
     * `null` under a refusal: a dead confirm reads as an option somebody
     * merely failed to use, rather than one the wallet never offered.
     */
    val confirmAction: String?,
    val confirmEnabled: Boolean,
    val panelTitle: String,
    /**
     * Spec 099 R7: why the confirm is shut, one line under it — the core's
     * `ConfirmState.key`, translated. `null` while it is armed, or where the
     * sheet already says it its own way.
     */
    val confirmBlockLine: String? = null,
    /** Spec 079: the ✕'s label — the sheet's one explicit close. */
    val closeLabel: String = "",
    /**
     * Spec 079: once the person has approved, the sheet stops being a form and
     * shows this — the send receipt's own model and words, so a dApp
     * transaction and a send look the same while they land.
     */
    val receipt: app.getvela.wallet.feature.flows.SendReceiptModel? = null,
    /**
     * Which request this sheet is drawing (the request's id; the gallery's
     * state). What the sheet holds across frames — a line kept while the fee
     * is measured again — is held per request, never carried to the next.
     */
    val requestKey: String = "",
)

/**
 * A line whose room outlives its words. The signing sheet is anchored at the
 * bottom and wraps its content, so a line that came and went with every 30 s
 * re-quote, speed pick and refresh moved the whole form under the person's
 * eyes (~33 px on the Xiaomi; the web measured 29). The web's rule, kept
 * here: what is not said now is drawn invisible and silent, at the last
 * words' height. A plain holder, not state: reading it schedules no
 * recomposition, and the same inputs always give the same line.
 */
internal class HeldLine {
    private var last: String? = null

    /**
     * The line under the fee card: [line] when there is one; while
     * [measuring], the last one there was (to hold its room, not to say it),
     * or — before any — [reserve]; otherwise none — a fee landed with
     * nothing to say lets it go.
     */
    fun next(line: String?, measuring: Boolean, reserve: String? = null): String? {
        if (line != null || !measuring) last = line
        // Before any line was said, while measuring: the one the core already
        // knows the first figure will bring ([FeeModel.OnChain.reserve]).
        return line ?: last ?: reserve.takeIf { measuring }
    }

    /**
     * The confirm's note: the last one said, said now or not. `null` until a
     * note has been said — a board that never shut its confirm gains no blank
     * line — and from then on its line stays.
     */
    fun room(line: String?): String? {
        if (line != null) last = line
        return last
    }
}

/** The signed-in wallet's identity over the fixture's signer row. */
fun SigningScreenModel.withIdentity(name: String, address: String): SigningScreenModel =
    copy(signerName = name, signerSeed = address)
