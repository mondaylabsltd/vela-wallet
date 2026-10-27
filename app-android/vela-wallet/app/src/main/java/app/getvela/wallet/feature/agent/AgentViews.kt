package app.getvela.wallet.feature.agent

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.SettingsTone
import app.getvela.wallet.feature.settings.components.VelaSwitchRow
import app.getvela.wallet.feature.settings.components.VelaUrlField
import kotlinx.coroutines.delay
import org.json.JSONObject
import org.json.JSONTokener
import uniffi.vela_core_uniffi.AgentLink
import uniffi.vela_core_uniffi.clientmcpAccountUrl
import uniffi.vela_core_uniffi.intentApprovalDetailIn

/** The agent's words, as the person reads them — the core catalog's `settings.agent.*`. */
object AgentWords {
    /** What an action is called: `settings.agent.action.<camelCase type>`, or its own name where the catalog has none. */
    fun action(kind: String, strings: VelaStrings): String {
        val parts = kind.split('_')
        val camel = parts.first() + parts.drop(1).joinToString("") { it.replaceFirstChar(Char::uppercaseChar) }
        val key = "settings.agent.action.$camel"
        return strings.t(key).takeIf { it != key } ?: kind
    }

    /** How the connection stands. */
    fun link(link: AgentLink, strings: VelaStrings): String = strings.t(
        when (link) {
            AgentLink.Off -> "settings.agent.statusOff"
            AgentLink.Connecting -> "settings.agent.statusConnecting"
            AgentLink.Online -> "settings.agent.statusOnline"
            AgentLink.Offline -> "settings.agent.statusOffline"
            is AgentLink.Closed -> "settings.agent.statusRefused"
        },
    )

    /**
     * What a request names, as the person knows it: a contact's or a wallet's
     * name, the store, the preference — and, the core's, the network, URL or
     * record it changes (`intentApprovalDetail`).
     */
    fun detail(request: AgentRequest, wallet: AgentWallet, strings: VelaStrings): String? {
        val action = request.action
        return when (request.kind) {
            "delete_contact" -> {
                val address = action.optString("address")
                wallet.contacts.view.value.contacts.firstOrNull { it.address.equals(address, ignoreCase = true) }
                    ?.name?.takeIf { it.isNotBlank() } ?: address
            }
            "delete_group" -> {
                val id = action.optString("id")
                wallet.contacts.view.value.groups.firstOrNull { it.id == id }?.name ?: id
            }
            "remove_wallet" -> {
                val address = action.optString("address")
                wallet.session.view.value.accounts.firstOrNull { it.address.equals(address, ignoreCase = true) }
                    ?.let { "${it.name} · $address" } ?: address
            }
            "clear_storage_item" -> action.optString("item")
            "set_preference" -> action.optJSONObject("preference")?.let { "${it.optString("kind")}: ${it.opt("value")}" }
            "set_balance_privacy" -> strings.t(if (action.optBoolean("hidden")) "home.a11yHideBalance" else "home.a11yShowBalance")
            // Spec 105: the space it switches to, by its own name.
            "set_network_space" -> strings.t(
                if (action.optString("space") == "testnet") I18nKeys.NetworkSpace.TESTNET else I18nKeys.NetworkSpace.MAINNET,
            )
            // The core's: the network, URL or record it changes — and, read
            // against the wallet, a site's networks and wallet, or the wallet
            // a switch leaves (spec 107).
            else -> runCatching {
                JSONTokener(intentApprovalDetailIn(action.toString(), AgentWorld.json(wallet))).nextValue() as? String
            }.getOrNull()
        }
    }
}

/** The agent, for the screens that draw its state deep in their trees (Settings → AI agent). */
val LocalAgentStore = androidx.compose.runtime.staticCompositionLocalOf<AgentStore?> { null }

/** How high the capsule floats over the bottom: clear of the tab bar. */
private val CapsuleClearance = 84.dp

/**
 * The agent's capsule, floating over every screen as the web's floats over
 * its page (spec 103): whether it is connected — a dot in the link's tone —
 * what it did last, and Stop, which refuses everything of its still waiting
 * and switches it off. Shown while the agent is set up, or a request waits on
 * the person. Low on the screen, above the tab bar: the headers are the
 * screens' own. On a wallet flow the foot of the page is the flow's — its
 * button and the note above it (Confirm & Send, "First time sending here") —
 * so there it is smaller, in the header's empty corner beside the back arrow.
 */
