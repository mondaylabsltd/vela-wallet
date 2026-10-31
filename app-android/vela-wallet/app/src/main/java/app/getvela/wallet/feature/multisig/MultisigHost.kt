package app.getvela.wallet.feature.multisig

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.flows.components.QrCard
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigOutsideSessionModel
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.str
import kotlinx.coroutines.delay
import kotlinx.serialization.json.JsonElement

/** What the host needs from the app around it. */
data class MultisigHostContext(
    val words: MultisigWords,
    val accounts: List<MsLocal>,
    val contacts: List<MsLocal>,
    val spaceNetworks: List<Int>,
    val otherSpaceName: String,
    val origin: String,
    val locale: String,
    val tokenInfo: suspend (Int, String) -> Pair<String, Int>,
    val explorerTx: (Int, String) -> String?,
    val onAddNetwork: (Int) -> Unit,
    val onFund: () -> Unit,
    /** The first time there is something to say (M6): ask once for notifications. */
    val askNotifications: () -> Unit,
)

/**
 * The multisig side of the app (the web's `MultisigHost.svelte`, iOS's
 * `MultisigHostModifier`): whichever multisig sheet is open — create, add, a
 * link where it stands, the invite, settings, members, a transaction, the
 * review before a proposal, a site's request — and over it the two sheets a
 * signature may wait on: the high-risk question and an outside member's
 * wallet. ONE sheet at a time; the two overlays come after it.
 */
@Composable
fun MultisigHost(hub: MultisigHub, context: MultisigHostContext) {
    val sheet by hub.sheet.collectAsStateWithLifecycle()
    val risk by hub.riskAsk.collectAsStateWithLifecycle()
    val outside by hub.outsideSession.collectAsStateWithLifecycle()
    val accounts by hub.accountsView.collectAsStateWithLifecycle()
    val words = context.words
    val close = { hub.sheet.value = null }
    when (val s = sheet) {
        null -> Unit
        MultisigSheet.Create -> MultisigCreateSheet(
            hub, words, context.accounts, context.contacts, context.spaceNetworks, context.otherSpaceName,
            onClose = close,
            onCreated = { close(); context.askNotifications() },
            onInvite = { hub.sheet.value = MultisigSheet.Invite(it) },
        )
        is MultisigSheet.Add -> MultisigAddSheet(
            hub, words, invite = null, prefill = s.prefill, spaceNetworks = context.spaceNetworks,
            onClose = close,
            onAdded = { close(); context.askNotifications() },
        )
        MultisigSheet.Link -> if (accounts.link?.kind == "invite") {
            MultisigAddSheet(
                hub, words, invite = accounts.link, prefill = null, spaceNetworks = context.spaceNetworks,
                onClose = { hub.send(app.getvela.wallet.feature.multisig.core.ev("close_link")); close() },
                onAdded = { hub.send(app.getvela.wallet.feature.multisig.core.ev("close_link")); close(); context.askNotifications() },
            )
        } else {
            MultisigLinkStandSheet(hub, words, onClose = close, onAddNetwork = context.onAddNetwork)
        }
        is MultisigSheet.Invite -> MultisigInviteSheet(hub, words, s.address, context.origin, context.locale, onClose = close)
        is MultisigSheet.Settings -> MultisigSettingsSheet(hub, words, s.address, onClose = close, onOpen = { hub.sheet.value = it }, onRemoved = close)
        is MultisigSheet.Members -> MultisigMembersSheet(hub, words, s.address, onClose = { hub.sheet.value = MultisigSheet.Settings(s.address) })
        is MultisigSheet.Proposal -> MultisigProposalSheet(
            hub, s.chainId, s.safeTxHash, words, context.origin, context.locale, context.tokenInfo, context.explorerTx,
            onClose = close, onAddNetwork = context.onAddNetwork,
        )
        is MultisigSheet.Review -> MultisigDraftReviewSheet(hub, words, context.tokenInfo)
        is MultisigSheet.Dapp -> MultisigDappSheet(hub, s.id, context)
    }
    risk?.let { MultisigRiskSheet(it, words) }
    if (risk == null) outside?.let { session ->
        MultisigOutsideSheet(session, words) {
            session.close()
            hub.outsideSession.value = null
        }
    }
}

/**
 * A site's request to a multisig (M5), held by the proposals machine: its
 * review first (the draft), then — once proposed — the transaction's own
 * sheet with what the site is told and when; a refusal said as the core
 * said it. The site is answered by the machine, once (the hub's settle).
 */
