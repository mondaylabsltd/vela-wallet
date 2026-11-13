package app.getvela.wallet.feature.multisig

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextDecoration
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaValueButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.feature.multisig.core.MsPhase
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.isNullish
import app.getvela.wallet.feature.multisig.core.jo
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.obj
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement

/**
 * A multisig's settings (M1, M3) — the web's `MultisigSettings`, iOS's
 * `MultisigSettingsSheet`: its members per network as each chain holds them
 * (the members machine), the invite, its record (shared, or "share it again"
 * on one passkey), its shared name (a rename is one of its own
 * transactions, said honestly while its record is on its way — r7-10), its
 * setup on Ethereum, and removing it from this device.
 */
@Composable
fun MultisigSettingsSheet(
    hub: MultisigHub,
    words: MultisigWords,
    address: String,
    onClose: () -> Unit,
    onOpen: (MultisigSheet) -> Unit,
    onRemoved: () -> Unit,
) {
    val view by hub.accountsView.collectAsStateWithLifecycle()
    val membersState by hub.membersView.collectAsStateWithLifecycle()
    // Read through the lenient accessors (`MsJson.kt`): a JsonObject's own `get` would hand back JsonNull.
    val members: kotlinx.serialization.json.JsonElement = membersState
    val scope = rememberCoroutineScope()
    val row = view.row(address)
    var removing by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }
    var newName by remember { mutableStateOf("") }
    val colors = VelaTheme.colors
    LaunchedEffect(address) {
        hub.showMembers(address)
        hub.send(ev("plan_rename", "address" to address))
        hub.send(ev("check_backup", "address" to address))
    }
    MsSheet(row?.let { words.name(it.account, it.name) } ?: words.t("switcher.section"), words.raw("common.done"), onClose = onClose, tag = "multisig.settings") {
        row ?: return@MsSheet
        // Identity.
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
            IdenticonAvatar(seed = row.account.address, size = VelaIconSize.xl * 1.5f, tappable = false)
            Column {
                // r9-14: the members machine's `rule_chip` (each network's members now).
                MsText(
                    members.takeIf { it["address"].str.equals(address, ignoreCase = true) }?.let { words.ruleTitleOf(it["rule_chip"]) }
                        // r9-14 (core §9.2): before the members machine reads, the
                        // accounts machine's rule (the chains' last word), never the record's.
                        ?: words.ruleTitleOf(row.rule["chip"])
                        ?: words.ruleTitle(row.account.threshold, row.account.owners.size),
                    MsType.Field,
                )
                MsText(row.account.chainIds.joinToString(", ", transform = words::network), MsType.Sub, color = colors.fgSubtle)
            }
        }
        MsRow(words.t("create.address"), row.account.address, mono = true, copyable = true, tag = "multisig.settings.address")

        // Members per network (the members machine's word).
        val originalMembers = members["original"]["members"].arr
        MsSection(words.t("settings.originalMembers")) {
            val list = originalMembers.ifEmpty { row.account.owners.map { jo("address" to it) } }
            for (m in list) MemberLine(words, m)
            MsNote(words.t("settings.originalNote"))
        }
        for (network in members["networks"].arr) {
            val chainId = network["chain_id"].int ?: 0
            val state = network["state"]
            when (state.kind) {
                "deployed" -> MsSection(words.t("settings.currentOn", "network" to words.network(chainId)), Modifier.testTag("multisig.settings.network.$chainId")) {
                    if (state["same"].bool == true) {
                        MsNote(words.t("settings.sameAsOriginal"))
                    } else {
                        for (m in state["members"].arr) MemberLine(words, m, added = m["added"].bool == true)
                        for (m in state["removed"].arr) MemberLine(words, m, removed = true)
                        if (state["threshold_was"].int != state["threshold_is"].int) {
                            MsNote(words.t("settings.thresholdChanged", "was" to state["threshold_was"].str.orEmpty(), "is" to state["threshold_is"].str.orEmpty()))
                        }
                    }
                }
                "undeployed" -> MsNote(words.t("settings.undeployedOn", "network" to words.network(chainId)), tag = "multisig.settings.undeployed")
                // core-ms-2: a chain that did not answer — nothing claimed of its members.
                "unreadable" -> MsNote(words.t("settings.unreadableOn", "network" to words.network(chainId)), tone = MsTone.Warning, tag = "multisig.settings.unreadable")
                else -> Unit
            }
        }
        MsNavRow(words.t("members.title"), icon = VelaIcons.UsersRound, tag = "multisig.settings.members") { onOpen(MultisigSheet.Members(address)) }
        if (row.account.owners.size > 1) {
            MsNavRow(words.t("panel.invite"), icon = VelaIcons.QrCode, tag = "multisig.settings.invite") { onOpen(MultisigSheet.Invite(address)) }
        }
        MultisigNotifySection(words)

        // The record.
        if (!row.shared) {
            MsSection(words.t("settings.shareAgain")) {
                MsNote(words.t(if (row.recordWaiting) "panel.notShared" else "settings.shareAgainNote"), tone = MsTone.Warning)
                view.lookup(view.shareAgainFailure, address)?.let { MsNote(words.failureText(it), tone = MsTone.Danger) }
                val owner = row.localMembers.firstOrNull()
                if (owner != null && !row.recordWaiting) {
                    VelaPrimaryButton(
                        words.t("settings.shareAgain"),
                        { hub.send(ev("share_again", "address" to address, "owner" to owner)) },
                        Modifier.fillMaxWidth().testTag("multisig.settings.shareAgain"),
                        loading = view.lookup(view.shareAgain, address) == MsPhase.Working,
                    )
                }
            }
        }

        // The shared name (r7-10: the record's state said honestly).
        val plan = view.lookup(view.rename, address)
        MsSection(words.t("settings.renameTitle")) {
            when {
                plan == null -> CircularProgressIndicator(color = colors.fgMuted, strokeWidth = VelaSpacing.xs)
                plan.blocked == "unrecorded" -> MsNote(words.t("settings.renameNotRecorded"), tag = "multisig.settings.rename.blocked")
                plan.blocked == "pending" -> MsNote(words.t("settings.renamePending", "network" to words.network(plan.landsOn ?: plan.chainId ?: 100)), tag = "multisig.settings.rename.pending")
                plan.blocked == "unreachable" || plan.blocked == "elsewhere" -> MsNote(words.t("settings.renameUnavailable"), tag = "multisig.settings.rename.blocked")
                plan.chainId != null && plan.deployed == false -> MsNote(words.t("settings.renameNotDeployed", "network" to words.network(plan.chainId)))
                plan.chainId != null -> {
                    MsNote(words.t("settings.renameNote", "network" to words.network(plan.chainId)))
                    if (renaming) {
                        MsField(words.t("create.name"), newName, { newName = it }, words.name(row.account), tag = "multisig.settings.rename.field")
                        VelaPrimaryButton(
                            words.t("settings.renameAction"),
                            {
                                val name = newName.trim()
                                renaming = false
                                scope.launch { hub.propose(plan.chainId, mapOf("kind" to "rename", "name" to name), address = address) }
                            },
                            Modifier.fillMaxWidth().testTag("multisig.settings.rename.propose"),
                            enabled = MsRules.validName(newName.trim()),
                        )
                    } else {
                        MsTextButton(words.t("settings.renameAction"), tag = "multisig.settings.rename") {
                            renaming = true
                            newName = words.name(row.account)
                        }
                    }
                }
            }
        }

        // Setup on Ethereum.
        val backup = view.lookup(view.backup, address)?.let { it.str ?: it.kind }
        if (backup != null && backup != "unavailable") {
            MsSection(words.t("settings.backupTitle")) {
                val line = when (backup) {
                    "backedUp" -> words.t("settings.backedUp")
                    "notBackedUp", "failed" -> words.t("settings.notBackedUp")
                    "sent" -> words.t("proposal.executing")
                    "checking" -> words.t("panel.checking")
                    else -> words.t("settings.couldNotCheck")
                }
                MsNote(line, tone = if (backup == "backedUp") MsTone.Positive else MsTone.Neutral, tag = "multisig.settings.backup")
                MsNote(words.t("settings.backupNote"))
                val from = row.localMembers.firstOrNull()
                if ((backup == "notBackedUp" || backup == "failed") && from != null) {
                    VelaPrimaryButton(words.t("settings.backupTitle"), { hub.send(ev("save_to_ethereum", "address" to address, "from" to from)) }, Modifier.fillMaxWidth())
                }
                if (backup == "couldNotCheck") MsTextButton(words.t("review.state.tryAgain")) { hub.send(ev("check_backup", "address" to address)) }
            }
        }

        MsTextButton(words.t("settings.remove"), Modifier.padding(top = VelaSpacing.lg), tag = "multisig.settings.remove", danger = true) { removing = true }
        if (removing) {
            AlertDialog(
                onDismissRequest = { removing = false },
                title = { Text(words.t("settings.remove")) },
                text = { Text(words.t("switcher.removeBody")) },
                confirmButton = {
                    TextButton(onClick = {
                        removing = false
                        hub.send(ev("remove", "address" to address))
                        onRemoved()
                    }, modifier = Modifier.testTag("multisig.settings.removeConfirm")) { Text(words.t("settings.remove"), color = colors.errorBase) }
                },
                dismissButton = { TextButton(onClick = { removing = false }) { Text(words.raw("common.cancel")) } },
            )
        }
    }
}

