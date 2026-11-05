package app.getvela.wallet.feature.multisig

import app.getvela.wallet.AppContainer
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigLive
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MultisigOutsideSessionModel
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.SpeedControl
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.wallet.core.Abi
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import uniffi.vela_core_uniffi.multisigApprovalRoute

/**
 * The multisig, wired into the app (native multisig M1–M6): the one hub over
 * the app's pipelines — the pool, the relay, the account store, its own
 * submit spine and fee session, the one trusted page — and the facts it is
 * fed (iOS's `AppGraph` + `MultisigFacts`). Read, never decided: what a fact
 * means is the machines'.
 */
object MultisigWiring {

    /** The core's default multisig service (`DEFAULT_GENESIS_SERVICE_URL`). */
    const val DEFAULT_SERVICE = "https://safe-genesis.getvela.app"

    /** The relay key a debug profile may point at (`vela.peerconnectRelay`). */
    const val RELAY_KEY = "vela.peerconnectRelay"

    fun build(container: AppContainer, application: android.app.Application): MultisigHub {
        val store = VelaStore(application)
        val accountPort = StoreAccountPort(container.accountStore)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        // The multisig's own fee session: a quote here never supersedes the send screen's.
        val feeExecutor = FeeExecutor(
            relay = container.relay,
            keyHexes = { address -> accountPort.keysOf(address).map { it.publicKeyHex } },
            measureCall = { chainId, from, to, valueHex, data ->
                (container.pool.call(chainId, "eth_estimateGas", listOf(JSONObject().put("from", from).put("to", to).put("value", valueHex).put("data", data))) as? RpcResult.Body)
                    ?.json?.takeIf { it.has("result") && !it.isNull("result") }?.optString("result")?.takeIf { it.startsWith("0x") }
            },
        )
        val fees by lazy {
            SpeedControl(
                scope, container.relay, feeExecutor,
                preferredTier = { container.settings.feeTier.value.tier },
                numberPreset = { app.getvela.wallet.core.format.Formats.current.resolvedNumber().wire },
                area = "multisig",
            ).start()
        }
        // The submit spine an execution and a member's passkey go through — the
        // send's own, with its own instance (the page's hand-off is the
        // proposal's sheet, so no second card).
        val spine = UserOpSpine(
            relay = container.relay,
            accounts = accountPort,
            signer = {
                app.getvela.wallet.dev.ParallelSpaceHook.signer()
                    ?: container.passkeySigner
                    ?: error("no signer attached: a multisig signature before an activity attached the passkey ceremony")
            },
            measureCall = { chainId, from, to, valueHex, data ->
                (container.pool.call(chainId, "eth_estimateGas", listOf(JSONObject().put("from", from).put("to", to).put("value", valueHex).put("data", data))) as? RpcResult.Body)
                    ?.json?.takeIf { it.has("result") && !it.isNull("result") }?.optString("result")?.takeIf { it.startsWith("0x") }
            },
            trustedSigner = { container.trustedSigner },
            handoffShown = true,
            words = { key, vars -> container.i18nRuntime.t(key, vars) },
        )
        // The hub's machines open no room of their own today (the web hands them
        // rooms whose news goes nowhere); a session gets its own below.
        val peer = app.getvela.wallet.feature.multisig.core.MultisigPeerConnect(scope, store)
        // The hub, once built: the shell's facts read whether an address is one of its multisigs.
        var hubRef: MultisigHub? = null
        val live = MultisigLive(
            pool = container.pool,
            relay = container.relay,
            spine = spine,
            accountStore = container.accountStore,
            store = store,
            fees = { fees },
            trustedSigner = { container.trustedSigner },
            chainLabel = { chainId ->
                val row = container.settings.networks.value.networks.firstOrNull { it.chain_id.toInt() == chainId }
                row?.display_name to row?.native_symbol
            },
            sentTo = { address -> container.wallet.feedExecutor.sentTo(address) },
            own = { address ->
                container.session.view.value.accounts.any { it.address.equals(address, ignoreCase = true) } ||
                    hubRef?.isMultisig(address) == true
            },
            peer = peer,
        )
        val hub = MultisigHub(
            live = live,
            facts = { facts(container, store, accountPort) },
            writeRows = { rows -> container.wallet.feedExecutor.writeRecords(rows) },
            tokenInfo = { chainId, token -> tokenInfo(container, chainId, token, hubRef) },
            scope = scope,
        )
        hub.titleOf = { chainId, action, nonce ->
            val token = action?.let { a -> (a as? kotlinx.serialization.json.JsonObject)?.get("token") }
                ?.let { (it as? kotlinx.serialization.json.JsonPrimitive)?.content }
            val info = if (token != null) tokenInfo(container, chainId, token, hubRef) else null
            // The members named as the accounts machine names them (r12-8).
            words(container, hubRef).actionTitle(chainId, action, nonce, info)
        }
        hub.onRecorded = {
            container.wallet.feedReconciled(1)
            // r13 S4: an execution landed (this app's own, or a member's) — the multisig's
            // balances moved, read now (never left to the next poll), and the
            // scan looks for what came back (the feed's `sent`).
            container.wallet.multisigExecuted()
        }
        hubRef = hub
        // r13-5 (r12-8): the executed member changes in Activity are worded in
        // the language and with the names that stand — again whenever either
        // moves (the web's and iOS's `rewordMemberRows`).
        scope.launch {
            kotlinx.coroutines.flow.combine(
                container.i18nRuntime.state.map { it.ready to it.language },
                hub.accountsView.map { it.loaded to it.memberLabels },
            ) { language, labels -> language to labels }
                .distinctUntilChanged()
                .collect { (language, labels) ->
                    if (!language.first || !labels.first) return@collect
                    rewordMemberRows(container, hub)
                }
        }
        // r9-12 (core §8): the passkey index's word on each member the accounts
        // machine asks about (`index_wanted`, this device's own accounts too) —
        // the name that account registered, the same on every device. The
        // lookup is the core's walk (`registry_lookup`); an address nobody
        // answered for is left out, asked again when the list moves.
        scope.launch {
            hub.accountsView.map { it.indexWanted }.distinctUntilChanged().collect { wanted ->
                if (wanted.isEmpty()) return@collect
                val answers = linkedMapOf<String, Any?>()
                for (address in wanted) {
                    val verdict = runCatching { container.registryNames.verdictFor(address) }.getOrNull() ?: continue
                    answers[address.lowercase()] = mapOf("vela" to verdict.vela, "name" to verdict.name)
                }
                VelaLog.event("multisig.names", "index", "asked" to wanted.size, "answered" to answers.size)
                if (answers.isNotEmpty()) hub.send(app.getvela.wallet.feature.multisig.core.ev("index_names", "answers" to answers))
            }
        }
        hub.openOutsideSession = { request, onOutcome ->
            MultisigOutsideSessionModel(
                request = request,
                ports = live,
                rooms = app.getvela.wallet.feature.multisig.core.MultisigPeerConnect(scope, store),
                relayUrl = runCatching { kotlinx.coroutines.runBlocking { store.read(RELAY_KEY) } }.getOrNull(),
                serviceUrl = runCatching { kotlinx.coroutines.runBlocking { serviceUrl(container, store) } }.getOrDefault(DEFAULT_SERVICE),
                connectPage = connectPage(container),
                scope = scope,
                onOutcome = onOutcome,
            )
        }
        return hub
    }