@Composable
fun MultisigDappSheet(hub: MultisigHub, id: String, context: MultisigHostContext) {
    val view by hub.proposalsView.collectAsStateWithLifecycle()
    val words = context.words
    val held = view.dapp[id]
    val host = held["origin"].str.orEmpty().let { o -> runCatching { java.net.URI(o).host }.getOrNull() ?: o }
    val leave = {
        hub.dappLeave(id)
        if (hub.sheet.value == MultisigSheet.Dapp(id)) hub.sheet.value = null
    }
    val hash = held["safe_tx_hash"].str
    when {
        !hash.isNullOrEmpty() -> MultisigProposalSheet(
            hub, held["chain_id"].int ?: 0, hash, words, context.origin, context.locale, context.tokenInfo, context.explorerTx,
            onClose = leave,
            siteNote = when {
                held["method"].str == "wallet_sendCalls" -> words.t("proposal.siteFollows", "host" to host)
                held["told"].bool == true -> words.t("proposal.siteTold", "host" to host)
                else -> words.t("proposal.siteWaits", "host" to host)
            },
        )
        held["stage"].str == "refused" -> MsSheet(words.t("proposal.title"), words.raw("common.done"), onClose = leave, tag = "multisig.dapp") {
            MsNote(words.t("proposal.siteAsks", "host" to host))
            MsNote(held["refusal"]["message"].str ?: words.t("action.unsupported"), tone = MsTone.Warning, tag = "multisig.dapp.refused")
        }
        view.draft != null -> MultisigDraftReviewSheet(hub, words, context.tokenInfo, siteNote = words.t("proposal.siteAsks", "host" to host), onWithdraw = leave)
        else -> MsSheet(words.t("proposal.title"), words.raw("common.done"), onClose = leave, tag = "multisig.dapp") {
            MsNote(words.t("proposal.siteAsks", "host" to host))
            Row(Modifier.testTag("multisig.dapp.opening"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                CircularProgressIndicator(color = VelaTheme.colors.fgMuted, strokeWidth = VelaSpacing.xs)
                MsText(words.t("proposal.opening"), MsType.Body, color = VelaTheme.colors.fgMuted)
            }
        }
    }
}

/**
 * An outside member's wallet asked to sign (M4) — the session machine's view:
 * the link (and code) the owner opens in a browser with its wallet, where it
 * stands, the relay, what went wrong and the way on.
 */
@Composable
fun MultisigOutsideSheet(session: MultisigOutsideSessionModel, words: MultisigWords, onClose: () -> Unit) {
    val view by session.view.collectAsStateWithLifecycle()
    val colors = VelaTheme.colors
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) { while (true) { delay(1_000); now = System.currentTimeMillis() } }
    MsSheet(words.t("external.sheetTitle"), words.raw("common.done"), onClose = onClose, tag = "multisig.outside") {
        val resumed = view["resumed"].bool == true
        val wallet = view["wallet"].str ?: words.t("external.ownerWallet")
        MsNote(if (resumed) words.t("external.resumeBody", "wallet" to wallet) else words.t("external.scanBody"))
        session.link?.let { link ->
            if (!resumed) QrCard(label = words.t("external.sheetTitle"), payload = link, modifier = Modifier.testTag("multisig.outside.qr"))
            MsText(link, MsType.Mono, Modifier.testTag("multisig.outside.link"), color = colors.fgMuted, maxLines = 4)
            ShareRow(words, link, "multisig.outside")
        }
        val phase = view["phase"].str ?: "waiting"
        val phaseText = when (phase) {
            "waiting" -> if (view["away"].bool == true) words.t("external.pageAway") else words.t("external.waiting")
            "connecting" -> words.t("external.connecting")
            "wrongAccount" -> words.t("external.wrongAccount", "account" to MultisigWords.short(view["accounts"].arr.firstOrNull().str.orEmpty()))
            "switching" -> words.t("external.switching", "chain" to view["chain"].str.orEmpty())
            "confirming" -> {
                val since = view["since"].long ?: 0
                val elapsed = if (since > 0) "${((now - since) / 1000).coerceAtLeast(0)} s" else ""
                words.t("external.inWallet", "wallet" to wallet, "elapsed" to elapsed)
            }
            else -> null
        }
        phaseText?.let {
            Row(Modifier.fillMaxWidth().testTag("multisig.outside.phase"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                if (phase != "done") CircularProgressIndicator(color = colors.fgMuted, strokeWidth = VelaSpacing.xs)
                MsText(it, MsType.Body)
            }
        }
        view["relay"]?.takeIf { it.kind == "retrying" || it.kind == "failed" }?.let { relay ->
            val reason = if (relay["why"].str == "refused") words.t("external.relayRefused") else relay["detail"].str.orEmpty()
            Row(verticalAlignment = Alignment.Top) {
                MsNote(
                    if (relay.kind == "failed") words.t("external.relayFailed", "reason" to reason) else words.t("external.relayUnreachable", "reason" to reason),
                    Modifier.weight(1f), tone = MsTone.Warning, tag = "multisig.outside.relay",
                )
                MsTextButton(words.t("external.retry")) { session.retryRelay() }
            }
        }
        val failure: JsonElement? = view["failure"]
        failure?.let { f ->
            val text = when (f.kind) {
                "connectDeclined" -> words.t("external.connectDeclined")
                "walletDeclined" -> words.t("external.declined")
                "otherChain" -> words.t("external.otherChain", "chain" to view["chain"].str.orEmpty())
                "badSignature" -> words.t("external.badSignature")
                "stoppedWaiting" -> words.t("external.stoppedWaiting")
                "noRoom" -> words.t("external.relayFailed", "reason" to f["reason"].str.orEmpty())
                else -> words.t("external.stillWaiting", "wallet" to wallet)
            }
            MsNote(text, tone = MsTone.Warning, tag = "multisig.outside.failure")
            VelaPrimaryButton(words.t("external.askAgain"), { session.askAgain() }, Modifier.fillMaxWidth().testTag("multisig.outside.askAgain"))
        }
        if (view["outcome"].kind == "approved") {
            MsNote(words.t("external.signedBy", "owner" to words.label(view["pairedAccount"].str.orEmpty())), tone = MsTone.Positive, tag = "multisig.outside.approved")
        }
        if (phase == "confirming") VelaSecondaryButton(words.t("external.stopWaiting"), { session.stopWaiting() }, Modifier.fillMaxWidth())
        if (view["pairedAccount"].str != null && phase != "done") MsTextButton(words.t("external.unpair")) { session.anotherWallet(unpair = true) }
    }
}