@Composable
private fun MemberLine(words: MultisigWords, member: JsonElement, added: Boolean = false, removed: Boolean = false) {
    val colors = VelaTheme.colors
    val address = member["address"].str.orEmpty()
    val name = member["name"].str ?: words.localName(address) ?: words.keptName(address)
    val tags = buildList {
        if (member["on_device"].bool == true || words.localName(address) != null) add(words.t("panel.thisDevice"))
        if (added) add(words.t("settings.added"))
        if (removed) add(words.t("settings.removed"))
    }
    Row(
        Modifier.fillMaxWidth().padding(vertical = VelaSpacing.xs).semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        IdenticonAvatar(seed = address, size = VelaIconSize.xl, tappable = false)
        Column {
            Text(
                name ?: MultisigWords.short(address),
                color = if (removed) colors.fgSubtle else colors.fgBase,
                fontFamily = VelaFontFamily, fontWeight = VelaFontWeight.medium, fontSize = VelaTextSize.lg,
                textDecoration = if (removed) TextDecoration.LineThrough else null,
                // A short form is one line, never broken at its ellipsis.
                softWrap = name != null,
            )
            MsText((listOf(MultisigWords.short(address)) + tags).joinToString(" · "), MsType.Sub, color = colors.fgSubtle)
        }
    }
}