    /**
     * M6: the notifier keeps up with the views while the app is up (said by
     * the screens, recorded so a background round says only what is new);
     * an execution while away is said; the poll is scheduled while there is
     * a multisig to follow.
     */
    fun attachNotifier(container: AppContainer, application: android.app.Application, hub: MultisigHub) {
        var last = ""
        hub.onProposalsMoved = { accounts, proposals ->
            // r11-1: decided as the view lands — on screen it is seen; away
            // (after a round began in this process) a read landing late is
            // said now, measured from that round; in between, neither.
            val onScreen = container.onScreen()
            if (onScreen) hub.awaySince = null
            val look = MultisigNotifier.lookNow(onScreen = onScreen, started = container.inForeground(), since = hub.awaySince)
            val signature = proposals.presence.toString() + proposals.items.joinToString { "${it.id}=${it.approved}" }
            if (look != null && signature != last) {
                last = signature
                hub.scope.launch {
                    val notes = container.multisigNotifier.evaluate(accounts, proposals, look)
                    if (look is MultisigNotifier.Look.Away && notes.isNotEmpty()) {
                        VelaLog.event("multisig.poll", "said as it landed", "notes" to notes.size)
                    }
                }
            }
        }
        hub.onExecuted = { title, chainId, hash -> container.multisigNotifier.executed(title, chainId, hash, away = !container.inForeground()) }
        hub.scope.launch {
            hub.accountsView.map { it.loaded to it.accounts.isNotEmpty() }.distinctUntilChanged().collect { (loaded, any) ->
                if (loaded) MultisigNotifier.schedule(application, on = any)
            }
        }
    }

