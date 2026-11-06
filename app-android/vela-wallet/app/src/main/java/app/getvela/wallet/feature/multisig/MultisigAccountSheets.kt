package app.getvela.wallet.feature.multisig

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.selection.selectable
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
import app.getvela.wallet.feature.multisig.core.ints
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
    // How many must approve — the accounts machine's (`create.threshold`):
    // the sensible start for these members, or the person's pick for them
    // (`step_threshold`).
    val thresholdView = sheet.threshold
    val threshold = thresholdView["value"].int ?: 1
    var chainId by remember { mutableStateOf<Int?>(null) }
    var chose by remember { mutableStateOf(false) }
    var creator by remember { mutableStateOf<String?>(null) }
    var resumed by remember { mutableStateOf(false) }
    var created by remember { mutableStateOf<String?>(null) }
    var ensAsked by remember { mutableStateOf<String?>(null) }
    var started by remember { mutableStateOf(false) }

    // The networks offered: the space's, minus those found without the Safe
    // contracts, the likely ones first (the core's order).
    // r5reg-19: the machine's `used_networks` / `recent_networks`.
    val hints = remember(view.usedNetworks, view.recentNetworks) { view.networkHints() }
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
    // The draft's address, as the members, the rule and the name change (its
    // members are also what the machine's threshold step is for).
    LaunchedEffect(owners.toList(), threshold, cleanName, nameOk) {
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
    // r10 S4 (real device): Back — reached for to put the keyboard away —
    // closed a filled form without a word. A form with something in it asks
    // first, and says what is kept (the machine's draft).
    val filled = step != CreateStep.Created && createFilled(name, owners.filterNot(::isLocal).size, typed, typedName)
    var asking by remember { mutableStateOf(false) }
    val close = {
        when {
            step == CreateStep.Created && created != null -> onCreated(created!!)
            filled -> asking = true
            else -> onClose()
        }
    }
    val colors = VelaTheme.colors
    if (asking) {
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { asking = false },
            title = { androidx.compose.material3.Text(words.t("create.closeTitle")) },
            text = { androidx.compose.material3.Text(words.t("create.closeBody")) },
            confirmButton = {
                androidx.compose.material3.TextButton(onClick = {
                    asking = false
                    // A member still being typed goes into the kept draft too, when it reads as one.
                    if (typed.isNotBlank()) addMember(typed, typedName)
                    onClose()
                }, modifier = Modifier.testTag("multisig.create.closeConfirm")) { androidx.compose.material3.Text(words.t("create.closeConfirm")) }
            },
            dismissButton = {
                androidx.compose.material3.TextButton(onClick = { asking = false }, modifier = Modifier.testTag("multisig.create.closeKeep")) {
                    androidx.compose.material3.Text(words.t("create.closeKeep"))
                }
            },
            modifier = Modifier.testTag("multisig.create.closeAsk"),
        )
    }

    MsSheet(words.t("create.title"), words.raw("common.done"), onClose = close, dismissible = sheet.phase != MsPhase.Working, tag = "multisig.create", scrollKey = step, guarded = filled) {
        when (step) {
            CreateStep.Members -> {
                MsText(words.t("create.intro"), MsType.Body, color = colors.fgMuted)
                if (resumed) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        MsNote(words.t("create.draftKept"), Modifier.weight(1f))
                        MsTextButton(words.t("create.discardDraft"), tag = "multisig.create.discard") {
                            hub.send(ev("discard_draft"))
                            owners.clear(); accounts.firstOrNull()?.let { owners.add(it.address) }
                            names.clear(); name = ""; nameAsked = false; chose = false
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
                            // r10 S4: the row is the switch and says whether it is on
                            // (TalkBack read "unchecked" for every one: a clickable with a
                            // switch's role and no state, and an English "on"/"off").
                            Modifier.fillMaxWidth().toggleable(value = on, role = Role.Switch) { toggle(account.address) }.padding(vertical = VelaSpacing.sm)
                                .testTag("multisig.create.account"),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                MsText(account.name, MsType.Field)
                                MsText(MultisigWords.short(account.address), MsType.Mono, color = colors.fgSubtle)
                            }
                            Switch(checked = on, onCheckedChange = null, colors = SwitchDefaults.colors(checkedTrackColor = colors.fgBase))
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
                            // Where it starts is the machine's (`create.threshold`).
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
                        canFewer = thresholdView["can_lower"].bool ?: false, canMore = thresholdView["can_raise"].bool ?: false,
                        fewer = words.t("create.fewer"), more = words.t("create.more"),
                        tag = "multisig.create.threshold",
                        onStep = { by -> hub.send(ev("step_threshold", "up" to (by > 0))) },
                    )
                    val noteView = thresholdView["note"]
                    val note = when (noteView.kind) {
                        "solo" -> words.t("create.solo")
                        "all" -> words.t("create.thresholdAll")
                        "one" -> words.t("create.thresholdOne")
                        "spare" -> words.plural("create.thresholdSpare", noteView["count"].int ?: 0)
                        else -> null
                    }
                    val risky = thresholdView["risky"].bool ?: false
                    note?.let { MsNote(it, tone = if (risky) MsTone.Warning else MsTone.Neutral, tag = "multisig.create.thresholdNote") }
                    // Passkey-native recovery: a lost passkey's member is replaced by the others.
                    if (threshold < owners.size) {
                        MsText(words.t("members.recovery", "m" to "$threshold", "n" to "${owners.size}"), MsType.Sub, Modifier.testTag("multisig.create.recovery"), color = colors.fgSubtle)
                    }
                }
                MsSection(words.t("create.network")) {
                    if (networks.isEmpty()) MsNote(words.t("create.noSafeNetworks"), tone = MsTone.Warning)
                    for (id in networks) {
                        ChoiceRow(words.network(id), id == chainId, "multisig.create.network.$id") { chainId = id; chose = true }
                    }
                    MsNote(words.t("create.networkHint"))
                    if (sheet.otherSpace.isNotEmpty()) {
                        MsNote(words.t("create.otherSpace", "space" to otherSpaceName, "networks" to words.list(sheet.otherSpace.map(words::network))), tag = "multisig.create.otherSpace")
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
                    VelaSecondaryButton(words.t("create.back"), { step = CreateStep.Members }, Modifier.weight(1f))
                    VelaPrimaryButton(words.t("create.next"), { step = CreateStep.Review }, Modifier.weight(1f).testTag("multisig.create.toReview"), enabled = chainId != null)
                }
            }

            CreateStep.Review -> {
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
                    // Round 16 leftover 4: `draft_error` is the core's English — never drawn.
                    sheet.draftError != null -> MsNote(words.createDraftFailed, tone = MsTone.Danger, tag = "multisig.create.draftFailed")
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
                    // Round 16 leftover 4: the service's refusal text is never drawn; Settings offers "Share it again".
                    sheet.refused != null -> words.createRefused
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

/**
 * r10 S4: the create form holds something a close would take from the
 * person's sight: a name, a member beyond this device's own accounts, or a
 * member being typed. (This device's accounts alone are the form's start.)
 */
internal fun createFilled(name: String, others: Int, typed: String, typedName: String): Boolean =
    name.isNotBlank() || others > 0 || typed.isNotBlank() || typedName.isNotBlank()

/** One choice among several, ticked when chosen. */
@Composable
fun ChoiceRow(text: String, selected: Boolean, tag: String, onPick: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        // r10 S4: selectable — the chosen network is said as selected, never "not checked".
        Modifier.fillMaxWidth().selectable(selected = selected, role = Role.RadioButton, onClick = onPick).padding(vertical = VelaSpacing.md).testTag(tag),
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
                    Box(Modifier.selectable(selected = on, role = Role.RadioButton) { onPick(if (on) null else kind) }.testTag("multisig.kind.$kind")) {
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
 * The add sheet's press (r14-5): "Try again" once the outcome said is a read
 * that did not come back ([retry], the core's `view.add.retry`) and is still
 * on screen ([failed]); "Add" otherwise.
 */
internal fun addPressLabel(words: MultisigWords, retry: Boolean, failed: Boolean): String =
    if (retry && failed) words.t("review.state.tryAgain") else words.t("add.add")

/**
 * One exported Safe's import, in words — the core's outcome as it said it:
 * a network that only said "slow down" is named busy (r14 leftovers), never
 * "unavailable"; `null` for nothing said yet.
 */
/**
 * r15-8 (the web's `here`): "Already added" is for a Safe that was here
 * before this import said anything of it. One this import added — by its
 * row's Try again after a busy answer, say — is what the import came to
 * ("Imported"), never "Already added".
 */
internal fun importAlready(here: Boolean, outcome: JsonElement?): Boolean = here && outcome == null

/** One exported Safe's row status: "Already added", else what its import came to ([importStatus]). */
internal fun importRowStatus(words: MultisigWords, here: Boolean, outcome: JsonElement?): String? =
    if (importAlready(here, outcome)) words.t("import.already") else importStatus(words, outcome)

internal fun importStatus(words: MultisigWords, outcome: JsonElement?): String? = outcome?.let {
    when (it.kind) {
        "imported" -> words.t("import.imported")
        "notDeployed" -> words.t("import.notDeployed")
        "refused" -> words.t("authority.refusedFlag")
        "highRisk" -> words.t("authority.riskFlag")
        "busy" -> words.t("add.busy", "network" to words.network(it["chain_id"].int ?: 0))
        "unverified" -> words.t("authority.unverified", "network" to words.network(it["chain_id"].int ?: 0))
        "notFound", "unavailable", "unsupported" -> words.t("add.${it.kind}")
        else -> words.t("add.unavailable")
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
    onAddNetwork: ((Int) -> Unit)? = null,
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
    // r14-5: the outcome said is a read that did not come back — the core's
    // `view.add.retry` — so Add reads Try again (web, iOS, desktop alike).
    var retry by remember { mutableStateOf(false) }
    val picked = remember { mutableStateListOf<String>() }
    // The invite as the accounts machine keeps it (`invite`): the networks
    // this device has, the first it lacks and its name. Adding its multisig
    // uses its networks and keeps its members' labels — the machine's, on Add.
    val kept = view.invite?.takeIf { k -> invite != null && k["link"]["multisig"].str.equals(invite.multisig, ignoreCase = true) }
    val linkChains = kept?.get("on_device").ints()
    val linkLacks = kept?.get("missing").int
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
                // The invite's networks and members' labels landed with the add (the machine's).
                val added = outcome["address"].str ?: address
                haptic(VelaHaptic.Success)
                onAdded(added)
            }
            "chooseNetwork" -> choosing = true
            "refused" -> refused = outcome["findings"].arr
            "highRisk" -> risky = outcome["findings"].arr
            "unverified" -> failure = words.t("authority.unverified", "network" to words.network(outcome["chain_id"].int ?: 0))
            // r14-5 (core): its network's nodes only said "slow down" — named, never "unavailable".
            "busy" -> failure = words.t("add.busy", "network" to words.network(outcome["chain_id"].int ?: 0))
            "notFound", "unavailable", "unsupported", "notMultisig", "unchecked" -> failure = words.t("add.${outcome.kind}")
            else -> failure = words.t("add.unavailable")
        }
        retry = failure != null && MsAccountsView.of(hub.accounts.view.value).add.retry
    }
    // r9-16: an invite (or a transaction) link pasted here is read by the
    // core's link reading (`ScannedLink`, `carriedLinkHash`) and opens in the
    // accounts machine's one link flow — the invite's own add sheet, with its
    // members and networks — never refused as "not an address".
    fun openedAsLink(text: String): Boolean {
        val hash = app.getvela.wallet.feature.wallet.core.ScannedLink.multisigHash(text) ?: return false
        hub.openLink(hash)
        hub.sheet.value = app.getvela.wallet.feature.multisig.core.MultisigSheet.Link
        return true
    }
    fun add(acceptRisks: Boolean) {
        val typed = address.trim()
        if (openedAsLink(typed)) return
        when (MsRules.addressEntryOf(typed)) {
            "valid" -> Unit
            "checksum" -> { failure = words.t("send.invalidChecksum"); return }
            else -> { failure = words.t("send.invalidAddress"); return }
        }
        busy = true; failure = null; refused = null; retry = false
        if (!acceptRisks) risky = null
        // The person's pick when asked; an invite's own network is the machine's.
        val named = if (choosing) chosen else null
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
                if (linkChains.isNotEmpty()) {
                    MsNote(words.t("add.usedOn", "networks" to words.list(linkChains.map(words::network))))
                } else if (linkLacks != null) {
                    // r13-6: ONE corpus sentence, its `{{network}}` the network's
                    // name — never another sentence ("It's used on On Arc Testnet
                    // (chain 5042002) — a network this device doesn't have, which
                    // this device doesn't have yet").
                    MsNote(words.t("add.networkMissing", "network" to words.missingName(linkLacks, kept?.get("missing_name").str)), tone = MsTone.Warning, tag = "multisig.add.networkMissing")
                    onAddNetwork?.let { addNetwork -> VelaSecondaryButton(words.t("proposal.addNetwork"), { addNetwork(linkLacks) }, Modifier.fillMaxWidth()) }
                }
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
        MsField(words.t("add.title"), address, { typed ->
            val pasted = typed.length - address.length > 1
            address = typed
            failure = null
            retry = false
            // A link pasted whole opens at once; one typed key by key is read
            // on Add (a half-typed `#ms=` is not a link yet).
            if (pasted) openedAsLink(typed)
        }, words.t("add.placeholder"), error = failure, tag = "multisig.add.address")
        if (choosing) {
            MsNote(words.t("add.chooseNetwork"), tone = MsTone.Warning)
            val offered = MsRules.likelyNetworks(spaceNetworks, view.networkHints() + ("named" to if (fromLink) linkChains else emptyList<Int>())).ifEmpty { spaceNetworks }
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
            text = addPressLabel(words, retry = retry, failed = failure != null),
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
                    val outcome = view.importing.outcomes[a.lowercase()] ?: view.importing.outcomes[a]
                    // r15-8: one this import added says "Imported", never "Already added".
                    val here = view.row(a) != null
                    val deployed = safe["chains"].arr.any { it["deployed"].bool == true }
                    val status = importRowStatus(words, here, outcome)
                    val on = a in picked
                    // r14 leftovers: a read that did not come back (busy, unavailable,
                    // unverified) offers Try again — the core's `import.retry`.
                    val tryAgain = !here && view.importing.offersRetry(a)
                    Row(
                        Modifier.fillMaxWidth().toggleable(value = on || here, enabled = !here, role = Role.Checkbox) { if (on) picked.remove(a) else picked.add(a) }
                            .padding(vertical = VelaSpacing.sm).testTag("multisig.import.safe"),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            MsText(safe["name"].str ?: MultisigWords.short(a), MsType.Field)
                            MsText(listOfNotNull(MultisigWords.short(a), if (deployed) null else words.t("import.undeployed"), status).joinToString(" · "), MsType.Sub, color = colors.fgSubtle)
                        }
                        Switch(checked = on || here, onCheckedChange = null, enabled = !here, colors = SwitchDefaults.colors(checkedTrackColor = colors.fgBase))
                    }
                    if (tryAgain) {
                        MsTextButton(
                            words.t("review.state.tryAgain"),
                            enabled = view.importing.busy == null,
                            tag = "multisig.import.retry",
                        ) { hub.send(ev("import", "address" to a, "accept_risks" to false)) }
                    }
                }
                // r15-8: a Safe already here (imported a moment ago) is not counted
                // again — a press that would import nothing is never offered.
                val toImport = picked.filter { view.row(it) == null }
                VelaPrimaryButton(
                    text = words.t("import.importSome", "total" to toImport.size.toString()),
                    onClick = { toImport.forEach { hub.send(ev("import", "address" to it, "accept_risks" to false)) } },
                    modifier = Modifier.fillMaxWidth().testTag("multisig.import.go"),
                    enabled = toImport.isNotEmpty() && view.importing.busy == null,
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
fun MultisigLinkStandSheet(
    hub: MultisigHub,
    words: MultisigWords,
    onClose: () -> Unit,
    onAddNetwork: ((Int) -> Unit)?,
    /** r14-2: this sheet is still the one open — a late dismissal never closes the link that replaced it. */
    isCurrent: () -> Boolean = { true },
) {
    val view by hub.accountsView.collectAsStateWithLifecycle()
    val stand = view.link
    val close = {
        if (isCurrent()) {
            // The person closed the missing-network sheet: the waiting link is dropped (W9).
            if (stand?.kind == "missingNetwork") hub.send(ev("drop_link")) else hub.send(ev("close_link"))
            onClose()
        }
    }
    MsSheet(if (stand?.kind == "invite") words.t("add.title") else words.t("proposal.title"), words.raw("common.done"), onClose = close, tag = "multisig.link") {
        when (stand?.kind) {
            // A damaged link is said in words, never an empty note: the corpus's
            // sentence, and what the link held (the core's `text`) when it held anything.
            "broken" -> {
                MsNote(words.t("proposal.badLink"), tone = MsTone.Warning, tag = "multisig.link.broken")
                stand.text?.takeIf { it.isNotBlank() }?.let { MsText(it, MsType.Mono, color = VelaTheme.colors.fgMuted, maxLines = 3) }
            }
            "missingNetwork" -> {
                val chainId = stand.chainId ?: 0
                MsNote(words.missingTitle(chainId), tone = MsTone.Warning, tag = "multisig.link.missingNetwork")
                MsNote(words.t("proposal.missingNetworkBody"))
                onAddNetwork?.let { add -> VelaPrimaryButton(words.t("proposal.addNetwork"), { add(chainId) }, Modifier.fillMaxWidth().testTag("multisig.link.addNetwork")) }
            }
            "failed" -> {
                val reason = stand.reason
                val line = when (reason.kind) {
                    null, "notFound" -> words.t("proposal.notFound")
                    "unavailable" -> words.t("add.unavailable")
                    "busy" -> words.t("add.busy", "network" to words.network(reason["chain_id"].int ?: 0))
                    else -> words.t("add.${reason.kind}")
                }
                MsNote(line, tone = MsTone.Warning, tag = "multisig.link.failed")
                // §18 (c), round 14: a lookup that did not come back — the core offers Try again.
                if (stand.retry) {
                    MsTextButton(words.t("review.state.tryAgain"), tag = "multisig.link.retry") { hub.send(ev("retry_link")) }
                }
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
        // r9-12 (core §8): the invite the core composes (`invite_members`:
        // every member, its name where one is known, its kind) through the
        // link door's `addLink`; a core without the field: its inviteLink.
        if (r.inviteMembers.isNotEmpty()) {
            MsRules.addLink(origin, locale, r.account.address, r.account.chainIds, r.inviteMembers)
        } else {
            MsRules.inviteLink(
                origin, locale, r.account.address, r.account.owners, r.account.chainIds, proposals.membersAnywhere,
                inviteKnown(words, r.account, proposals.membersAnywhere),
            )
        }
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

/**
 * r9-12: what the invite says of each member (the core's `inviteLink`
 * `known`): the label the core's own rule gives it here (`names.label` —
 * what the multisigs keep, else this device's own account name), never one
 * picked in Kotlin, and the kind the record keeps. Every member of it, the
 * founding ones and those its chains name now — this device's own accounts
 * included, so the device that opens it shows them by name, as the web's
 * invite does (`invitedMembers`). A label that is only the short address is
 * not sent (the core writes no name for it).
 */
internal fun inviteKnown(words: MultisigWords, account: app.getvela.wallet.feature.multisig.core.MsAccount, now: List<String>): Map<String, Any?> {
    val out = linkedMapOf<String, Any?>()
    for ((key, info) in account.members) out[key.lowercase()] = mapOf("name" to info.name, "kind" to info.kind)
    for (member in (account.owners + now).distinctBy { it.lowercase() }) {
        val label = words.label(member).takeIf { it.isNotBlank() && it != MultisigWords.short(member) && it != MsRules.eip55(member) }
            ?: continue
        @Suppress("UNCHECKED_CAST")
        val kept = out[member.lowercase()] as? Map<String, Any?>
        out[member.lowercase()] = mapOf("name" to label, "kind" to kept?.get("kind"))
    }
    return out
}
