package app.getvela.wallet.feature.multisig

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.core.designsystem.components.VelaDangerButton
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaValueButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.feature.flows.components.QrCard
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsCreateView
import app.getvela.wallet.feature.multisig.core.MsKindChoice
import app.getvela.wallet.feature.multisig.core.MsLinkStand
import app.getvela.wallet.feature.multisig.core.MsPhase
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull

/** A device account as the create sheet offers it. */
data class MsLocal(val address: String, val name: String)

private enum class CreateStep { Members, Threshold, Review, Created }

/**
 * Creating a multisig (M1) — the web's `CreateMultisigSheet`, iOS's
 * `MultisigCreateSheet`, on the accounts machine: its name and members (the
 * order is part of the address), how many must approve, a review that shows
 * the address before anything is signed, then ONE passkey — the creator's
 * proof, with which the service records it. Nothing is deployed and no fee is
 * paid until its first transfer out.
 *
 * The machine does it: `new_draft`, `draft` (the address the review shows),
 * `check_networks`, `read_kinds`, `create`. The form is kept by the machine
 * (`keep_draft`) until created or discarded, and the sheet opens on it; the
 * networks of the other space are said, with the way there (r7-9).
 */
@Composable
fun MultisigCreateSheet(
    hub: MultisigHub,
    words: MultisigWords,
    accounts: List<MsLocal>,
    contacts: List<MsLocal>,
    /** The space in front's networks, in the wallet's order. */
    spaceNetworks: List<Int>,
    /** The other space's name ("Testnets" / "Mainnets"), for the hint. */
    otherSpaceName: String,
    onClose: () -> Unit,
    onCreated: (String) -> Unit,
    onInvite: (String) -> Unit,
) {
    val view by hub.accountsView.collectAsStateWithLifecycle()
    val sheet: MsCreateView = view.create
    val haptic = rememberVelaHaptic()
    val scope = rememberCoroutineScope()
    var step by remember { mutableStateOf(CreateStep.Members) }
    val owners = remember { mutableStateListOf<String>() }
    val names = remember { mutableStateMapOf<String, String>() }
    var name by remember { mutableStateOf("") }
    var nameAsked by remember { mutableStateOf(false) }
    var typed by remember { mutableStateOf("") }
    var typedName by remember { mutableStateOf("") }
    var typedError by remember { mutableStateOf<String?>(null) }
    var threshold by remember { mutableIntStateOf(1) }
    var thresholdFor by remember { mutableIntStateOf(0) }
    var chainId by remember { mutableStateOf<Int?>(null) }
    var chose by remember { mutableStateOf(false) }
    var creator by remember { mutableStateOf<String?>(null) }
    var resumed by remember { mutableStateOf(false) }
    var created by remember { mutableStateOf<String?>(null) }
    var ensAsked by remember { mutableStateOf<String?>(null) }
    var started by remember { mutableStateOf(false) }

    // The networks offered: the space's, minus those found without the Safe
    // contracts, the likely ones first (the core's order).
    val hints = remember(view.accounts) {
        mapOf("used" to (MsRules.lastUsedNetworks(view.accounts.map { mapOf("chainIds" to it.account.chainIds, "addedMs" to (it.account.raw["addedMs"].long ?: 0)) }) ?: JsonNull))
    }
    val networks = run {
        val offered = spaceNetworks.filter { sheet.contracts(it) != "missing" }
        MsRules.likelyNetworks(offered, hints).ifEmpty { offered }
    }

    LaunchedEffect(Unit) {
        if (started) return@LaunchedEffect
        started = true
        hub.send(ev("new_draft"))
        hub.send(ev("check_networks", "chain_ids" to spaceNetworks))
        val kept = hub.accountsView.value.create.kept
        if (kept != null && kept.owners.isNotEmpty()) {
            owners.addAll(kept.owners)
            kept.members.forEach { (k, v) -> v.name?.let { names[k.lowercase()] = it } }
            name = kept.name
            threshold = kept.threshold
            thresholdFor = kept.owners.size
            kept.chainId?.let { chainId = it; chose = true }
            resumed = true
        } else {
            accounts.firstOrNull()?.let { owners.add(it.address) }
        }
        if (chainId == null) chainId = MsRules.firstDefaultNetwork(networks, hints) ?: networks.firstOrNull()
    }

    val cleanName = name.trim()
    val nameBytes = cleanName.toByteArray(Charsets.UTF_8).size
    val nameOk = cleanName.isNotEmpty() && MsRules.validName(cleanName)
    val nameError = when {
        cleanName.isEmpty() -> if (nameAsked) words.t("create.nameNeeded") else null
        nameBytes > NAME_LIMIT -> words.t("create.nameTooLong", "bytes" to "$nameBytes", "limit" to "$NAME_LIMIT")
        !nameOk -> words.t("create.nameInvalid")
        else -> null
    }
    fun isLocal(address: String) = accounts.any { it.address.equals(address, ignoreCase = true) }
    val local = owners.filter(::isLocal)
    fun label(address: String) = accounts.firstOrNull { it.address.equals(address, ignoreCase = true) }?.name
        ?: names[address.lowercase()] ?: words.label(address)

    // Every change is kept by the machine (a blank sheet keeps nothing).
    LaunchedEffect(owners.toList(), names.toMap(), name, threshold, chose, chainId) {
        if (!started || step == CreateStep.Created) return@LaunchedEffect
        val members = owners.associateWith { o -> names[o.lowercase()]?.takeIf { it.isNotEmpty() }?.let { mapOf("name" to it) } }.filterValues { it != null }
        hub.send(ev("keep_draft", "draft" to mapOf("owners" to owners.toList(), "members" to members, "name" to name, "threshold" to threshold, "chainId" to (if (chose) chainId else null))))
    }
    // r7: each member's kind, read once for the networks offered.
    LaunchedEffect(owners.toList()) {
        val ask = owners.filterNot(::isLocal).filter { hub.accountsView.value.memberKinds[it.lowercase()] == null }
            .map { mapOf("address" to it, "chain_ids" to networks) }
        if (ask.isNotEmpty()) hub.send(ev("read_kinds", "members" to ask))
    }
    // A name typed is read on Ethereum and shown as its address first.
    LaunchedEffect(view.add.ens) {
        val ens = view.add.ens
        if (ensAsked != null && ens["name"].str == ensAsked && ens.kind == "found") {
            ens["address"].str?.let { typed = it; if (typedName.isEmpty()) typedName = ensAsked.orEmpty() }
        }
    }
    fun askAddress() {
        if (owners.isNotEmpty() && nameOk) hub.send(ev("draft", "owners" to owners.toList(), "threshold" to threshold, "name" to cleanName))
    }
    fun addMember(address: String, memberName: String?): Boolean {
        val value = address.trim()
        when (MsRules.addressEntryOf(value)) {
            "valid" -> Unit
            "checksum" -> { typedError = words.t("send.invalidChecksum"); return false }
            else -> { typedError = words.t("send.invalidAddress"); return false }
        }
        if (!MsRules.validOwner(value)) { typedError = words.t("create.notMember"); return false }
        if (owners.any { it.equals(value, ignoreCase = true) }) { typedError = words.t("create.duplicate"); return false }
        val member = MsRules.eip55(value)
        memberName?.trim()?.takeIf { it.isNotEmpty() }?.let { names[member.lowercase()] = it.take(MsRules.memberNameMax) }
        owners.add(member)
        typed = ""; typedName = ""; typedError = null; ensAsked = null
        return true
    }
    fun toggle(address: String) {
        val at = owners.indexOfFirst { it.equals(address, ignoreCase = true) }
        if (at >= 0) owners.removeAt(at) else owners.add(address)
    }
    val close = { if (step == CreateStep.Created && created != null) onCreated(created!!) else onClose() }
    val colors = VelaTheme.colors

    MsSheet(words.t("create.title"), words.raw("common.done"), onClose = close, dismissible = sheet.phase != MsPhase.Working, tag = "multisig.create", scrollKey = step) {
        when (step) {
            CreateStep.Members -> {
                MsText(words.t("create.intro"), MsType.Body, color = colors.fgMuted)
                if (resumed) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        MsNote(words.t("create.draftKept"), Modifier.weight(1f))
                        MsTextButton(words.t("create.discardDraft"), tag = "multisig.create.discard") {
                            hub.send(ev("discard_draft"))
                            owners.clear(); accounts.firstOrNull()?.let { owners.add(it.address) }
                            names.clear(); name = ""; threshold = 1; thresholdFor = 0; nameAsked = false; chose = false
                            chainId = MsRules.firstDefaultNetwork(networks, hints); resumed = false
                        }
                    }
                }
                MsField(
                    label = if (nameBytes >= NAME_LIMIT - 8) "${words.t("create.name")} · ${words.t("create.nameBytes", "bytes" to "$nameBytes", "limit" to "$NAME_LIMIT")}" else words.t("create.name"),
                    value = name, onValueChange = { name = it }, placeholder = words.t("create.namePlaceholder"), error = nameError, tag = "multisig.create.name",
                )
                if (nameError == null) MsNote(words.t("create.nameHint"))
                MsSection(words.t("create.yourAccounts")) {
                    for (account in accounts) {
                        val on = owners.any { it.equals(account.address, ignoreCase = true) }
                        Row(
                            Modifier.fillMaxWidth().clickable(role = Role.Switch) { toggle(account.address) }.padding(vertical = VelaSpacing.sm)
                                .testTag("multisig.create.account")
                                .semantics(mergeDescendants = true) { stateDescription = if (on) "on" else "off" },
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                MsText(account.name, MsType.Field)
                                MsText(MultisigWords.short(account.address), MsType.Mono, color = colors.fgSubtle)
                            }
                            Switch(checked = on, onCheckedChange = { toggle(account.address) }, colors = SwitchDefaults.colors(checkedTrackColor = colors.fgBase))
                        }
                    }
                }
                MsSection(words.t("create.members")) {
                    for (owner in owners.filterNot(::isLocal)) {
                        Row(Modifier.fillMaxWidth().padding(vertical = VelaSpacing.sm), horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
                            IdenticonAvatar(seed = owner, size = VelaIconSize.xl, tappable = false)
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                                MsText(label(owner), MsType.Field)
                                MsText(MsRules.eip55(owner), MsType.Mono, color = colors.fgSubtle)
                                KindChoice(words, view.memberKinds[owner.lowercase()]) { kind -> hub.send(ev("pick_kind", "address" to owner, "kind" to kind)) }
                            }
                            Box(Modifier.size(VelaIconSize.xl).clickable(role = Role.Button) { toggle(owner) }.semantics { contentDescription = words.t("members.remove") }, contentAlignment = Alignment.Center) {
                                Icon(VelaIcons.Close, null, tint = colors.fgSubtle, modifier = Modifier.size(VelaIconSize.base))
                            }
                        }
                    }
                    MsField(words.t("create.byAddress"), typed, { typed = it; typedError = null }, words.t("create.addressPlaceholder"), error = typedError, tag = "multisig.create.address")
                    ensAsked?.let { asked ->
                        val ens = view.add.ens
                        if (ens["name"].str == asked) {
                            val (text, tone) = when (ens.kind) {
                                "found" -> "${words.t("create.ensResolved", "name" to asked)} ${ens["address"].str.orEmpty()}" to MsTone.Neutral
                                "none" -> words.t("create.ensNotFound", "name" to asked) to MsTone.Warning
                                "unavailable" -> words.t("create.ensUnavailable", "name" to asked) to MsTone.Warning
                                else -> words.t("create.ensUnsupported", "name" to asked) to MsTone.Warning
                            }
                            MsNote(text, tone = tone, tag = "multisig.create.ens")
                        }
                    }
                    MsField(words.t("create.memberName"), typedName, { typedName = it }, words.t("create.memberNamePlaceholder"), tag = "multisig.create.memberName")
                    Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xl)) {
                        MsTextButton(words.t("create.addMember"), enabled = typed.isNotBlank(), tag = "multisig.create.addMember") {
                            val value = typed.trim()
                            if (value.contains(".") && !value.startsWith("0x")) {
                                ensAsked = value
                                typedError = null
                                hub.send(ev("resolve_ens", "name" to value))
                            } else {
                                addMember(value, typedName)
                            }
                        }
                    }
                    if (contacts.isNotEmpty()) {
                        var picking by remember { mutableStateOf(false) }
                        MsTextButton(words.t("create.fromContacts"), tag = "multisig.create.fromContacts") { picking = !picking }
                        if (picking) {
                            for (c in contacts.filter { c -> owners.none { it.equals(c.address, ignoreCase = true) } }) {
                                MsNavRow(c.name.ifBlank { MultisigWords.short(c.address) }, subtitle = MultisigWords.short(c.address)) {
                                    addMember(c.address, c.name)
                                    picking = false
                                }
                            }
                        }
                    }
                    MsNote(words.t("create.order"))
                }
                if (local.isEmpty()) MsNote(words.t("create.needLocal"), tone = MsTone.Warning, tag = "multisig.create.needLocal")
                VelaPrimaryButton(
                    text = words.t("create.next"),
                    onClick = {
                        if (!nameOk) {
                            nameAsked = true
                        } else {
                            if (thresholdFor != owners.size) {
                                threshold = if (owners.size <= 2) owners.size else owners.size - 1
                                thresholdFor = owners.size
                            }
                            step = CreateStep.Threshold
                        }
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.create.next"),
                    enabled = owners.isNotEmpty() && local.isNotEmpty(),
                )
            }

            CreateStep.Threshold -> {
                MsSection(words.t("create.threshold")) {
                    Stepper(
                        text = words.ruleText(threshold, owners.size),
                        canFewer = threshold > 1, canMore = threshold < owners.size,
                        fewer = words.t("create.fewer"), more = words.t("create.more"),
                        tag = "multisig.create.threshold",
                        onStep = { by -> threshold = (threshold + by).coerceIn(1, maxOf(1, owners.size)) },
                    )
                    val spare = owners.size - threshold
                    val note = when {
                        owners.size == 1 -> words.t("create.solo")
                        threshold == owners.size -> words.t("create.thresholdAll")
                        threshold == 1 -> words.t("create.thresholdOne")
                        else -> words.plural("create.thresholdSpare", spare)
                    }
                    val risky = owners.size > 1 && (threshold == owners.size || threshold == 1)
                    MsNote(note, tone = if (risky) MsTone.Warning else MsTone.Neutral, tag = "multisig.create.thresholdNote")
                }
                MsSection(words.t("create.network")) {
                    if (networks.isEmpty()) MsNote(words.t("create.noSafeNetworks"), tone = MsTone.Warning)
                    for (id in networks) {
                        ChoiceRow(words.network(id), id == chainId, "multisig.create.network.$id") { chainId = id; chose = true }
                    }
                    MsNote(words.t("create.networkHint"))
                    if (sheet.otherSpace.isNotEmpty()) {
                        MsNote(words.t("create.otherSpace", "space" to otherSpaceName, "networks" to sheet.otherSpace.joinToString(", ", transform = words::network)), tag = "multisig.create.otherSpace")
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
                    VelaSecondaryButton(words.t("create.back"), { step = CreateStep.Members }, Modifier.weight(1f))
                    VelaPrimaryButton(words.t("create.next"), { askAddress(); step = CreateStep.Review }, Modifier.weight(1f).testTag("multisig.create.toReview"), enabled = chainId != null)
                }
            }

            CreateStep.Review -> {
                LaunchedEffect(Unit) { askAddress() }
                val chosenCreator = creator?.let { c -> local.firstOrNull { it.equals(c, ignoreCase = true) } } ?: local.firstOrNull()
                MsRow(words.t("create.name"), cleanName)
                MsRow(words.t("create.threshold"), words.ruleText(threshold, owners.size))
                MsSection(words.t("create.members")) {
                    for (o in owners) MsRow(label(o), MsRules.eip55(o), mono = true)
                }
                when {
                    sheet.draftAddress != null -> {
                        MsRow(words.t("create.address"), sheet.draftAddress, mono = true, copyable = true, tag = "multisig.create.draftAddress")
                        MsNote(words.t("create.sameEverywhere"))
                    }
                    sheet.draftError != null -> MsNote(sheet.draftError, tone = MsTone.Danger)
                    else -> CircularProgressIndicator(color = colors.fgMuted, strokeWidth = VelaSpacing.xs)
                }
                chainId?.let { MsRow(words.t("create.network"), words.network(it)) }
                if (local.size > 1) {
                    MemberPicker(words, words.t("create.creator"), local, chosenCreator, "multisig.create.creator") { creator = it }
                } else if (chosenCreator != null) {
                    MsRow(words.t("create.creator"), label(chosenCreator))
                }
                if (sheet.phase == MsPhase.Failed && sheet.error != null) MsNote(words.failureText(sheet.error), tone = MsTone.Danger, tag = "multisig.create.failed")
                val working = sheet.phase == MsPhase.Working
                // Create: the ink (the accent is only Send, Approve, Propose and approve, Execute).
                VelaPrimaryButton(
                    text = words.t("create.create"),
                    onClick = {
                        val c = chosenCreator ?: return@VelaPrimaryButton
                        val chain = chainId ?: return@VelaPrimaryButton
                        haptic(VelaHaptic.Press)
                        val members = owners.associateWith { o -> names[o.lowercase()]?.takeIf { it.isNotEmpty() }?.let { mapOf("name" to it) } ?: emptyMap() }
                        scope.launch {
                            hub.refreshLocal()
                            val create = ev("create", "choice" to mapOf("owners" to owners.toList(), "threshold" to threshold, "name" to cleanName, "chain_id" to chain, "creator" to c, "members" to members))
                            val first = hub.accounts.request(create, 300_000) { v ->
                                val create = MsAccountsView.of(v).create
                                if (create.address != null || create.phase != MsPhase.Working) create else null
                            }
                            first?.address?.let {
                                created = it
                                step = CreateStep.Created
                                haptic(VelaHaptic.Success)
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.create.create"),
                    enabled = sheet.draftAddress != null && chosenCreator != null && chainId != null && !working,
                    loading = working,
                )
                VelaSecondaryButton(words.t("create.back"), { step = CreateStep.Threshold }, Modifier.fillMaxWidth(), enabled = !working)
            }

            CreateStep.Created -> {
                val row = view.row(created)
                val solo = owners.size == 1
                val shared = when {
                    row?.shared == true -> words.t(if (solo) "create.sharedSolo" else "create.shared")
                    sheet.refused != null -> words.t("create.refused", "reason" to sheet.refused)
                    sheet.phase == MsPhase.Working -> words.t("create.savedSharing")
                    else -> words.t("create.savedLater")
                }
                MsText(words.t("create.created"), MsType.Title, Modifier.testTag("multisig.create.created"))
                MsNote(shared, tone = if (row?.shared == true) MsTone.Positive else if (sheet.refused != null) MsTone.Warning else MsTone.Neutral, tag = "multisig.create.shared")
                created?.let { MsRow(words.t("create.address"), it, mono = true, copyable = true) }
                MsNote(words.t(if (solo) "create.createdBodySolo" else "create.createdBody"))
                if (!solo) created?.let { address ->
                    VelaPrimaryButton(words.t("create.copyInvite"), { onInvite(address) }, Modifier.fillMaxWidth().testTag("multisig.create.invite"))
                }
                VelaSecondaryButton(words.t("create.done"), { created?.let(onCreated) }, Modifier.fillMaxWidth().testTag("multisig.create.done"))
            }
        }
    }
}

private const val NAME_LIMIT = 64

/** −  value  + : a threshold stepper (the core says what may move). */
@Composable
fun Stepper(text: String, canFewer: Boolean, canMore: Boolean, fewer: String, more: String, tag: String, onStep: (Int) -> Unit) {
    val colors = VelaTheme.colors
    val haptic = rememberVelaHaptic()
    Row(Modifier.fillMaxWidth().testTag(tag), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
        MsText(text, MsType.RowTitle, Modifier.weight(1f).testTag("$tag.value"))
        for ((label, by, enabled) in listOf(Triple(fewer, -1, canFewer), Triple(more, 1, canMore))) {
            Box(
                Modifier.size(app.getvela.wallet.core.designsystem.tokens.VelaSizing.hitTarget)
                    .clickable(enabled = enabled, role = Role.Button) { haptic(VelaHaptic.Detent); onStep(by) }
                    .semantics { contentDescription = label }
                    .testTag(if (by < 0) "$tag.fewer" else "$tag.more"),
                contentAlignment = Alignment.Center,
            ) {
                MsText(if (by < 0) "−" else "+", MsType.Title, color = if (enabled) colors.fgBase else colors.fgSubtle)
            }
        }
    }
}

/** One choice among several, ticked when chosen. */
@Composable
fun ChoiceRow(text: String, selected: Boolean, tag: String, onPick: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        Modifier.fillMaxWidth().clickable(role = Role.RadioButton, onClick = onPick).padding(vertical = VelaSpacing.md).testTag(tag)
            .semantics(mergeDescendants = true) { stateDescription = if (selected) "selected" else "" },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        MsText(text, MsType.Field, Modifier.weight(1f))
        if (selected) Icon(VelaIcons.Check, null, tint = colors.fgBase, modifier = Modifier.size(VelaIconSize.lg))
    }
}