    /**
     * The send machine's `propose_safe_tx`: the reviewed transfers, proposed by
     * the proposals machine (its review first, then one passkey) — answered
     * `proposed` / `propose_failed` in the send's own words.
     */
    suspend fun proposeFromSend(hub: MultisigHub, op: app.getvela.wallet.feature.send.core.SendOperation.ProposeSafeTx): app.getvela.wallet.feature.send.core.SendShellResult {
        hub.boot()
        val transfers = op.transfers.map { mapOf("token" to it.token, "to" to it.to, "amount" to it.amount) }
        return when (val outcome = hub.proposeFromSend(op.chain_id, op.multisig, transfers)) {
            is MultisigHub.ProposeOutcome.Proposed -> app.getvela.wallet.feature.send.core.SendShellResult.Proposed(outcome.safeTxHash)
            is MultisigHub.ProposeOutcome.Failed -> app.getvela.wallet.feature.send.core.SendShellResult.ProposeFailed(
                cancelled = outcome.cancelled,
                reason = when (outcome.reason) {
                    "mailbox_down" -> app.getvela.wallet.feature.send.core.SendProposeFailure.MailboxDown
                    "offline" -> app.getvela.wallet.feature.send.core.SendProposeFailure.Offline
                    else -> null
                },
            )
        }
    }

    /**
     * r13-5: every executed member change in Activity worded again in the
     * language in front and with the names as they stand ([MemberRowTitles]);
     * the feed re-reads when any row moved. Answers how many moved.
     */
    suspend fun rewordMemberRows(container: AppContainer, hub: MultisigHub): Int {
        val words = words(container, hub)
        val moved = container.wallet.feedExecutor.rewordRecords { row -> MemberRowTitles.reword(row) { words.memberChangeTitle(it) } }
        if (moved > 0) {
            VelaLog.event("multisig.activity", "member rows worded again", "rows" to moved, "language" to container.i18nRuntime.language)
            container.wallet.feedReconciled(moved)
        }
        return moved
    }

    /** The multisig's words, with this device's account names and networks. */
    fun words(container: AppContainer, hub: MultisigHub? = null): MultisigWords {
        container.i18nRuntime.include(I18nCatalogs.MULTISIG)
        val networks = container.settings.networks.value.networks
        return MultisigWords(
            strings = container.i18nRuntime,
            accounts = hub?.accountsView?.value ?: app.getvela.wallet.feature.multisig.core.MsAccountsView(),
            localNames = container.session.view.value.accounts.associate { it.address.lowercase() to it.name },
            networkName = { id -> networks.firstOrNull { it.chain_id.toInt() == id }?.display_name },
            nativeSymbol = { id -> networks.firstOrNull { it.chain_id.toInt() == id }?.native_symbol },
            tokenFacts = { id, token -> hub?.proposalsView?.value?.tokens?.get("$id:${token.lowercase()}") },
        )
    }

    /** The multisig service in use, as the machines are told it. */
    suspend fun serviceUrl(container: AppContainer, store: KeyValueStore): String =
        resolveServiceUrl(
            fromSettings = runCatching { container.settings.endpointOverride(NetEndpointField.GenesisService) }.getOrNull(),
            storedEndpoints = store.read(KeyValueStore.Keys.SERVICE_ENDPOINTS),
        )

    /**
     * Which multisig service: the person's own (as Settings holds it, else as
     * `vela.serviceEndpoints` stores it), else Vela's.
     *
     * Real-device pass 2026-11-01: this used to take Settings' row with its
     * DEFAULT filled in, so whenever the network machine had not loaded the
     * stored endpoints yet the answer was Vela's service and the store was
     * never read — a phone pointed at its own (or a test harness's) service
     * created its multisig on safe-genesis.getvela.app. The store is the
     * source Settings itself writes, so it is read before any default.
     */
    internal fun resolveServiceUrl(fromSettings: String?, storedEndpoints: String?): String {
        val stored = storedEndpoints?.let { runCatching { JSONObject(it).optString("genesisServiceURL") }.getOrNull() }
        val raw = fromSettings?.trim()?.ifBlank { null } ?: stored?.trim()?.ifBlank { null }
        return raw?.trimEnd('/')?.ifBlank { null } ?: DEFAULT_SERVICE
    }

    /** Vela's connect page, at the wallet's public web address, in the person's language. */
    fun connectPage(container: AppContainer): String {
        val origin = MsRules.shareOrigin()
        return "$origin/${pageLocale(container)}/connect"
    }

    /** The page's locale segment for the app's language (a page that exists: else `en`). */
    fun pageLocale(container: AppContainer): String {
        val lang = runCatching { container.i18nRuntime.language }.getOrDefault("en")
        return lang.ifBlank { "en" }
    }

    private suspend fun readRelay(store: KeyValueStore): String? = store.read(RELAY_KEY)?.ifBlank { null }

