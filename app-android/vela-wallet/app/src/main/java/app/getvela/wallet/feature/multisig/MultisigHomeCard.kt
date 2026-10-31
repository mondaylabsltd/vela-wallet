package app.getvela.wallet.feature.multisig

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.multisig.core.MsAccountRow
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar

/**
 * The multisig's own lines on its home (the web's `MultisigHomeCard` and
 * Activity's waiting group, iOS's `MultisigHomeCard`):
 *
 * - its rule ("Needs 2 of 3") and what it is here (not shared yet, read-only);
 * - what waits for this device ("1 needs your approval · 1 ready"), then every
 *   open transaction with the core's one word for it, a tap away;
 * - the group's one note (offline, the service away, a chain unreachable, the
 *   last known list) with Retry where the core says one helps;
 * - once created here, its first steps — invite, add funds, what its first
 *   transfer out costs — until dismissed or it is set up.
 */
@Composable
fun MultisigHomeCard(
    hub: MultisigHub,
    row: MsAccountRow,
    words: MultisigWords,
    /** The multisig holds something (the balance read). */
    funded: Boolean,
    time: (Long) -> String,
    onOpen: (MultisigSheet) -> Unit,
    onFund: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val view by hub.proposalsView.collectAsStateWithLifecycle()
    val account = row.account
    val following = view.followed.equals(account.address, ignoreCase = true)
    val colors = VelaTheme.colors
    Column(modifier.fillMaxWidth().padding(vertical = VelaSpacing.lg).testTag("multisig.home"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
        // Header: the rule in force (each network's, read from the chain).
        Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            // The machine's `rule_chip` for the one it follows; the record's rule before.
            val chip = (if (following) words.ruleChip(view.ruleChip) else null) ?: words.ruleChip(account.threshold, account.owners.size)
            MsChip(chip, Modifier.testTag("multisig.home.rule"))
            if (!row.shared) MsChip(words.t("switcher.notShared"), tone = MsTone.Warning)
            if (row.highRiskOn.isNotEmpty()) MsChip(words.t("authority.riskFlag"), tone = MsTone.Danger)
        }
        if (row.localMembers.isEmpty()) {
            MsNote(words.t("panel.readOnly"), tag = "multisig.home.readOnly")
        } else if (!row.shared) {
            MsNote(words.t("panel.notShared"), tone = MsTone.Warning, tag = "multisig.home.notShared")
        }
        for (chainId in row.refusedOn) MsNote(words.t("authority.refusedOn", "network" to words.network(chainId)), tone = MsTone.Danger)
        MsNavRow(
            words.t("panel.members"),
            // The rule in force as the machine read it (`rule_chip`); the record's before.
            subtitle = view.ruleChip?.takeIf { following && it.kind == "needs" }?.get("rule")
                ?.let { r -> words.ruleTitle(r["m"].int ?: account.threshold, r["n"].int ?: account.owners.size) }
                ?: words.ruleTitle(account.threshold, account.owners.size),
            icon = VelaIcons.UsersRound,
            tag = "multisig.home.settings",
        ) { onOpen(MultisigSheet.Settings(account.address)) }

        // What waits for this device: the machine's `banner` (its parts and the one it opens).
        val banner = if (following) view.banner else null
        banner?.let { words.waitingParts(it["parts"].arr) }?.let { line ->
            Row(
                Modifier.fillMaxWidth().clickable(role = Role.Button) {
                    val chain = banner["first"]["chain_id"].int
                    val hash = banner["first"]["safe_tx_hash"].str
                    if (chain != null && hash != null) onOpen(MultisigSheet.Proposal(chain, hash))
                }.padding(vertical = VelaSpacing.sm).testTag("multisig.home.waiting"),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                MsText(line, MsType.RowTitle, Modifier.weight(1f))
                Icon(VelaIcons.ChevronRight, null, tint = colors.fgSubtle, modifier = Modifier.size(VelaIconSize.sm))
            }
        }
        if (following) {
            waitingNote(words, view.waitingNote, time)?.let { (text, retry) ->
                Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                    MsNote(text, Modifier.weight(1f), tone = MsTone.Warning, tag = "multisig.home.note")
                    if (retry) MsTextButton(words.t("service.retry"), tag = "multisig.home.retry") { hub.sendProposal(ev("retry")) }
                }
            }
        }
        val open = view.items.filter { it.status != "done" }
        if (following && open.isNotEmpty()) {
            MsSection(words.t("panel.pending")) {
                Column {
                    for (item in open) {
                        val standing = view.standing(item)
                        val title = words.actionTitle(item.chainId, item.action, item.nonce)
                        val status = "${words.standing(standing, item)} · ${words.network(item.chainId)}"
                        Row(
                            Modifier.fillMaxWidth().clickable(role = Role.Button) { onOpen(MultisigSheet.Proposal(item.chainId, item.safeTxHash)) }
                                .padding(vertical = VelaSpacing.lg).testTag("multisig.home.proposal")
                                .semantics(mergeDescendants = true) { contentDescription = "$title, $status" },
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
                        ) {
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                                MsText(title, MsType.Field, maxLines = 2)
                                MsText(status, MsType.Sub, color = if (standing.kind == "yourTurn") colors.fgBase else colors.fgSubtle, maxLines = 2)
                            }
                            MsText("${item.approved}/${item.threshold}", MsType.Label, color = colors.fgMuted)
                            Icon(VelaIcons.ChevronRight, null, tint = colors.fgSubtle, modifier = Modifier.size(VelaIconSize.sm))
                        }
                        MsDivider()
                    }
                }
            }
        }
        for (missing in view.missing) {
            val chainId = missing["chainId"].int ?: 0
            MsNote(
                "${words.missingTitle(chainId)} · ${words.t("proposal.missingCount", "n" to (missing["count"].int ?: 0).toString())}",
                tone = MsTone.Warning,
                tag = "multisig.home.missingNetwork",
            )
        }

        // First steps, until dismissed or set up.
        val setUp = following && view.setUp
        if (account.firstRun == true && !setUp) {
            Column(
                Modifier.fillMaxWidth().background(colors.bgSunken, RoundedCornerShape(VelaRadius.xl)).padding(VelaSpacing.xl),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                MsText(words.t("create.firstRunTitle"), MsType.RowTitle, Modifier.semantics { heading() })
                val network = account.chainIds.firstOrNull()
                if (account.owners.size > 1) {
                    FirstStep(words.t("create.firstRunInvite"), words.t("create.firstRunInviteNote"), row.inviteShared, "multisig.home.firstRun.invite", VelaIcons.QrCode) {
                        onOpen(MultisigSheet.Invite(account.address))
                    }
                }
                FirstStep(
                    words.t("create.firstRunFund"),
                    network?.let { words.t("create.firstRunFundNote", "network" to words.network(it)) },
                    funded, "multisig.home.firstRun.fund", onClick = onFund,
                )
                network?.let { MsNote(words.t("create.firstRunCost", "network" to words.network(it))) }
                MsTextButton(words.t("create.firstRunDismiss"), tag = "multisig.home.firstRun.dismiss") {
                    hub.send(ev("set_first_run", "address" to account.address, "on" to false))
                }
            }
        }
    }
}