/**
 * Changing who the members are and how many must approve (M3) — the web's
 * `MemberChanges`, on the members machine: Add member, Replace and Remove
 * on each member, the approvals stepper. Each action is ONE proposal per
 * network the multisig is deployed on, built by the core against that
 * network's members; a refusal is said for that network before anything is
 * signed. The rule each leaves ("Approvals: 2 of 3 → 2 of 4") is shown.
 */
@Composable
fun MultisigMembersSheet(hub: MultisigHub, words: MultisigWords, address: String, onClose: () -> Unit) {
    val viewState by hub.membersView.collectAsStateWithLifecycle()
    val view: kotlinx.serialization.json.JsonElement = viewState
    val haptic = rememberVelaHaptic()
    var entry by remember { mutableStateOf("") }
    val proposing = remember { androidx.compose.runtime.mutableStateMapOf<Int, String>() }
    val colors = VelaTheme.colors
    LaunchedEffect(address) { hub.showMembers(address) }
    val close = {
        hub.sendMembers(ev("compose", "compose" to null))
        onClose()
    }
    MsSheet(words.t("members.title"), words.raw("common.done"), onClose = close, tag = "multisig.members") {
        if (view["none_deployed"].bool == true) MsNote(words.t("members.noneDeployed"), tone = MsTone.Warning, tag = "multisig.members.noneDeployed")
        view["recovery"]?.let { recovery ->
            MsNote(
                if (recovery.kind == "every_member") words.t("members.recoveryAll")
                else words.t("members.recovery", "m" to recovery["m"].str.orEmpty(), "n" to recovery["n"].str.orEmpty()),
            )
        }
        for ((chainId, line) in proposing.toSortedMap()) MsNote(line, tag = "multisig.members.proposing.$chainId")

        val groups = view["groups"].arr
        if (groups.isNotEmpty()) {
            MsSection(words.t("members.waitingTitle")) {
                for (group in groups) {
                    val parts = group["parts"].arr.joinToString(" · ") { "${words.network(it["chain_id"].int ?: 0)} ${it["approved"].int ?: 0}/${it["threshold"].int ?: 0}" }
                    Column(Modifier.semantics(mergeDescendants = true) {}) {
                        MsText(words.actionTitle(0, group["action"], ""), MsType.Field)
                        MsText(parts, MsType.Sub, color = colors.fgSubtle)
                    }
                }
            }
        }

        val composing = view["compose"]
        if (composing.isNullish) {
            MsSection(words.t("panel.members")) {
                for (r in view["rows"].arr) {
                    val a = r["address"].str.orEmpty()
                    val name = r["name"].str ?: words.label(a)
                    var menu by remember(a) { mutableStateOf(false) }
                    Column(Modifier.fillMaxWidth()) {
                        MsNavRow(name, subtitle = MultisigWords.short(a), tag = "multisig.members.menu") { menu = !menu }
                        if (menu) {
                            Row(Modifier.padding(start = VelaSpacing.lg), horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xl)) {
                                MsTextButton(words.t("members.replaceLabel", "name" to name), tag = "multisig.members.replace") {
                                    hub.sendMembers(ev("compose", "compose" to mapOf("kind" to "replace", "owner" to a)))
                                }
                                MsTextButton(words.t("members.removeLabel", "name" to name), tag = "multisig.members.remove") {
                                    hub.sendMembers(ev("compose", "compose" to mapOf("kind" to "remove", "owner" to a)))
                                }
                            }
                        }
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xl)) {
                    MsTextButton(words.t("members.add"), tag = "multisig.members.add") { hub.sendMembers(ev("compose", "compose" to mapOf("kind" to "add"))) }
                    MsTextButton(words.t("members.thresholdTitle"), tag = "multisig.members.threshold") { hub.sendMembers(ev("compose", "compose" to mapOf("kind" to "threshold"))) }
                }
                MsNote(words.t("members.lostPasskey"))
            }
        } else {
            val kind = composing.kind
            when (kind) {
                "replace" -> MsNote(words.t("members.replaceIntro", "name" to words.label(composing["owner"].str.orEmpty())))
                "remove" -> MsNote(words.t("members.removeIntro", "name" to words.label(composing["owner"].str.orEmpty())))
            }
            if (kind == "add" || kind == "replace") {
                val entryView = view["entry"]
                MsField(
                    words.t("members.entryLabel"), entry,
                    { entry = it; hub.sendMembers(ev("entry", "text" to it)) },
                    words.t("members.entryPlaceholder"),
                    error = entryNote(words, entryView["note"]),
                    tag = "multisig.members.entry",
                )
                // Read once the typing stops (a name is looked up, an address checked) — the web reads it on enter.
                LaunchedEffect(entry) {
                    if (entry.isBlank()) return@LaunchedEffect
                    kotlinx.coroutines.delay(600)
                    hub.sendMembers(ev("read_entry"))
                }
                if (entryView["resolving"].bool == true) MsNote(words.t("members.resolving", "name" to entry))
                for (s in entryView["suggestions"].arr.take(5)) {
                    val a = s["address"].str.orEmpty()
                    MsNavRow(s["name"].str ?: MultisigWords.short(a), subtitle = MultisigWords.short(a), tag = "multisig.members.suggestion") {
                        entry = a
                        hub.sendMembers(ev("pick", "address" to a))
                    }
                }
                if (!entryView["entry"].isNullish) {
                    MsField(
                        words.t("members.memberName"), entryView["name"].str.orEmpty(),
                        { hub.sendMembers(ev("entry_name", "name" to it)) },
                        words.t("members.memberNamePlaceholder"), tag = "multisig.members.name",
                    )
                    MsNote(words.t("members.nameNote"))
                    entryView["choice"]?.let { choice ->
                        KindChoice(words, app.getvela.wallet.feature.multisig.core.MsKindChoice.of(choice)) { kind -> hub.sendMembers(ev("entry_kind", "kind" to kind)) }
                    }
                }
            }
            view["stepper"]?.let { stepper ->
                Stepper(
                    text = "${if (kind == "threshold") words.t("members.thresholdTitle") else words.t("members.thresholdAfter")}: " +
                        words.t("members.ruleValue", "m" to stepper["value"].str.orEmpty(), "n" to stepper["n"].str.orEmpty()),
                    canFewer = stepper["can_fewer"].bool == true, canMore = stepper["can_more"].bool == true,
                    fewer = words.t("create.fewer"), more = words.t("create.more"), tag = "multisig.members.rule",
                    onStep = { by -> hub.sendMembers(ev("step", "by" to by)) },
                )
            }
            val networks = view["plan"]["networks"].arr
            if (networks.isNotEmpty()) {
                MsSection(words.t("members.perNetwork")) {
                    for (network in networks) {
                        val chainId = network["chain_id"].int ?: 0
                        val verdict = network["verdict"]
                        if (verdict.kind == "refused" || network["proposal"].isNullish) {
                            MsNote(words.t("members.failedOn", "network" to words.network(chainId), "reason" to memberRefusal(words, verdict)), tone = MsTone.Warning)
                        } else {
                            val before = verdict["before"]
                            val after = verdict["after"]
                            MsNote(
                                words.t(
                                    "members.onNetwork", "network" to words.network(chainId),
                                    "before" to words.t("members.ruleValue", "m" to (before["threshold"].int ?: 0).toString(), "n" to before["owners"].arr.size.toString()),
                                    "after" to words.t("members.ruleValue", "m" to (after["threshold"].int ?: 0).toString(), "n" to after["owners"].arr.size.toString()),
                                ),
                                tag = "multisig.members.plan.$chainId",
                            )
                        }
                    }
                }
            }
            // Proposing a member change signs it (its approval travels with it): the accent.
            VelaValueButton(
                words.raw("home.multisig.send.propose"),
                {
                    haptic(VelaHaptic.Press)
                    val toPropose = networks.filter { !it["proposal"].isNullish }
                    hub.sendMembers(ev("proposing"))
                    proposing.clear()
                    // On the hub's scope, as Reject's: each network's review
                    // takes this sheet's place while it is up, and a scope
                    // tied to this sheet died with it — the first network
                    // was proposed, the rest never drawn, and the review
                    // sheet stayed on "Opening…".
                    hub.scope.launch {
                        for (network in toPropose) {
                            val chainId = network["chain_id"].int ?: 0
                            val proposal = network["proposal"].obj
                            proposing[chainId] = words.t("members.proposing", "network" to words.network(chainId))
                            val settled = hub.propose(chainId, proposal.mapValues { it.value }, address = address)
                            proposing[chainId] = if (settled.kind == "done") {
                                words.t("members.proposed", "network" to words.network(chainId))
                            } else {
                                words.t("members.failedOn", "network" to words.network(chainId), "reason" to (words.actionFailure(settled, chainId) ?: words.t("proposal.reason.unknown")))
                            }
                        }
                        hub.sendMembers(ev("compose", "compose" to null))
                    }
                },
                Modifier.fillMaxWidth().testTag("multisig.members.propose"),
                enabled = view["can_propose"].bool == true,
            )
            VelaSecondaryButton(words.raw("common.cancel"), { hub.sendMembers(ev("compose", "compose" to null)) }, Modifier.fillMaxWidth())
        }
    }
}