@Composable
fun AgentCapsule(agent: AgentStore, modifier: Modifier = Modifier) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val config by agent.config.collectAsStateWithLifecycle()
    val link by agent.link.collectAsStateWithLifecycle()
    val approvals by agent.approvals.collectAsStateWithLifecycle()
    val activity by agent.activity.collectAsStateWithLifecycle()
    val flowOpen by agent.flowOpen.collectAsStateWithLifecycle()
    if (!config.ready && approvals.isEmpty()) return
    val latest = activity.firstOrNull()
    val dot = when (link) {
        AgentLink.Online -> if (latest?.stage != null) colors.infoBase else colors.successBase
        is AgentLink.Closed -> colors.errorBase
        else -> colors.warningBase
    }
    // Online, what it did last; otherwise how the link stands.
    val what = when (link) {
        AgentLink.Online -> latest?.let { AgentWords.action(it.kind, strings) } ?: strings.t("settings.agent.statusOnline")
        AgentLink.Off -> strings.t("settings.agent.statusConnecting")
        else -> AgentWords.link(link, strings)
    }
    Box(
        modifier = modifier
            .fillMaxSize()
            .statusBarsPadding()
            .navigationBarsPadding()
            .padding(
                // Level with the back arrow, below the parallel space's badge.
                top = if (flowOpen) 30.dp else 0.dp,
                end = if (flowOpen) VelaSpacing.md else VelaSpacing.lg,
                start = VelaSpacing.lg,
                bottom = if (flowOpen) 0.dp else CapsuleClearance,
            ),
        contentAlignment = if (flowOpen) Alignment.TopEnd else Alignment.BottomCenter,
    ) {
        Row(
            modifier = Modifier
                .widthIn(max = 520.dp)
                .shadow(12.dp, CircleShape)
                .clip(CircleShape)
                .background(colors.bgRaised)
                .border(VelaBorder.hairline, colors.borderBase, CircleShape)
                .padding(start = VelaSpacing.lg, end = VelaSpacing.xs, top = VelaSpacing.xs, bottom = VelaSpacing.xs)
                .testTag("agent-capsule"),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Box(modifier = Modifier.size(8.dp).clip(CircleShape).background(dot))
            Text(
                text = strings.t("settings.agent.title"),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
            )
            if (!flowOpen) {
                Text(
                    text = what,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
            }
            Box(
                modifier = Modifier
                    .height(28.dp)
                    .clip(CircleShape)
                    .background(colors.errorSoft)
                    .clickable { agent.stop() }
                    .padding(horizontal = VelaSpacing.md)
                    .testTag("agent-stop"),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = strings.t("settings.agent.stop"),
                    color = colors.errorBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
    }
}

/** How long a request is on screen before Allow takes a tap. */
private const val ARM_MS = 700L

/**
 * The agent's manage-level request, asked here in the wallet — not in the
 * agent's chat. Dismissing it is a no.
 *
 * One dialog per request (`key`): the next request is a new dialog, never the
 * same one with its words changed under a finger on Allow — and its Allow
 * takes a tap only once it has been on screen a moment (`ARM_MS`).
 */
@Composable
fun AgentApprovalDialog(agent: AgentStore) {
    val strings = LocalVelaStrings.current
    val approvals by agent.approvals.collectAsStateWithLifecycle()
    var answered by remember { mutableStateOf<ULong?>(null) }
    val request = approvals.firstOrNull()?.takeIf { it.id != answered } ?: return
    key(request.id) {
        var armed by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) {
            delay(ARM_MS)
            armed = true
        }
        val answer = { approved: Boolean ->
            answered = request.id
            agent.answer(request.id, approved)
        }
        val name = AgentWords.action(request.kind, strings)
        val what = AgentWords.detail(request, agent.wallet, strings)
        val hint = strings.t("settings.agent.approveHint")
        AgentApprovalAlert(
            strings = strings,
            text = if (what != null) "$name — $what. $hint" else "$name. $hint",
            armed = armed,
            answer = answer,
        )
    }
}

@Composable
private fun AgentApprovalAlert(strings: VelaStrings, text: String, armed: Boolean, answer: (Boolean) -> Unit) {
    AlertDialog(
        onDismissRequest = { answer(false) },
        title = { Text(strings.t("settings.agent.approveTitle")) },
        text = { Text(text) },
        // The safe answer is the easy one: Deny where a dialog's answer sits,
        // in the accent; Allow beside it, quiet. Asked to tap Deny, the owner
        // twice hit the red Allow that used to sit there (2026-10-27).
        confirmButton = {
            TextButton(onClick = { answer(false) }, modifier = Modifier.testTag("agent-deny")) {
                Text(strings.t("settings.agent.deny"), color = VelaTheme.colors.accentBase, fontWeight = FontWeight.SemiBold)
            }
        },
        dismissButton = {
            TextButton(onClick = { answer(true) }, enabled = armed, modifier = Modifier.testTag("agent-allow")) {
                Text(strings.t("settings.agent.allow"), color = VelaTheme.colors.fgMuted)
            }
        },
    )
}

/**
 * Spec 106: the MCP server address for the person's AI agent, to copy from
 * here — masked until Show: it is a credential.
 */
@Composable
private fun AgentAddress(url: String) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val clipboard = LocalClipboardManager.current
    val context = LocalContext.current
    var shown by remember { mutableStateOf(false) }
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) {
        if (copied) {
            delay(1800)
            copied = false
        }
    }
    val masked = url.lastIndexOf('/').let { at -> if (at < 0) "••••••••••••" else url.substring(0, at + 1) + "••••••••••••" }
    val action = @Composable { label: String, tag: String, onClick: () -> Unit ->
        Text(
            text = label,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = FontWeight.SemiBold,
            fontSize = VelaTextSize.sm,
            modifier = Modifier
                .clip(RoundedCornerShape(VelaRadius.full))
                .clickable(onClick = onClick)
                .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.sm)
                .testTag(tag),
        )
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            .padding(VelaSpacing.lg)
            .testTag("agent-address"),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Text(
            text = strings.t("settings.agent.agentAddress"),
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
        )
        Text(
            text = if (shown) url else masked,
            color = colors.fgBase,
            fontFamily = VelaMonoFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.testTag("agent-address-value"),
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xs, Alignment.End),
        ) {
            action(strings.t(if (shown) "settings.agent.hide" else "settings.agent.show"), "agent-address-show") { shown = !shown }
            action(strings.t("settings.agent.share"), "agent-address-share") {
                val send = android.content.Intent(android.content.Intent.ACTION_SEND)
                    .setType("text/plain")
                    .putExtra(android.content.Intent.EXTRA_TEXT, url)
                runCatching { context.startActivity(android.content.Intent.createChooser(send, null)) }
            }
            action(strings.t(if (copied) "settings.agent.copied" else "settings.agent.copy"), "agent-address-copy") {
                clipboard.setText(AnnotatedString(url))
                copied = true
            }
        }
        Text(
            text = strings.t("settings.agent.agentAddressHint"),
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
        )
    }
}

