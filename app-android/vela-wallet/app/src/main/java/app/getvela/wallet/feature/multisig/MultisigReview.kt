package app.getvela.wallet.feature.multisig

import androidx.compose.foundation.clickable
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import app.getvela.wallet.core.designsystem.components.VelaDangerButton
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaValueButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.multisig.core.MsDraft
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MsSignDoor
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.MultisigRiskAsk
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.isNullish
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import kotlinx.serialization.json.JsonElement

/**
 * The one review every signer signs from (§3.6, the owner's Bybit lesson):
 * the core's `MultisigReview` — what moves, who it reaches, the state-diff
 * guard, the member changes and the rule they leave, every concern, the
 * SafeTx fields and the three hashes — drawn, never decoded again here (the
 * web's `ReviewBody.svelte`, iOS's `MsReviewBody`).
 */
@Composable
fun MultisigReviewBody(
    review: JsonElement,
    chainId: Int,
    words: MultisigWords,
    tokenInfo: suspend (Int, String) -> Pair<String, Int>,
    /**
     * S4 (r9): it ran. What it does stays (the reading, its recipients, its
     * member change, the hashes); what was judged BEFORE it ran — whether it
     * may be signed, the concerns, the simulated balance changes, the state
     * guard's "running it changes nothing" — is not said of a transaction
     * that already ran (the desktop's rule).
     */
    executed: Boolean = false,
) {
    val tokens = remember { mutableStateMapOf<String, Pair<String, Int>>() }
    val tokenAddresses = review["simulation"]["deltas"].arr.mapNotNull { it["token"].str }
    LaunchedEffect(tokenAddresses) {
        for (token in tokenAddresses) {
            if (tokens[token.lowercase()] == null) tokens[token.lowercase()] = tokenInfo(chainId, token)
        }
    }
    val network = words.network(chainId)
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
        // Concerns, the core's, in order.
        if (!executed) {
            // r9-9 (core §8): the refusal's own reason (`refused.key`, a full
            // corpus key), said ONCE — never "would change who controls" for a
            // delegatecall; `review.blocked` only from a core without it.
            refusalLine(review, words)?.let { MsNote(it, tone = MsTone.Danger, tag = "multisig.review.blocked") }
            for (concern in shownConcerns(review)) {
                reviewConcern(words, concern)?.let { (text, tone) -> MsNote(text, tone = tone, tag = "multisig.review.concern.${concern.kind}") }
            }
        }
        Reading(review, chainId, words)
        if (!executed) Moves(review, chainId, words, tokens)
        Recipients(review, words)
        MemberChanges(review, words, network)
        if (!executed) StateGuard(review, words, network)
        Technical(review, words)
    }
}

/**
 * §12: the review's place on a sheet, as the core's sign door says it
 * (`review.door`): the review itself once it is read; while it is read, a
 * calm "Reading the transaction…" placeholder; when it could not be read,
 * the reason, "Nothing can be signed until it's read." and "Try again"
 * ([onTryAgain] sends the machine's `review_again`). The button under it is
 * the door's ([MsSignDoor.enabled]) — never pressed on a review not drawn.
 */