private fun entryNote(words: MultisigWords, note: JsonElement?): String? = when (note.kind) {
    "invalid_address" -> words.t("members.refuse.notAnAddress")
    "invalid_checksum" -> words.t("send.invalidChecksum")
    "no_one" -> words.t("members.refuse.noOne")
    "name_not_found" -> words.t("members.nameNotFound", "name" to note["name"].str.orEmpty())
    "name_unavailable" -> words.t("members.nameUnavailable", "name" to note["name"].str.orEmpty())
    else -> null
}

internal fun memberRefusal(words: MultisigWords, verdict: JsonElement?): String {
    val reason = verdict["refusal"] ?: verdict["reason"] ?: verdict
    return when (reason.kind) {
        "alreadyMember" -> words.t("members.refuse.alreadyMember")
        "notMember" -> words.t("members.refuse.notMember")
        "lastMember" -> words.t("members.refuse.lastMember")
        "theMultisig" -> words.t("members.refuse.theMultisig")
        "thresholdZero" -> words.t("members.refuse.thresholdZero")
        "unchanged" -> words.t("members.refuse.unchanged")
        "changeWaiting" -> words.t("members.refuse.changeWaiting")
        "thresholdTooHigh" -> words.t("members.refuse.thresholdTooHigh", "owners" to reason["owners"].str.orEmpty(), "threshold" to reason["threshold"].str.orEmpty())
        "belowThreshold" -> words.t("members.refuse.belowThreshold", "owners" to reason["owners"].str.orEmpty(), "threshold" to reason["threshold"].str.orEmpty())
        "tooMany" -> words.t("members.refuse.tooMany", "max" to reason["max"].str.orEmpty())
        else -> words.t("members.refuse.noOne")
    }
}