/**
 * Spec 107 E8: a switch per wallet on this device; the agent sees and acts
 * only in the ones on. Each change is the person's own `configure_agent`,
 * the list whole.
 */
@Composable
private fun AgentWalletsSection(agent: AgentStore, allowed: List<String>) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val session by agent.wallet.session.view.collectAsStateWithLifecycle()
    Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.md), modifier = Modifier.testTag("agent-wallets")) {
        Text(
            text = strings.t("settings.agent.wallets"),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = FontWeight.SemiBold,
            fontSize = VelaTextSize.base,
        )
        Text(
            text = strings.t("settings.agent.walletsHint"),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
        )
        if (allowed.isEmpty()) {
            Text(
                text = strings.t("settings.agent.walletsNone"),
                color = colors.warningBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.testTag("agent-wallets-none"),
            )
        }
        session.accounts.forEach { row ->
            val address = row.address
            val on = allowed.any { it.equals(address, ignoreCase = true) }
            VelaSwitchRow(
                title = row.name,
                body = if (address.length > 12) "${address.take(6)}…${address.takeLast(4)}" else address,
                checked = on,
                onCheckedChange = { want ->
                    val next = allowed.filterNot { it.equals(address, ignoreCase = true) } + listOfNotNull(address.takeIf { want })
                    agent.person(JSONObject().put("type", "configure_agent").put("wallets", org.json.JSONArray(next)))
                },
                modifier = Modifier.testTag("agent-wallet-$address"),
            )
        }
    }
}