/**
 * r7: a member's kind — the accounts machine's (`member_kinds`): what the
 * chains say of it, else the person's word, else asked; "Other wallet" hinted
 * when no Vela account is found, kept only if picked.
 */
@Composable
fun KindChoice(words: MultisigWords, choice: MsKindChoice?, onPick: (String?) -> Unit) {
    choice ?: return
    val colors = VelaTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
        if (choice.locked) {
            MsChip(
                when (choice.kind) {
                    "wallet" -> words.t("members.kindWallet")
                    "contract" -> words.t("members.kindContract")
                    else -> words.t("members.kindVela")
                },
            )
        } else {
            Row(Modifier.testTag("multisig.kind"), horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                for ((kind, label) in listOf("vela" to words.t("members.kindVela"), "wallet" to words.t("members.kindWallet"))) {
                    val on = choice.kind == kind
                    Box(Modifier.clickable(role = Role.RadioButton) { onPick(if (on) null else kind) }.testTag("multisig.kind.$kind")) {
                        MsChip(label, tone = if (on) MsTone.Positive else MsTone.Neutral)
                    }
                }
            }
        }
        when (choice.note) {
            "known" -> words.t("members.kindKnown")
            "contract" -> words.t("members.kindContractNote")
            "likely" -> words.t("members.kindLikely")
            "ask" -> words.t("members.kindAsk")
            else -> null
        }?.let { MsText(it, MsType.Sub, color = colors.fgSubtle) }
    }
}

