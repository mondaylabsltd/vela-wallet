package app.getvela.wallet.feature.multisig

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
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
        if (review["sign"]["allowed"].bool == false) MsNote(words.t("review.blocked"), tone = MsTone.Danger, tag = "multisig.review.blocked")
        for (concern in shownConcerns(review)) {
            reviewConcern(words, concern)?.let { (text, tone) -> MsNote(text, tone = tone, tag = "multisig.review.concern.${concern.kind}") }
        }
        Moves(review, chainId, words, tokens)
        Recipients(review, words)
        MemberChanges(review, words, network)
        StateGuard(review, words, network)
        Technical(review, words)
    }
}

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
    val recipients = review["recipients"].arr
    val sim = review["simulation"]
    val simNotice = sim["kind"].str != "deltas" && !sim["notice_key"].str.isNullOrEmpty()
    val said = mutableSetOf<String>()
    return review["concerns"].arr.filter { concern ->
        val address = concern["address"].str
        val drawnElsewhere = when (concern.kind) {
            "firstTime" -> recipients.any { it["address"].str.equals(address, ignoreCase = true) && it["firstTimeCue"].bool == true }
            "unsimulated", "reverts" -> simNotice
            else -> false
        }
        !drawnElsewhere && said.add(concern.toString())
    }
}

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
    else -> null
}

@Composable
private fun Moves(review: JsonElement, chainId: Int, words: MultisigWords, tokens: Map<String, Pair<String, Int>>) {
    val sim = review["simulation"]
    val deltas = sim["deltas"].arr
    val colors = VelaTheme.colors
    when {
        sim["kind"].str == "deltas" && deltas.isNotEmpty() -> MsSection(words.raw("componentsUi.signing.simResultLabel")) {
            for (delta in deltas) {
                val text = deltaText(delta, chainId, words, tokens)
                MsText(text, MsType.RowTitle, Modifier.testTag("multisig.review.delta"), color = if (text.startsWith("-")) colors.fgBase else colors.successBase)
            }
            if (review["simulationSource"].str == "probe") MsNote(words.t("review.movesProbe"))
        }
        sim["kind"].str == "deltas" -> MsNote(words.raw(sim["no_change_key"].str ?: "componentsUi.signing.simResultNoChange"))
        !sim["notice_key"].str.isNullOrEmpty() -> {
            val key = sim["notice_key"].str!!
            val reason = sim["revert_reason"].str
            MsNote(
                if (reason != null) words.raw(key, "reason" to reason) else words.raw(key),
                tone = if (sim["notice_risk"].str == "danger") MsTone.Danger else MsTone.Warning,
                tag = "multisig.review.sim",
            )
        }
    }
}

internal fun deltaText(delta: JsonElement, chainId: Int, words: MultisigWords, tokens: Map<String, Pair<String, Int>>): String {
    val raw = delta["delta"].str ?: "0"
    val negative = raw.startsWith("-")
    val magnitude = raw.removePrefix("-")
    val token = delta["token"].str?.takeIf { it.isNotEmpty() }
    val (symbol, decimals) = if (token != null) tokens[token.lowercase()] ?: (MultisigWords.short(token) to 18) else words.symbol(chainId) to 18
    return "${if (negative) "-" else "+"}${MsRules.formatUnits(magnitude, decimals)} $symbol"
}

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
        draft.review?.takeUnless { it.isNullish }?.let { MultisigReviewBody(it, item.chainId, words, tokenInfo) }
        words.actionFailure(state, item.chainId)?.let { MsNote(it, tone = MsTone.Danger, tag = "multisig.draft.failed") }
        // The core's sign rule (r8-1): `allowed` false signs nothing; `anyway`
        // signs only through the "… anyway" friction — never a plain accent press.
        val reviewed = draft.review != null && !draft.review.isNullish
        val allowed = reviewed && (draft.review["sign"]["allowed"].bool ?: true)
        val anyway = draft.review["sign"]["anyway"].bool ?: false
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
            enabled = allowed && !working,
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
                Modifier.fillMaxWidth().clickable(role = Role.RadioButton) { onPick(address) }.padding(vertical = VelaSpacing.sm),
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