/** Why connecting with clientmcp.dev did not work, in the person's words. */
internal fun linkProblemWords(problem: LinkProblem, strings: VelaStrings): String = when (problem) {
    is LinkProblem.Refused -> strings.t("settings.agent.linkRefused").replace("{{reason}}", problem.reason)
    LinkProblem.Unavailable -> strings.t("settings.agent.linkUnavailable")
    LinkProblem.Expired -> strings.t("settings.agent.linkExpired")
    LinkProblem.NotOurs -> strings.t("settings.agent.linkNotOurs")
    LinkProblem.Network -> strings.t("settings.agent.linkNetwork")
    LinkProblem.Failed -> strings.t("settings.agent.linkFailed")
}

/**
 * Settings → AI agent: whether this wallet answers a remote AI agent through
 * a ClientMCP bridge, and which bridge (spec 103 D1). Every change is the
 * person's own, through the `configure_agent` intent — an agent can never
 * make it.
 */
@Composable
fun AgentSettingsBody(agent: AgentStore, modifier: Modifier = Modifier) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val config by agent.config.collectAsStateWithLifecycle()
    val link by agent.link.collectAsStateWithLifecycle()
    val activity by agent.activity.collectAsStateWithLifecycle()
    var url by remember { mutableStateOf<String?>(null) }
    var token by remember { mutableStateOf<String?>(null) }
    var urlFocused by remember { mutableStateOf(false) }
    var tokenFocused by remember { mutableStateOf(false) }
    val clipboard = LocalClipboardManager.current
    val uris = LocalUriHandler.current
    val linking by agent.clientmcp.linking.collectAsStateWithLifecycle()
    val problem by agent.clientmcp.problem.collectAsStateWithLifecycle()
    // "Use your own bridge" unfolded by the person (`null`: unfolded when a token was entered by hand).
    var ownOpen by remember { mutableStateOf<Boolean?>(null) }
    val configure = { change: JSONObject -> agent.person(change.put("type", "configure_agent")) }
    Column(
        modifier = modifier.fillMaxWidth().padding(bottom = CapsuleClearance),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
    ) {
        Text(
            text = strings.t("settings.agent.description"),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        val enableRow = @Composable {
            VelaSwitchRow(
                title = strings.t("settings.agent.enable"),
                body = AgentWords.link(link, strings),
                checked = config.enabled,
                onCheckedChange = { on -> configure(JSONObject().put("enabled", on)) },
                modifier = Modifier.testTag("agent-enable"),
            )
        }
        val problemLine = @Composable {
            problem?.let {
                Text(
                    text = linkProblemWords(it, strings),
                    color = colors.errorBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    modifier = Modifier.testTag("agent-link-problem"),
                )
            }
        }
        val linked = config.linked
        // Spec 106: connected through clientmcp.dev — whose it is, where the
        // person connects their agent, Reconnect and Disconnect.
        if (linked != null) {
            Text(
                text = strings.t("settings.agent.connectedAs").replace("{{login}}", linked.login),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = FontWeight.SemiBold,
                fontSize = VelaTextSize.lg,
                modifier = Modifier.testTag("agent-linked"),
            )
            Text(
                text = strings.t("settings.agent.clientId").replace("{{id}}", linked.clientId),
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.testTag("agent-client-id"),
            )
            if (config.agentUrl.isNotEmpty()) AgentAddress(config.agentUrl)
            enableRow()
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(VelaRadius.lg))
                    .background(colors.bgSunken)
                    .clickable { uris.openUri(clientmcpAccountUrl(linked.site)) }
                    .padding(VelaSpacing.lg)
                    .testTag("agent-connect-agent"),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
            ) {
                Text(
                    text = "${strings.t("settings.agent.connectAgent")} →",
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = VelaTextSize.base,
                )
                Text(
                    text = strings.t("settings.agent.connectAgentHint"),
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                VelaSecondaryButton(
                    text = strings.t("settings.agent.reconnect"),
                    onClick = { agent.clientmcp.start() },
                    loading = linking,
                    modifier = Modifier.weight(1f).testTag("agent-reconnect"),
                )
                VelaSecondaryButton(
                    text = strings.t("settings.agent.disconnect"),
                    // Forgotten here; the client stays on clientmcp.dev until deleted there.
                    onClick = {
                        configure(JSONObject().put("enabled", false).put("bridge_url", "").put("client_token", ""))
                    },
                    modifier = Modifier.weight(1f).testTag("agent-disconnect"),
                )
            }
            problemLine()
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                VelaSecondaryButton(
                    text = strings.t("settings.agent.connect"),
                    onClick = { agent.clientmcp.start() },
                    loading = linking,
                    busyLabel = true,
                    modifier = Modifier.fillMaxWidth().testTag("agent-connect"),
                )
                if (linking) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = strings.t("settings.agent.connecting"),
                            color = colors.fgMuted,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.sm,
                        )
                        Text(
                            text = strings.t("common.cancel"),
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.sm,
                            fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.clickable { agent.clientmcp.cancel() }.testTag("agent-link-cancel"),
                        )
                    }
                } else {
                    Text(
                        text = strings.t("settings.agent.connectHint"),
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                }
                problemLine()
            }
            val open = ownOpen ?: config.token.isNotEmpty()
            Text(
                text = "${if (open) "▾" else "▸"}  ${strings.t("settings.agent.ownBridge")}",
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.clickable { ownOpen = !open }.testTag("agent-own-bridge"),
            )
            if (open) {
        enableRow()
        VelaUrlField(
            label = strings.t("settings.agent.bridge"),
            value = url ?: config.url,
            placeholder = "wss://bridge.clientmcp.dev/connect",
            hint = strings.t(if (config.urlProblem == null) "settings.agent.bridgeHint" else "settings.agent.bridgeInvalid"),
            tone = if (config.urlProblem == null) null else SettingsTone.Error,
            onValueChange = { url = it },
            modifier = Modifier
                .testTag("agent-bridge")
                .onFocusChanged { state ->
                    if (urlFocused && !state.isFocused) url?.let { configure(JSONObject().put("bridge_url", it)) }
                    urlFocused = state.isFocused
                },
        )
        // The client token: a credential, so it is not shown once entered.
        Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            Text(
                text = strings.t("settings.agent.token"),
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
            BasicTextField(
                value = token ?: config.token,
                onValueChange = { token = it },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                textStyle = TextStyle(color = colors.fgBase, fontFamily = VelaMonoFontFamily, fontSize = VelaTextSize.base),
                cursorBrush = SolidColor(colors.accentBase),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = VelaSizing.controlMd)
                    .clip(RoundedCornerShape(VelaRadius.lg))
                    .background(colors.bgSunken)
                    .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.lg))
                    .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.lg)
                    .testTag("agent-token")
                    .onFocusChanged { state ->
                        if (tokenFocused && !state.isFocused) token?.let { configure(JSONObject().put("client_token", it)) }
                        tokenFocused = state.isFocused
                    },
            )
            Text(
                text = strings.t("settings.agent.tokenHint"),
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
        config.endpoint?.let { endpoint ->
            Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
                Text(
                    text = strings.t("settings.agent.endpoint"),
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
                Text(
                    text = endpoint,
                    color = colors.fgBase,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(VelaRadius.lg))
                        .background(colors.bgSunken)
                        .clickable { clipboard.setText(AnnotatedString(endpoint)) }
                        .padding(VelaSpacing.lg),
                )
                Text(
                    text = strings.t("settings.agent.endpointHint"),
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
        }
            }
        }
        AgentWalletsSection(agent, config.wallets)
        activity.firstOrNull()?.let { latest ->
            Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(
                    text = strings.t("settings.agent.lastAction"),
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                )
                Text(
                    text = "${AgentWords.action(latest.kind, strings)} · ${latest.code ?: latest.status ?: latest.stage.orEmpty()}",
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
    }
}