/**
 * Adding a multisig (M1) — the web's `AddMultisigSheet`, on the accounts
 * machine: by its address, from an invite (its networks and its members'
 * labels ride along; nothing is added until the person's Add), or imported
 * from a Safe{Wallet} export (read by the core, on this device).
 *
 * Who can act for it is the chain's, or the core's counterfactual rule
 * (§3.5): a genesis the core refuses is said with every reason and nothing
 * adds it; one something besides its members can act for is shown with
 * what that is, and added only on the person's explicit tap.
 */
@Composable
fun MultisigAddSheet(
    hub: MultisigHub,
    words: MultisigWords,
    invite: MsLinkStand?,
    prefill: String?,
    spaceNetworks: List<Int>,
    onClose: () -> Unit,
    onAdded: (String) -> Unit,
) {
    val view by hub.accountsView.collectAsStateWithLifecycle()
    val haptic = rememberVelaHaptic()
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var address by remember { mutableStateOf(invite?.multisig ?: prefill ?: "") }
    var failure by remember {
        mutableStateOf(invite?.fault?.let { words.t(if (it == "checksum") "add.badChecksum" else "add.badInvite") })
    }
    var choosing by remember { mutableStateOf(false) }
    var chosen by remember { mutableStateOf<Int?>(null) }
    var refused by remember { mutableStateOf<List<JsonElement>?>(null) }
    var risky by remember { mutableStateOf<List<JsonElement>?>(null) }
    var busy by remember { mutableStateOf(false) }
    val picked = remember { mutableStateListOf<String>() }
    val linkChains = invite?.chainIds ?: emptyList()
    val fromLink = invite?.multisig?.equals(address.trim(), ignoreCase = true) == true
    val colors = VelaTheme.colors

    val pickExport = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        val text = runCatching { context.contentResolver.openInputStream(uri)?.use { it.readBytes().toString(Charsets.UTF_8) } }.getOrNull()
        if (text != null) {
            picked.clear()
            hub.send(ev("parse_export", "json" to text))
        }
    }

    fun said(outcome: JsonElement?) {
        when (outcome.kind) {
            "added" -> {
                val added = outcome["address"].str ?: address
                if (fromLink && invite != null) {
                    hub.send(
                        ev(
                            "keep_invited", "address" to added, "chain_ids" to linkChains,
                            "members" to invite.link["members"].arr.map { mapOf("address" to it["address"], "name" to it["name"], "kind" to it["kind"]) },
                        ),
                    )
                }
                haptic(VelaHaptic.Success)
                onAdded(added)
            }
            "chooseNetwork" -> choosing = true
            "refused" -> refused = outcome["findings"].arr
            "highRisk" -> risky = outcome["findings"].arr
            "unverified" -> failure = words.t("authority.unverified", "network" to words.network(outcome["chain_id"].int ?: 0))
            "notFound", "unavailable", "unsupported", "notMultisig", "unchecked" -> failure = words.t("add.${outcome.kind}")
            else -> failure = words.t("add.unavailable")
        }
    }
    fun add(acceptRisks: Boolean) {
        val typed = address.trim()
        when (MsRules.addressEntryOf(typed)) {
            "valid" -> Unit
            "checksum" -> { failure = words.t("send.invalidChecksum"); return }
            else -> { failure = words.t("send.invalidAddress"); return }
        }
        busy = true; failure = null; refused = null
        if (!acceptRisks) risky = null
        val named = if (choosing) chosen else if (fromLink) linkChains.firstOrNull() else null
        scope.launch {
            val outcome = hub.accounts.request(ev("add", "address" to typed, "network" to named, "accept_risks" to acceptRisks), 120_000) { v ->
                val add = MsAccountsView.of(v).add
                if ((add.phase == MsPhase.Done || add.phase == MsPhase.Failed) && add.address.equals(typed, ignoreCase = true)) add.outcome ?: JsonNull else null
            }
            busy = false
            said(outcome)
        }
    }

    MsSheet(words.t("add.title"), words.raw("common.done"), onClose = onClose, tag = "multisig.add") {
        MsText(words.t("add.intro"), MsType.Body, color = colors.fgMuted)
        if (invite != null) {
            Column(Modifier.testTag("multisig.add.invite"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                if (linkChains.isNotEmpty()) MsNote(words.t("add.usedOn", "networks" to linkChains.joinToString(", ", transform = words::network)))
                val members = invite.link["members"].arr
                if (members.isNotEmpty()) {
                    MsText(words.t("add.invitedMembers"), MsType.Label, color = colors.fgSubtle)
                    for (m in members) {
                        val a = m["address"].str
                        val kind = m["kind"].str?.let { if (it == "wallet") words.t("add.kindWallet") else words.t("add.kindPasskey") }
                        val you = a?.let { words.localName(it) }?.let { words.t("add.thatsYou") }
                        val line = listOfNotNull(m["name"].str, kind, you).filter { it.isNotEmpty() }.joinToString(" · ")
                        Column {
                            if (line.isNotEmpty()) MsText(line, MsType.Sub)
                            a?.let { MsText(MsRules.eip55(it), MsType.Mono, color = colors.fgMuted) }
                        }
                    }
                    MsNote(words.t("add.labelsNote"))
                }
            }
        }
        MsField(words.t("add.title"), address, { address = it; failure = null }, words.t("add.placeholder"), error = failure, tag = "multisig.add.address")
        if (choosing) {
            MsNote(words.t("add.chooseNetwork"), tone = MsTone.Warning)
            val offered = MsRules.likelyNetworks(spaceNetworks, mapOf("named" to if (fromLink) linkChains else emptyList<Int>())).ifEmpty { spaceNetworks }
            for (id in offered) ChoiceRow(words.network(id), id == chosen, "multisig.add.network.$id") { chosen = id }
        }
        refused?.let { list ->
            MsSection(words.t("authority.refusedTitle"), Modifier.testTag("multisig.add.refused")) {
                MsNote(words.t("authority.refusedBody"), tone = MsTone.Danger)
                for (f in list) MsNote(words.finding(f), tone = MsTone.Danger)
            }
        }
        risky?.let { list ->
            MsSection(words.t("authority.riskTitle")) {
                MsNote(words.t("authority.riskBody"), tone = MsTone.Danger)
                for (f in list) MsNote(words.finding(f), tone = MsTone.Danger)
                VelaDangerButton(words.t("authority.riskConfirm"), { add(true) }, Modifier.fillMaxWidth().testTag("multisig.add.acceptRisk"))
            }
        }
        VelaPrimaryButton(
            text = words.t("add.add"),
            onClick = { add(false) },
            modifier = Modifier.fillMaxWidth().testTag("multisig.add.add"),
            enabled = address.isNotBlank() && !(choosing && chosen == null) && !busy,
            loading = busy,
        )
        // Import from Safe{Wallet}: the core reads the export, on this device.
        MsSection(words.t("import.or")) {
            MsNote(words.t("import.body"))
            MsTextButton(words.t("import.choose"), tag = "multisig.import.choose") { pickExport.launch(arrayOf("application/json", "text/plain")) }
            view.importing.error?.let { MsNote(words.t(if (it == "empty") "import.empty" else "import.notExport"), tone = MsTone.Warning) }
            val safes = view.importing.safes
            if (safes.isNotEmpty()) {
                MsText(words.t("import.pick"), MsType.Label, color = colors.fgSubtle)
                for (safe in safes) {
                    val a = safe["address"].str.orEmpty()
                    val already = view.row(a) != null
                    val outcome = view.importing.outcomes[a.lowercase()] ?: view.importing.outcomes[a]
                    val deployed = safe["chains"].arr.any { it["deployed"].bool == true }
                    val status = if (already) words.t("import.already") else outcome?.let {
                        when (it.kind) {
                            "imported" -> words.t("import.imported")
                            "notDeployed" -> words.t("import.notDeployed")
                            "refused" -> words.t("authority.refusedFlag")
                            "highRisk" -> words.t("authority.riskFlag")
                            else -> words.t("add.${it.kind ?: "unavailable"}")
                        }
                    }
                    val on = a in picked
                    Row(
                        Modifier.fillMaxWidth().clickable(enabled = !already, role = Role.Checkbox) { if (on) picked.remove(a) else picked.add(a) }
                            .padding(vertical = VelaSpacing.sm).testTag("multisig.import.safe"),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            MsText(safe["name"].str ?: MultisigWords.short(a), MsType.Field)
                            MsText(listOfNotNull(MultisigWords.short(a), if (deployed) null else words.t("import.undeployed"), status).joinToString(" · "), MsType.Sub, color = colors.fgSubtle)
                        }
                        Switch(checked = on || already, onCheckedChange = null, enabled = !already, colors = SwitchDefaults.colors(checkedTrackColor = colors.fgBase))
                    }
                }
                VelaPrimaryButton(
                    text = words.t("import.importSome", "total" to picked.size.toString()),
                    onClick = { picked.filter { view.row(it) == null }.forEach { hub.send(ev("import", "address" to it, "accept_risks" to false)) } },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.import.go"),
                    enabled = picked.isNotEmpty() && view.importing.busy == null,
                    loading = view.importing.busy != null,
                )
            }
        }
    }
}