@Composable
fun MsReviewDoor(
    door: MsSignDoor,
    words: MultisigWords,
    tag: String,
    onTryAgain: () -> Unit,
    review: @Composable () -> Unit,
) {
    val colors = VelaTheme.colors
    when {
        door.showsReview -> review()
        door.failed -> Column(Modifier.fillMaxWidth().testTag("$tag.reviewFailed"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
            MsNote(reviewFailedReason(door, words), tone = MsTone.Warning, tag = "$tag.reviewFailedReason")
            MsNote(words.t("review.state.blocked"), tag = "$tag.reviewBlocked")
            if (door.retry) MsTextButton(words.t("review.state.tryAgain"), tag = "$tag.reviewTryAgain") { onTryAgain() }
        }
        else -> Row(
            Modifier.fillMaxWidth().testTag("$tag.reviewLoading"),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            CircularProgressIndicator(Modifier.padding(VelaSpacing.xs), color = colors.fgMuted, strokeWidth = VelaSpacing.xs)
            MsText(words.t("review.state.loading"), MsType.Sub, color = colors.fgMuted)
        }
    }
}

/** §12: a failed review's reason, in the core's words (`reason_key`, a full corpus key). */
internal fun reviewFailedReason(door: MsSignDoor, words: MultisigWords): String =
    words.raw(door.reasonKey?.takeIf { it.isNotBlank() } ?: "home.multisig.review.state.unreadable")

/** §12: the draft's sign door — its review's state and sign rule, asked of the core. */
internal fun draftDoor(draft: MsDraft): MsSignDoor = MsRules.reviewDoor(draft.reviewState, draft.review)

/** §12: the event the draft's "Try again" sends — its review, read again. */
internal fun draftReviewAgain(): kotlinx.serialization.json.JsonObject = ev("review_again")

/**
 * r8-14: the review's concerns this sheet draws as their own lines — each
 * said ONCE. A concern the review's other blocks already say is not said
 * again: a first-time recipient wears its "First time" chip on its own row
 * (`firstTimeCue`), and a simulation that could not run or would revert is
 * the moves block's notice (`notice_key`) — the phones drew both, so every
 * sheet said "Vela couldn't check what this transaction does" and "First
 * time sending here" twice. Nothing is dropped that is not drawn elsewhere.
 */
internal fun shownConcerns(review: JsonElement): List<JsonElement> {
    val refused = review["refused"]["key"].str != null
    val recipients = review["recipients"].arr
    val sim = review["simulation"]
    val withheld = sim["kind"].str == "withheld"
    val simNotice = sim["kind"].str != "deltas" && !sim["notice_key"].str.isNullOrEmpty()
    val said = mutableSetOf<String>()
    return review["concerns"].arr.filter { concern ->
        val address = concern["address"].str
        val drawnElsewhere = when (concern.kind) {
            "firstTime" -> recipients.any { it["address"].str.equals(address, ignoreCase = true) && it["firstTimeCue"].bool == true }
            // r10-6 (core §8): a review refused for another reason withholds
            // what running it would do — nothing in the simulation's place,
            // so no "expected to fail" either (the core keeps a `reverts`
            // concern when its state probe reverted; a refusal is not a
            // prediction about running it).
            "unsimulated", "reverts" -> withheld || simNotice
            // r9-9: the refusal line says why it is not signed — not its English again.
            "notSigned" -> refused
            else -> false
        }
        !drawnElsewhere && said.add(concern.toString())
    }
}

/** r9-9: the review's one refusal line — the core's `refused.key`, else (an older core) `review.blocked` when it may not be signed. */
internal fun refusalLine(review: JsonElement, words: MultisigWords): String? =
    review["refused"]["key"].str?.takeIf { it.isNotBlank() }?.let { words.raw(it) }
        ?: words.t("review.blocked").takeIf { review["sign"]["allowed"].bool == false }

/** A concern in words (`stateChange` / `stateUnread` are the guard's own block). */
internal fun reviewConcern(words: MultisigWords, concern: JsonElement): Pair<String, MsTone>? = when (concern.kind) {
    "notSigned" -> words.t("proposal.refused", "reason" to (concern["why"].str ?: "")) to MsTone.Danger
    "lookalike" -> words.t(
        "review.lookalike",
        "address" to concern["address"].str.orEmpty(), "label" to concern["label"].str.orEmpty(), "known" to concern["of"].str.orEmpty(),
    ) to MsTone.Danger
    "toContract" -> words.t("review.toContract", "address" to concern["address"].str.orEmpty()) to MsTone.Warning
    "firstTime" -> "${words.raw("componentsUi.signing.firstTimeTag")} · ${MultisigWords.short(concern["address"].str.orEmpty())}" to MsTone.Warning
    "reverts" -> words.raw("componentsUi.signing.simWillFail") to MsTone.Danger
    "unsimulated" -> words.raw("componentsUi.signing.simUnavailableWarning") to MsTone.Warning
    "blind" -> words.t("action.unsupported") to MsTone.Danger
    // r8-1 / r8-2 / r8-5: the core's reading of every call raises these
    // (the core's words, `review.reading.*`, as the web says them).
    "unlimitedApproval" -> words.t("review.reading.unlimitedLine", "spender" to MultisigWords.short(concern["spender"].str.orEmpty())) to MsTone.Danger
    "nestedBatch" -> words.t("review.reading.nestedLine") to MsTone.Danger
    "unreadableBatch" -> words.t("review.reading.unreadableLine") to MsTone.Danger
    "decimalsUnknown" -> words.t("review.reading.decimalsLine", "token" to MultisigWords.short(concern["token"].str.orEmpty())) to MsTone.Warning
    "toOwnToken" -> words.t("review.reading.ownTokenLine", "token" to MultisigWords.short(concern["token"].str.orEmpty())) to MsTone.Danger
    else -> null
}

// -- the core's reading of every call (r8-1, r8-2, r8-5) ----------------------------

/** One call of the reading: how deep it sits, its leg label, its title, its rows, how it reads. */
data class ReadingLine(val depth: Int, val label: String, val text: String, val tone: MsTone, val details: List<Pair<String, String>> = emptyList())

/** A token named by its facts: its symbol, else its short address; the network's coin when none. */
internal fun tokenName(facts: JsonElement?, words: MultisigWords, chainId: Int): String =
    facts?.get("symbol").str?.takeIf { it.isNotBlank() }
        ?: facts?.get("token").str?.takeIf { it.isNotBlank() }?.let(MultisigWords::short)
        ?: words.symbol(chainId)

/**
 * An amount as the core says it can honestly be said: unlimited, the exact
 * whole-token figure (`plain`, in the person's decimal mark), or — decimals
 * unknown — the base units and that the decimals are unknown (never a guess
 * at 18, r8-2).
 */
internal fun amountText(amount: JsonElement?, symbol: String, words: MultisigWords): String {
    if (amount["unlimited"].bool == true) return words.t("review.reading.unlimited")
    // r10 S4: grouped, as the title above it and the web say it ("1,000,000
    // USDC", never "1000000"), in the person's own separators — every digit
    // the core read kept (no rounding: as many places as it has).
    amount["plain"].str?.let { return "${groupedAmount(it)} $symbol".trim() }
    val raw = amount["raw"].str ?: "0"
    val said = words.t("review.reading.decimalsUnknown", "raw" to raw)
    // The raw base units are said whatever the catalog holds (a catalog
    // older than the key still never hides the figure, nor guesses it).
    return if (said.contains(raw)) said else "$raw · ${words.raw("componentsUi.signingApprove.decimalsUnverified")}"
}

/** A plain decimal (`1000000.25`) grouped in the person's style, every place kept. */
internal fun groupedAmount(plain: String): String {
    val places = plain.substringAfter('.', "").length
    return app.getvela.wallet.core.format.Formats.current.number(plain, 0, places)
}

/**
 * The review's `reading`: every call, every leg of every batch at every
 * level, each as the core read it — drawn, never decoded here (the Bybit
 * rule; the web's `reading-blocks.ts`). Empty when the review reads the
 * transaction as the multisig's own (its member change, rename or
 * cancellation say it).
 */
internal fun readingLines(review: JsonElement, chainId: Int, words: MultisigWords): List<ReadingLine> {
    val out = mutableListOf<ReadingLine>()
    val r = { key: String -> words.t("review.reading.$key") }
    fun walk(calls: List<JsonElement>, depth: Int) {
        for (call in calls) {
            // r10-10 (core §9.5): the leg's number is the core's (`leg`, "2.1");
            // `null` — the lone call, a lone batch included — draws NO number.
            // Never joined from `path` here ("第 笔调用").
            val label = call["leg"].str?.takeIf { it.isNotEmpty() }?.let { words.t("review.callNumber", "n" to it) }.orEmpty()
            val to = MsRules.eip55(call["to"].str.orEmpty())
            val intent = call["intent"]
            val rows = mutableListOf<Pair<String, String>>()
            val (title, tone) = when (intent.kind) {
                "send" -> {
                    val sym = tokenName(intent["token"], words, chainId)
                    rows += r("amount") to amountText(intent["amount"], sym, words)
                    intent["token"]["token"].str?.let { rows += r("token") to MsRules.eip55(it) }
                    rows += r("to") to MsRules.eip55(intent["to"].str.orEmpty())
                    r("send") to MsTone.Neutral
                }
                "sendFrom" -> {
                    val sym = tokenName(intent["token"], words, chainId)
                    rows += r("amount") to amountText(intent["amount"], sym, words)
                    intent["token"]["token"].str?.let { rows += r("token") to MsRules.eip55(it) }
                    rows += r("from") to MsRules.eip55(intent["from"].str.orEmpty())
                    rows += r("to") to MsRules.eip55(intent["to"].str.orEmpty())
                    r("sendFrom") to MsTone.Warning
                }
                "approve" -> {
                    val sym = tokenName(intent["token"], words, chainId)
                    val unlimited = intent["amount"]["unlimited"].bool == true
                    intent["token"]["token"].str?.let { rows += r("token") to "$sym · ${MsRules.eip55(it)}" }
                    rows += r("spender") to MsRules.eip55(intent["spender"].str.orEmpty())
                    rows += r("allowance") to if (intent["grant"].str == "daiPermit") r("allOfIt") else amountText(intent["amount"], sym, words)
                    val grant = intent["grant"].str ?: "approve"
                    r(grant) to if (unlimited || grant == "daiPermit") MsTone.Danger else MsTone.Warning
                }
                "approveAll" -> {
                    rows += r("collection") to MsRules.eip55(intent["collection"].str.orEmpty())
                    rows += r("operator") to MsRules.eip55(intent["operator"].str.orEmpty())
                    if (intent["granted"].bool == false) r("revokeAll") to MsTone.Neutral else r("approveAll") to MsTone.Danger
                }
                "batch" -> {
                    val legs = intent["calls"].arr
                    rows += r("runner") to MsRules.eip55(intent["runner"].str.orEmpty())
                    (if (legs.size == 1) r("batchOne") else words.t("review.reading.batchMany", "total" to legs.size.toString())) to MsTone.Danger
                }
                "unreadable" -> {
                    rows += r("to") to to
                    r("unreadable") to MsTone.Danger
                }
                "members" -> r("members") to MsTone.Warning
                "rename" -> words.t("review.reading.rename", "name" to intent["name"].str.orEmpty()) to MsTone.Neutral
                else -> {
                    rows += r("to") to to
                    intent["selector"].str?.let { rows += r("function") to it }
                    r("call") to MsTone.Warning
                }
            }
            val coin = call["value"]
            if (intent.kind != "send" && (coin["raw"].str ?: "0") != "0") rows += r("alsoSends") to amountText(coin, words.symbol(chainId), words)
            out += ReadingLine(depth, label, title, tone, rows)
            if (intent.kind == "batch") {
                out += ReadingLine(depth + 1, "", r("runnerNote"), MsTone.Warning)
                walk(intent["calls"].arr, depth + 1)
            }
        }
    }
    walk(review["reading"]["calls"].arr, 0)
    return out
}

/**
 * r10-8: the core reading's title for the sheet's header (the web's
 * `readingTitle`): a lone call by its intent ("Send 0.01 USDC",
 * "Approve · USDC"), more than one call — or a call that is itself a batch's
 * legs — as the batch ("Batch of 1 call"); `null` when the reading has no
 * calls (the multisig's own change, said by its action's title).
 */
internal fun readingTitle(review: JsonElement?, chainId: Int, words: MultisigWords): String? {
    val calls = review["reading"]["calls"].arr
    if (calls.isEmpty()) return null
    val r = { key: String -> words.t("review.reading.$key") }
    val first = calls.first()
    if (calls.size == 1 && first["path"].arr.isEmpty()) {
        val intent = first["intent"]
        return when (intent.kind) {
            "send" -> "${r("send")} ${amountText(intent["amount"], tokenName(intent["token"], words, chainId), words)}"
            "sendFrom" -> r("sendFrom")
            "approve" -> "${r(intent["grant"].str ?: "approve")} · ${tokenName(intent["token"], words, chainId)}"
            "approveAll" -> if (intent["granted"].bool == false) r("revokeAll") else r("approveAll")
            "batch" -> intent["calls"].arr.size.let { n -> if (n == 1) r("batchOne") else words.t("review.reading.batchMany", "total" to n.toString()) }
            "unreadable" -> r("unreadable")
            "members" -> r("members")
            "rename" -> words.t("review.reading.rename", "name" to intent["name"].str.orEmpty())
            else -> r("call")
        }
    }
    return if (calls.size == 1) r("batchOne") else words.t("review.reading.batchMany", "total" to calls.size.toString())
}

@Composable
private fun Reading(review: JsonElement, chainId: Int, words: MultisigWords) {
    val lines = readingLines(review, chainId, words)
    if (lines.isEmpty()) return
    val colors = VelaTheme.colors
    Column(Modifier.fillMaxWidth().testTag("multisig.review.reading"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
        for (line in lines) {
            Column(
                Modifier.fillMaxWidth().padding(start = VelaSpacing.lg * line.depth).testTag("multisig.review.reading.call"),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
            ) {
                if (line.details.isEmpty()) {
                    MsNote(line.text, tone = line.tone)
                } else {
                    MsText(
                        if (line.label.isEmpty()) line.text else "${line.label} · ${line.text}",
                        MsType.RowTitle,
                        color = if (line.tone == MsTone.Danger) colors.errorBase else colors.fgBase,
                    )
                    for ((k, v) in line.details) MsRow(k, v, mono = v.startsWith("0x"))
                }
            }
        }
    }
}

/** What the verdict's place holds, as the core's `simulation` says it. */
internal sealed interface MovesSaid {
    /**
     * Nothing: the core withholds what moves (`kind` `withheld` — refused for
     * a reason other than the state guard's, a delegatecall or a batch too
     * deep), or there is no line to say.
     */
    data object Empty : MovesSaid
    /** The balance rows. */
    data object Deltas : MovesSaid
    /** One line: the core's no-change key (quiet, `tone` null), or its notice. */
    data class Line(val key: String, val reason: String?, val tone: MsTone?) : MovesSaid
}

/** The core's `simulation` in the verdict's place — never "No asset changes" for one the core withheld. */
internal fun movesSaid(review: JsonElement): MovesSaid {
    val sim = review["simulation"]
    return when (sim["kind"].str) {
        "withheld" -> MovesSaid.Empty
        "deltas" ->
            if (sim["deltas"].arr.isNotEmpty()) MovesSaid.Deltas
            else MovesSaid.Line(sim["no_change_key"].str ?: "componentsUi.signing.simResultNoChange", null, null)
        else -> {
            val key = sim["notice_key"].str
            if (key.isNullOrEmpty()) MovesSaid.Empty
            else MovesSaid.Line(key, sim["revert_reason"].str, if (sim["notice_risk"].str == "danger") MsTone.Danger else MsTone.Warning)
        }
    }
}

@Composable
private fun Moves(review: JsonElement, chainId: Int, words: MultisigWords, tokens: Map<String, Pair<String, Int>>) {
    val deltas = review["simulation"]["deltas"].arr
    val colors = VelaTheme.colors
    when (val said = movesSaid(review)) {
        MovesSaid.Empty -> Unit
        // r11 S4: the heading is the review's own key (`movesTitleKey`, "Balance
        // changes" — the signing sheet's), never "SIMULATION RESULT".
        MovesSaid.Deltas -> MsSection(words.raw(movesTitleKey(review))) {
            for (delta in deltas) {
                val text = deltaText(delta, chainId, words, tokens)
                MsText(text, MsType.RowTitle, Modifier.testTag("multisig.review.delta"), color = if (delta["delta"].str?.startsWith("-") == true) colors.fgBase else colors.successBase)
            }
            if (review["simulationSource"].str == "probe") MsNote(words.t("review.movesProbe"))
        }
        is MovesSaid.Line -> {
            val text = if (said.reason != null) words.raw(said.key, "reason" to said.reason) else words.raw(said.key)
            if (said.tone == null) MsNote(text) else MsNote(text, tone = said.tone, tag = "multisig.review.sim")
        }
    }
}

internal fun deltaText(delta: JsonElement, chainId: Int, words: MultisigWords, tokens: Map<String, Pair<String, Int>>): String {
    val raw = delta["delta"].str ?: "0"
    val negative = raw.startsWith("-")
    val magnitude = raw.removePrefix("-")
    val token = delta["token"].str?.takeIf { it.isNotEmpty() }
    val (symbol, decimals) = if (token != null) tokens[token.lowercase()] ?: (MultisigWords.short(token) to 18) else words.symbol(chainId) to 18
    // r11 S4: signed the one way money is signed in Vela (the core's U+2212 / "+").
    return "${uniffi.vela_core_uniffi.signedAmountText(MsRules.formatUnits(magnitude, decimals), negative)} $symbol"
}

/** r11 S4: the heading over what moves — the review's key; the signing sheet's own before a core that names it. */
internal fun movesTitleKey(review: JsonElement): String =
    review["movesTitleKey"].str ?: review["moves_title_key"].str ?: "componentsUi.signing.balanceChangesTitle"

@Composable
private fun Recipients(review: JsonElement, words: MultisigWords) {
    val list = review["recipients"].arr
    if (list.isEmpty()) return
    val colors = VelaTheme.colors
    Column(Modifier.fillMaxWidth()) {
        list.forEachIndexed { index, r ->
            if (index > 0) MsDivider()
            val address = r["address"].str.orEmpty()
            val role = when (r["role"].str) {
                "contract" -> words.t("review.roleContract")
                "member" -> words.t("review.roleMember")
                else -> words.t("review.roleRecipient")
            }
            val label = r["call"].int?.let { "${words.t("review.callNumber", "n" to it.toString())} · $role" } ?: role
            val name = words.localName(address) ?: words.keptName(address) ?: r["known"]["label"].str
            val tags = buildList {
                if (r["firstTimeCue"].bool == true) add(words.raw("componentsUi.signing.firstTimeTag"))
                if (r["contractCue"].bool == true) add(words.raw("componentsUi.signing.contractTag"))
            }
            val whose = when {
                r["known"]["kind"].str == "own" -> words.t("review.knownOwn")
                r["known"]["kind"].str == "member" && r["role"].str != "member" -> words.t("review.knownMember")
                else -> null
            }
            Column(Modifier.fillMaxWidth().padding(vertical = VelaSpacing.md).testTag("multisig.review.recipient"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                MsText(label, MsType.Label, color = colors.fgSubtle)
                if (name != null && !name.equals(address, ignoreCase = true)) MsText(name, MsType.Field)
                MsText(MsRules.eip55(address), MsType.Mono)
                when {
                    tags.isNotEmpty() -> MsChip(tags.joinToString(" · "), tone = if (r["lookalike"].isNullish) MsTone.Warning else MsTone.Danger)
                    whose != null -> MsChip(whose)
                }
            }
        }
    }
}

@Composable
private fun MemberChanges(review: JsonElement, words: MultisigWords, network: String) {
    val changes = review["memberChanges"] ?: return
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
        for (change in changes["changes"].arr) {
            val text = changeText(words, change)
            MsText(change["call"].int?.let { "#$it · $text" } ?: text, MsType.RowTitle)
        }
        val before = changes["before"]
        val after = changes["after"]
        MsText(
            words.t(
                "change.rule",
                "before" to words.ruleText(before["threshold"].int ?: 0, before["owners"].int ?: 0),
                "after" to words.ruleText(after["threshold"].int ?: 0, after["owners"].int ?: 0),
            ),
            MsType.Body,
            Modifier.testTag("multisig.review.rule"),
        )
        MsNote(words.t("change.caution", "network" to network), tone = MsTone.Warning)
        if (changes["fits"].bool == false) MsNote(words.t("change.doesNotFit", "network" to network), tone = MsTone.Danger)
    }
}

internal fun changeText(words: MultisigWords, change: JsonElement): String = when (change["kind"].str) {
    "add" -> words.t("change.add", "member" to words.who(change["added"].str.orEmpty()))
    "remove" -> words.t("change.remove", "member" to words.who(change["removed"].str.orEmpty()))
    "replace" -> words.t("change.replace", "old" to words.who(change["removed"].str.orEmpty()), "new" to words.who(change["added"].str.orEmpty()))
    else -> words.t("change.threshold")
}

@Composable
private fun StateGuard(review: JsonElement, words: MultisigWords, network: String) {
    val state = review["state"]
    val verdict = state["verdict"]
    val source = if (state["source"].str == "trace") words.t("review.sourceTrace") else words.t("review.sourceProbe")
    val colors = VelaTheme.colors
    when (verdict.kind) {
        "undeclared" -> Column(Modifier.fillMaxWidth().testTag("multisig.review.state.undeclared"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            MsText(words.t("review.stateTitle"), MsType.RowTitle, color = colors.errorBase)
            MsNote(words.t("review.stateBody", "network" to network), tone = MsTone.Danger)
            for (line in stateWriteLines(words, verdict["writes"].arr)) MsText(line, MsType.Mono, color = colors.errorBase)
            MsNote(source)
        }
        "unchanged", "declared" -> Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            MsNote(
                words.t(if (verdict.kind == "declared") "review.stateDeclared" else "review.stateUnchanged", "network" to network),
                tone = MsTone.Positive,
                tag = "multisig.review.state.ok",
            )
            MsNote(source)
        }
        "unavailable" -> MsNote(words.t("review.stateUnread", "network" to network), tone = MsTone.Warning, tag = "multisig.review.state.unread")
        else -> Unit
    }
}

/** The guard's writes, one line each, said once. */
internal fun stateWriteLines(words: MultisigWords, writes: List<JsonElement>): List<String> {
    val out = mutableListOf<String>()
    for (w in writes) {
        val lines = when (w.kind) {
            "singleton" -> listOf(words.t("review.writeSingleton", "before" to w["before"].str.orEmpty(), "after" to w["after"].str.orEmpty()))
            "module" -> listOf(words.t(if (w["enabled"].bool == true) "review.writeModuleOn" else "review.writeModuleOff", "address" to w["address"].str.orEmpty()))
            "guard" -> listOf(words.t("review.writeGuard", "after" to w["after"].str.orEmpty()))
            "moduleGuard" -> listOf(words.t("review.writeModuleGuard", "after" to w["after"].str.orEmpty()))
            "handler" -> listOf(words.t("review.writeHandler", "after" to w["after"].str.orEmpty()))
            "owners" -> w["added"].arr.map { words.t("review.writeAdded", "address" to it.str.orEmpty()) } +
                w["removed"].arr.map { words.t("review.writeRemoved", "address" to it.str.orEmpty()) }
            "ownerEntry" -> listOf(words.t("review.writeOwnerEntry", "address" to w["address"].str.orEmpty()))
            "threshold" -> listOf(words.t("review.writeThreshold", "before" to w["before"].str.orEmpty(), "after" to w["after"].str.orEmpty()))
            "ownerCount" -> listOf(words.t("review.writeOwnerCount", "after" to w["after"].str.orEmpty()))
            "nonce" -> listOf(words.t("review.writeNonce"))
            else -> listOf(words.t("review.writeStorage", "slot" to w["slot"].str.orEmpty()))
        }
        lines.forEach { if (it !in out) out += it }
    }
    return out
}

@Composable
private fun Technical(review: JsonElement, words: MultisigWords) {
    var open by remember { mutableStateOf(false) }
    val colors = VelaTheme.colors
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().clickable(role = Role.Button) { open = !open }.padding(vertical = VelaSpacing.md).testTag("multisig.review.techToggle"),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        ) {
            MsText(words.t("review.techTitle"), MsType.Action, Modifier.weight(1f), color = colors.fgMuted)
            Icon(if (open) VelaIcons.ChevronDown else VelaIcons.ChevronRight, null, tint = colors.fgMuted, modifier = Modifier.size(VelaIconSize.sm))
        }
        if (open) {
            Column(Modifier.testTag("multisig.review.tech"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
                for (field in review["fields"].arr) MsRow(field["name"].str.orEmpty(), field["value"].str.orEmpty(), mono = true)
                MsText(words.t("review.hashesTitle"), MsType.Label, Modifier.padding(top = VelaSpacing.md), color = colors.fgSubtle)
                val hashes = review["hashes"]
                MsRow(words.t("external.domainHash"), hashes["domainHash"].str.orEmpty(), mono = true, copyable = true)
                MsRow(words.t("external.messageHash"), hashes["messageHash"].str.orEmpty(), mono = true, copyable = true)
                MsRow(words.t("external.safeTxHash"), hashes["safeTxHash"].str.orEmpty(), mono = true, copyable = true)
            }
        }
    }
}

/**
 * The review-first sheet (§3.6): a proposal the machine drew — a send from
 * the multisig, a rename, a member change, a site's calls — shown with its
 * one review BEFORE any passkey; the person proposes it (as one of this
 * device's members) or withdraws it, and nothing is signed until they do.
 */
@Composable
fun MultisigDraftReviewSheet(
    hub: MultisigHub,
    words: MultisigWords,
    tokenInfo: suspend (Int, String) -> Pair<String, Int>,
    siteNote: String? = null,
    onWithdraw: (() -> Unit)? = null,
    /** A site's transaction (M5): one its proposer's approval makes ready is proposed and executed in one press. */
    fromSite: Boolean = false,
) {
    val view by hub.proposalsView.collectAsStateWithLifecycle()
    val draft: MsDraft? = view.draft
    val state = view.action("propose")
    val working = state.kind == "working"
    var member by remember { mutableStateOf<String?>(null) }
    val withdraw = { onWithdraw?.invoke() ?: hub.withdrawDraft() }
    MsSheet(words.t("proposal.title"), words.raw("common.done"), onClose = { withdraw() }, dismissible = !working, tag = "multisig.draft") {
        siteNote?.let { MsNote(it, tag = "multisig.draft.site") }
        if (draft == null) {
            MsText(words.t("proposal.opening"), MsType.Body, color = VelaTheme.colors.fgMuted)
            return@MsSheet
        }
        val item = draft.item
        MsText(words.actionTitle(item.chainId, item.action, item.nonce), MsType.Title, Modifier.testTag("multisig.draft.title"))
        MsText("${words.network(item.chainId)} · ${words.t("action.numberValue", "nonce" to item.nonce)}", MsType.Sub, color = VelaTheme.colors.fgSubtle)
        item.refusal?.takeIf { it.isNotEmpty() }?.let { MsNote(words.t("proposal.refused", "reason" to it), tone = MsTone.Danger) }
        // §12: the core's sign door over the review's state and its sign rule
        // (r8-1): enabled only once it is read (`ready`) and the rule allows
        // it; `anyway` signs only through the "… anyway" friction — never a
        // plain accent press. Loading or failed: nothing is proposed.
        val door = remember(draft.reviewState, draft.review) { draftDoor(draft) }
        MsReviewDoor(door, words, "multisig.draft", onTryAgain = { hub.sendProposal(draftReviewAgain()) }) {
            draft.review?.takeUnless { it.isNullish }?.let { MultisigReviewBody(it, item.chainId, words, tokenInfo) }
        }
        words.actionFailure(state, item.chainId)?.let { MsNote(it, tone = MsTone.Danger, tag = "multisig.draft.failed") }
        val anyway = door.anyway
        if (draft.membersHere.size > 1) {
            MemberPicker(words, words.t("proposal.as"), draft.membersHere, member ?: draft.defaultMember ?: draft.membersHere.first(), tag = "multisig.draft.as") { member = it }
        }
        // A site's transaction ready on its proposer's approval: the machine's
        // one press (`draft.executeNow`, `propose_execute`) — the site gets its
        // transaction, not a wait for approvals nobody else has to give.
        val executeNow = fromSite && draft.executeNow && draft.membersHere.isNotEmpty()
        // Proposing signs (the member's approval travels with it): the accent.
        // Proposing with no member here signs nothing: no friction to ask for.
        val signs = draft.membersHere.isNotEmpty()
        MsSignButton(
            text = when {
                executeNow -> words.t(if (anyway) "review.approveExecuteAnyway" else "proposal.approveAndExecute")
                !signs -> words.t("send.proposeOnly")
                else -> words.t(if (anyway) "review.proposeAnyway" else "send.propose")
            },
            anyway = anyway && signs,
            dangerLines = draft.review?.let { r -> shownConcerns(r).mapNotNull { reviewConcern(words, it) }.filter { it.second == MsTone.Danger }.map { it.first } } ?: emptyList(),
            words = words,
            onConfirm = {
                val by = member ?: draft.defaultMember
                if (executeNow) hub.sendProposal(ev("propose_execute", "as_member" to by)) else hub.confirmDraft(by)
            },
            modifier = Modifier.fillMaxWidth().testTag("multisig.draft.propose"),
            enabled = door.enabled && !working,
            loading = working,
        )
        if (executeNow) MsNote(words.t("proposal.feeNote"))
        VelaSecondaryButton(
            text = words.t("proposal.dontPropose"),
            onClick = { withdraw() },
            modifier = Modifier.fillMaxWidth().testTag("multisig.draft.withdraw"),
            enabled = !working,
        )
    }
}

/** §3.5: the person's word on signing for a high-risk multisig. */
@Composable
fun MultisigRiskSheet(ask: MultisigRiskAsk, words: MultisigWords) {
    MsSheet(words.t("authority.signRiskTitle"), words.raw("common.done"), onClose = { ask.answer(false) }, tag = "multisig.risk") {
        MsNote(words.t("authority.signRiskBody", "network" to words.network(ask.chainId)), tone = MsTone.Danger)
        for (finding in ask.findings) MsText(words.finding(finding), MsType.Body)
        VelaDangerButton(
            text = words.t("authority.signRiskConfirm"),
            onClick = { ask.answer(true) },
            modifier = Modifier.fillMaxWidth().testTag("multisig.risk.sign"),
        )
        VelaSecondaryButton(
            text = words.t("authority.signRiskCancel"),
            onClick = { ask.answer(false) },
            modifier = Modifier.fillMaxWidth().testTag("multisig.risk.cancel"),
        )
    }
}

/** "As" — which of this device's members acts: a row of choices, the chosen ticked. */
@Composable
fun MemberPicker(words: MultisigWords, label: String, members: List<String>, selected: String?, tag: String, onPick: (String) -> Unit) {
    val colors = VelaTheme.colors
    MsSection(label, Modifier.testTag(tag)) {
        for (address in members) {
            val on = address.equals(selected, ignoreCase = true)
            Row(
                Modifier.fillMaxWidth().selectable(selected = on, role = Role.RadioButton) { onPick(address) }.padding(vertical = VelaSpacing.sm),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                Column(Modifier.weight(1f)) {
                    MsText(words.label(address), MsType.Field)
                    if (words.label(address) != MultisigWords.short(address)) MsText(MultisigWords.short(address), MsType.Mono, color = colors.fgSubtle)
                }
                if (on) Icon(VelaIcons.Check, null, tint = colors.fgBase, modifier = Modifier.size(VelaIconSize.lg))
            }
        }
    }
}

/**
 * The one button a signature is given with (r8-1). The core's sign rule
 * decides its friction: a plain request is the accent press; one the rule
 * says `anyway` (an unlimited allowance, a nested batch, a call Vela cannot
 * read) is danger-styled, worded "… anyway", and signs only after a SECOND,
 * explicit confirmation that says the review's danger lines again — so one
 * tap, on a sheet that scrolled or filled in under the finger, never signs
 * a danger request.
 */
@Composable
fun MsSignButton(
    text: String,
    anyway: Boolean,
    dangerLines: List<String>,
    words: MultisigWords,
    onConfirm: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    loading: Boolean = false,
) {
    var confirming by remember { mutableStateOf(false) }
    if (!anyway) {
        VelaValueButton(text = text, onClick = onConfirm, modifier = modifier, enabled = enabled, loading = loading)
        return
    }
    VelaDangerButton(text = text, onClick = { confirming = true }, modifier = modifier, enabled = enabled, loading = loading)
    if (confirming) {
        val colors = VelaTheme.colors
        AlertDialog(
            onDismissRequest = { confirming = false },
            title = { Text(text) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
                    for (line in anywayLines(dangerLines, words)) Text(line, color = colors.errorBase)
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirming = false
                        onConfirm()
                    },
                    modifier = Modifier.testTag("multisig.sign.anywayConfirm"),
                ) { Text(text, color = colors.errorBase) }
            },
            dismissButton = {
                TextButton(onClick = { confirming = false }, modifier = Modifier.testTag("multisig.sign.anywayCancel")) {
                    Text(words.t("proposal.rejectCancel"))
                }
            },
            modifier = Modifier.testTag("multisig.sign.anyway"),
        )
    }
}

/** What the "… anyway" confirmation says: the review's danger lines, else the core's general warning. */
internal fun anywayLines(dangerLines: List<String>, words: MultisigWords): List<String> =
    dangerLines.ifEmpty { listOf(words.raw("componentsUi.signing.simUnavailableWarning")) }