    /** The app's facts as the machines read them. */
    private suspend fun facts(container: AppContainer, store: KeyValueStore, port: StoreAccountPort): MultisigHub.Facts {
        // The networks list is the settings machine's: read it once it is.
        if (!container.settings.networks.value.loaded) {
            container.settings.startNetworks()
            withTimeoutOrNull(10_000) { container.settings.networks.first { it.loaded } }
        }
        // r13-6: the space as its store says it (read at launch, async), and
        // its networks from the list just read — not the derived flow, which
        // follows them a dispatch later and said "no networks" to a boot that
        // raced it.
        withTimeoutOrNull(5_000) { container.networkSpace.ready() }
        val space = app.getvela.wallet.feature.settings.core.NetworkSpaces.chainIds(
            container.networkSpace.space.value,
            container.settings.networks.value.networks,
        )
        val session = container.session.view.value
        val local = session.accounts.map { row ->
            val accountJson = port.accountJson(row.address)
            mapOf(
                "address" to row.address,
                "name" to row.name,
                "keys" to port.keysOf(row.address).map { mapOf("credential_id" to it.credentialId, "public_key_hex" to it.publicKeyHex) },
                "route" to (accountJson?.let { route(it) } ?: "vela"),
            )
        }
        val contacts = container.contacts.view.value.contacts
        val known = contacts.map { mapOf("address" to it.address, "label" to (it.name?.ifBlank { null } ?: it.resolved_name ?: it.address)) } +
            session.accounts.map { mapOf("address" to it.address, "label" to it.name.ifBlank { it.address }, "kind" to "own") }
        return MultisigHub.Facts(
            serviceUrl = serviceUrl(container, store),
            accounts = local,
            networks = container.settings.networks.value.networks.map { it.chain_id.toInt() },
            space = space,
            known = known,
            contacts = contacts.filter { ADDRESS.matches(it.address) }.map {
                mapOf("address" to it.address, "name" to it.name, "resolved_name" to it.resolved_name, "kind" to null)
            },
        )
    }

    private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

    /** How an account approves (`vela` / `page` / `blocked`): the core's `multisigApprovalRoute`. */
    fun route(accountJson: String): String {
        val answer = runCatching { multisigApprovalRoute(accountJson) }.getOrNull() ?: return "vela"
        val parsed = runCatching { JSONObject(answer).optString("type") }.getOrNull()
            ?: answer.trim('"')
        return when (parsed) {
            "page" -> "page"
            "blocked" -> "blocked"
            else -> "vela"
        }
    }

    /** A token's symbol and decimals: the chain's coin from this device's list, an ERC-20's from the chain. */
    /** A token's symbol and decimals, for a screen that words an amount. */
    suspend fun tokenInfoOf(container: AppContainer, chainId: Int, token: String?, hub: MultisigHub? = null): Pair<String, Int?> =
        tokenInfo(container, chainId, token, hub)

    /**
     * r12-1: a token's symbol and decimals — the core's facts first (the
     * registry, else the contract's own answer, `tokens` in the proposals
     * view), else the contract's `symbol()` / `decimals()` asked here; `null`
     * decimals when nobody said them, never 18 assumed. The network's coin
     * is counted in wei ([MsRules.COIN_DECIMALS]).
     */
    private suspend fun tokenInfo(container: AppContainer, chainId: Int, token: String?, hub: MultisigHub? = null): Pair<String, Int?> {
        val native = container.settings.networks.value.networks.firstOrNull { it.chain_id.toInt() == chainId }?.native_symbol ?: "ETH"
        if (token.isNullOrBlank()) return native to app.getvela.wallet.feature.multisig.core.MsRules.COIN_DECIMALS
        hub?.proposalsView?.value?.tokens?.get("$chainId:${token.lowercase()}")?.let { facts ->
            val decimals = (facts as? kotlinx.serialization.json.JsonObject)?.get("decimals")?.let { (it as? kotlinx.serialization.json.JsonPrimitive)?.content?.toIntOrNull() }
            val symbol = (facts as? kotlinx.serialization.json.JsonObject)?.get("symbol")?.let { (it as? kotlinx.serialization.json.JsonPrimitive)?.takeIf { p -> p.isString }?.content }?.takeIf { it.isNotBlank() }
            if (decimals != null) return (symbol ?: MultisigWords.short(token)) to decimals
        }
        return runCatching {
            val pool = container.pool
            val symbol = (pool.call(chainId, "eth_call", listOf(JSONObject().put("to", token).put("data", Abi.encodeSymbol()), "latest")) as? RpcResult.Body)
                ?.json?.optString("result")?.takeIf { it.startsWith("0x") }?.let(Abi::decodeString)
            val decimals = (pool.call(chainId, "eth_call", listOf(JSONObject().put("to", token).put("data", Abi.encodeDecimals()), "latest")) as? RpcResult.Body)
                ?.json?.optString("result")?.takeIf { it.startsWith("0x") }?.let(Abi::decodeUint8)
            if (symbol != null && decimals != null) symbol to decimals else null
        }.onFailure { VelaLog.failure("multisig", "token metadata", it) }.getOrNull()
            ?: (app.getvela.wallet.feature.multisig.MultisigWords.short(token) to null)
    }
}