/**
 * A multisig link the app was opened with, where it stands on the accounts
 * machine's one link flow (`MultisigLinkStand`): opening, a network this
 * device lacks, a member here (its transaction opens), a preview, failed,
 * damaged, or kept until there is an account. The native flow — never "open
 * the web wallet".
 */
@Composable
fun MultisigLinkStandSheet(hub: MultisigHub, words: MultisigWords, onClose: () -> Unit, onAddNetwork: ((Int) -> Unit)?) {
    val view by hub.accountsView.collectAsStateWithLifecycle()
    val stand = view.link
    val close = {
        // The person closed the missing-network sheet: the waiting link is dropped (W9).
        if (stand?.kind == "missingNetwork") hub.send(ev("drop_link")) else hub.send(ev("close_link"))
        onClose()
    }
    MsSheet(if (stand?.kind == "invite") words.t("add.title") else words.t("proposal.title"), words.raw("common.done"), onClose = close, tag = "multisig.link") {
        when (stand?.kind) {
            "broken" -> MsNote(stand.text ?: words.t("proposal.badLink"), tone = MsTone.Warning, tag = "multisig.link.broken")
            "missingNetwork" -> {
                val chainId = stand.chainId ?: 0
                MsNote(words.t("proposal.missingNetwork", "chainId" to chainId.toString()), tone = MsTone.Warning, tag = "multisig.link.missingNetwork")
                MsNote(words.t("proposal.missingNetworkBody"))
                onAddNetwork?.let { add -> VelaPrimaryButton(words.t("proposal.addNetwork"), { add(chainId) }, Modifier.fillMaxWidth().testTag("multisig.link.addNetwork")) }
            }
            "failed" -> {
                val reason = stand.reason
                val line = when (reason.kind) {
                    null, "notFound" -> words.t("proposal.notFound")
                    "unavailable" -> words.t("add.unavailable")
                    else -> words.t("add.${reason.kind}")
                }
                MsNote(line, tone = MsTone.Warning, tag = "multisig.link.failed")
            }
            "kept" -> MsNote(words.t("proposal.previewNote"), tag = "multisig.link.kept")
            else -> Row(Modifier.testTag("multisig.link.opening"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                CircularProgressIndicator(color = VelaTheme.colors.fgMuted, strokeWidth = VelaSpacing.xs)
                MsText(words.t("proposal.opening"), MsType.Body, color = VelaTheme.colors.fgMuted)
            }
        }
    }
}

/**
 * The invite (M1): a code and a link that add the multisig in Vela with its
 * network and the members' names (the core's `inviteLink`), handed on by the
 * share sheet or copy — the home's first step ticks (`invite_shared`).
 */
@Composable
fun MultisigInviteSheet(hub: MultisigHub, words: MultisigWords, address: String, origin: String, locale: String, onClose: () -> Unit) {
    val view by hub.accountsView.collectAsStateWithLifecycle()
    val proposals by hub.proposalsView.collectAsStateWithLifecycle()
    val row = view.row(address)
    val link = row?.let { r ->
        MsRules.inviteLink(
            origin, locale, r.account.address, r.account.owners, r.account.chainIds, proposals.membersAnywhere,
            r.account.members.mapValues { (_, v) -> mapOf("name" to v.name, "kind" to v.kind) },
        )
    }
    LaunchedEffect(address) { hub.send(ev("invite_shared", "address" to address)) }
    MsSheet(words.t("panel.invite"), words.raw("common.done"), onClose = onClose, tag = "multisig.invite") {
        MsNote(words.t("invite.body"))
        if (link != null) {
            QrCard(label = words.t("invite.qrLabel"), payload = link, modifier = Modifier.testTag("multisig.invite.qr"))
            MsText(link, MsType.Mono, Modifier.testTag("multisig.invite.link"), color = VelaTheme.colors.fgMuted)
            ShareRow(words, link, "multisig.invite")
        }
        MsNote(words.t("invite.outsideHint"))
    }
}