/** What the notifications section says (r10): on or off, and whether this phone holds the background checks back. */
internal data class NotifyStand(val on: Boolean, val limited: Boolean, val oem: Boolean)

/** The lines it draws, in order: the state, then (only while on) why a notification may come late. */
internal fun notifyLines(words: MultisigWords, stand: NotifyStand): List<Pair<String, MsTone>> = buildList {
    if (!stand.on) {
        add(words.t("notify.off") to MsTone.Warning)
        return@buildList
    }
    add(words.t("notify.on") to MsTone.Neutral)
    if (stand.limited) {
        add(words.t("notify.limited") to MsTone.Warning)
    } else if (stand.oem) {
        add(words.t("notify.oem") to MsTone.Neutral)
    }
}

/**
 * r10 (real device): the multisig's notifications, where the person looks
 * for them — on or off, with the way to turn them on; and, said here and
 * never as a banner, that this phone holds Vela's background checks back
 * (battery optimisation, a background restriction, or — named, since no API
 * reports it — a maker's own battery saver), with the way to its settings.
 * Read again whenever the sheet is back in front.
 */
@Composable
internal fun MultisigNotifySection(words: MultisigWords) {
    val context = androidx.compose.ui.platform.LocalContext.current
    var stand by remember { mutableStateOf(readNotifyStand(context)) }
    androidx.lifecycle.compose.LifecycleResumeEffect(Unit) {
        stand = readNotifyStand(context)
        onPauseOrDispose { }
    }
    MsSection(words.t("notify.title"), Modifier.testTag("multisig.settings.notify")) {
        for ((text, tone) in notifyLines(words, stand)) MsNote(text, tone = tone)
        if (!stand.on || stand.limited || stand.oem) {
            MsTextButton(words.t(if (stand.on) "notify.openSettings" else "notify.turnOn"), tag = "multisig.settings.notify.open") {
                val intent = if (!stand.on && android.os.Build.VERSION.SDK_INT >= 26) {
                    android.content.Intent(android.provider.Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                        .putExtra(android.provider.Settings.EXTRA_APP_PACKAGE, context.packageName)
                        .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
                } else {
                    MultisigNotifier.batterySettingsIntent(context)
                }
                runCatching { context.startActivity(intent) }
                    .onFailure { runCatching { context.startActivity(MultisigNotifier.batterySettingsIntent(context)) } }
            }
        }
    }
}

private fun readNotifyStand(context: android.content.Context) = NotifyStand(
    on = MultisigNotifier.allowed(context),
    limited = MultisigNotifier.backgroundLimited(context),
    oem = MultisigNotifier.oemHint(),
)
