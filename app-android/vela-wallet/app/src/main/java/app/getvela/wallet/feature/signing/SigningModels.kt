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
}

/** Semantic weight. `Accent` is the intent sentence; the rest colour warnings. */
enum class SigningTone { Neutral, Accent, Success, Caution, Danger }

@Immutable
data class TokenMark(val letter: String, val tint: Color)

@Immutable
data class AmountLine(
    /** Rendered ahead of the value and coloured with it: "−", "+", or "". */
    val sign: String,
    val value: String,
    val symbol: String,
    val token: TokenMark? = null,
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
}

@Immutable
data class TechIdentity(
    val role: String,
    val name: String,
    val address: String,
    val mark: TokenMark? = null,
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
    val mark: TokenMark,
    val name: String,
    val balance: String,
    val fee: String,
    val selected: Boolean,
    /** The core's `insufficient`: shown for context, never pickable (invariant ⑧). */
    val disabled: Boolean = false,
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
        /** Issue #262: why the slide is shut — the paying coin is not there. */
        val warning: String? = null,
    ) : FeeModel

    /** Off-chain signature: the ✓ line, in place of a fee row. */
    data class OffChain(val note: String) : FeeModel

    /** Nothing at all — cs20–cs22, where there is no fee and no reassurance. */
    data object Hidden : FeeModel
}

/** The per-request choice of WHERE the signing passkey is; opens in place, like the fee. */
@Immutable
data class SignWithModel(
    val label: String,
    val value: String,
    val open: Boolean,
    val options: List<SignWithOption>,
)

@Immutable
data class SignWithOption(val id: String, val title: String, val selected: Boolean)

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
     * The wallet asking ITSELF (the key backup): its own mark and name, and no
     * host — it is not a site.
     */
    val dappOwn: Boolean = false,
    /** The site's own icon, tried in order OVER the letter (founder ruling 2026-09-19). Https only. */
    val dappIconUrls: List<String> = emptyList(),
    /** The chain's logo from the chain-data endpoint; the dot shows until it lands. */
    val networkLogoUrl: String? = null,
    /** "Sign with · Automatic ›" — where the passkey that signs this is. Live only. */
    val signWith: SignWithModel? = null,
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
     * The slide. There is no reject button anywhere in this vocabulary:
     * dismissing the sheet is the rejection (product contract, SPEC 签名).
     *
     * `null` under a refusal: a dead slide reads as an option somebody merely
     * failed to use, rather than one the wallet never offered.
     */
    val confirmHint: String?,
    val confirmAction: String?,
    val confirmEnabled: Boolean,
    val panelTitle: String,
)

/** The signed-in wallet's identity over the fixture's signer row. */
fun SigningScreenModel.withIdentity(name: String, address: String): SigningScreenModel =
    copy(signerName = name, signerSeed = address)