@Composable
private fun FirstStep(title: String, note: String?, done: Boolean, tag: String, icon: androidx.compose.ui.graphics.vector.ImageVector = VelaIcons.ArrowDownLeft, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        Modifier.fillMaxWidth().clickable(role = Role.Button, onClick = onClick).padding(vertical = VelaSpacing.sm).testTag(tag)
            .semantics(mergeDescendants = true) { selected = done },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Icon(if (done) VelaIcons.Check else icon, null, tint = if (done) colors.successBase else colors.fgBase, modifier = Modifier.size(VelaIconSize.lg))
        Column(Modifier.weight(1f)) {
            MsText(title, MsType.Field)
            note?.let { MsText(it, MsType.Sub, color = colors.fgSubtle) }
        }
        Icon(VelaIcons.ChevronRight, null, tint = colors.fgSubtle, modifier = Modifier.size(VelaIconSize.sm))
    }
}

/** The switcher's multisig section: every multisig here, with its rule and what waits on it; Create and Add. */
@Composable
fun MultisigSwitcherSection(
    hub: MultisigHub,
    words: MultisigWords,
    onSelect: (String) -> Unit,
    onCreate: () -> Unit,
    onAdd: () -> Unit,
) {
    val accounts by hub.accountsView.collectAsStateWithLifecycle()
    val proposals by hub.proposalsView.collectAsStateWithLifecycle()
    val colors = VelaTheme.colors
    Column(Modifier.fillMaxWidth().testTag("multisig.switcher"), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
        MsText(words.t("switcher.section"), MsType.Label, Modifier.padding(top = VelaSpacing.xl).semantics { heading() }, color = colors.fgSubtle)
        for (row in accounts.accounts) {
            val selected = accounts.active.equals(row.account.address, ignoreCase = true)
            // The machine's `waiting_parts`: what waits here, in its words.
            val parts = proposals.waitingPartsOf(row.account.address)
            val count = parts.filter { it["kind"].str != "waitingPayer" }.sumOf { it["count"].int ?: 0 }
            val waitingWords = words.waitingParts(parts)
            val tags = buildList {
                add(words.ruleChip(row.account.threshold, row.account.owners.size))
                if (row.localMembers.isEmpty()) add(words.t("switcher.readOnly"))
                if (!row.shared) add(words.t("switcher.notShared"))
            }
            Row(
                Modifier.fillMaxWidth().clickable(role = Role.Button) { onSelect(row.account.address) }.padding(vertical = VelaSpacing.lg)
                    .testTag("multisig.switcher.row").semantics(mergeDescendants = true) { this.selected = selected },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                IdenticonAvatar(seed = row.account.address, size = VelaIconSize.xl * 1.5f, tappable = false)
                Column(Modifier.weight(1f)) {
                    MsText(words.name(row.account, row.name), MsType.RowTitle, color = colors.fgBase)
                    MsText(tags.joinToString(" · "), MsType.Sub, color = colors.fgSubtle)
                    MsText(MultisigWords.short(row.account.address), MsType.Mono, color = colors.fgSubtle)
                }
                if (count > 0) {
                    Box(
                        Modifier.background(colors.fgBase, RoundedCornerShape(VelaRadius.full)).padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs)
                            .semantics { contentDescription = waitingWords.orEmpty() },
                    ) { MsText(count.toString(), MsType.Label, color = colors.fgInverse) }
                }
                if (selected) Icon(VelaIcons.Check, null, tint = colors.fgBase, modifier = Modifier.size(VelaIconSize.lg))
            }
            MsDivider()
        }
        MsNavRow(words.t("switcher.create"), subtitle = words.t("switcher.createMultisigNote"), icon = VelaIcons.UsersRound, tag = "multisig.switcher.create", onClick = onCreate)
        MsNavRow(words.t("switcher.add"), subtitle = words.t("switcher.addMultisigNote"), icon = VelaIcons.Plus, tag = "multisig.switcher.add", onClick = onAdd)
    }
}
