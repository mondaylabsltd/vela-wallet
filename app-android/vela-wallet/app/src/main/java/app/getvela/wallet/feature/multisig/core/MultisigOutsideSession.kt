package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.crux.asBridge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import uniffi.vela_core_uniffi.MultisigOutsideSessionCore

/**
 * Vela's side of a live session with an outside owner's wallet (M4), run by
 * the core's session machine (`multisig_outside::session::OutsideSession`,
 * uniffi `MultisigOutsideSessionCore`): the room (or a pairing kept before),
 * `walletpair_connect`, the owner among the wallet's accounts, the network
 * switched, the SafeTx signed — and the answer verified by the core before it
 * is handed back — or the transaction sent. The machine decides; this holds
 * its view for the sheet and hands it the PeerConnect rooms, the store and
 * the mailbox (the web's `outside-session.svelte.ts`, iOS's
 * `MultisigOutsideSession.swift`).
 *
 * The outside owner opens Vela's connect page in a browser with its own
 * wallet ([link]); that page and this session meet in the room.
 */
class MultisigOutsideSessionModel(
    val request: JsonObject,
    ports: MultisigPorts,
    private val rooms: MultisigPeerRooms?,
    private val relayUrl: String?,
    private val serviceUrl: String,
    /** Vela's connect page in the person's language (`https://…/<locale>/connect`). */
    private val connectPage: String,
    private val scope: CoroutineScope,
    private val onOutcome: (JsonObject) -> Unit,
) {
    private val machine = MultisigMachine(
        MultisigOutsideSessionCore().asBridge(),
        scope,
        ScopedPorts(ports, rooms ?: NoPeer),
        "outside session",
    )

    /** The session machine's view, raw. */
    val view: StateFlow<JsonObject> = machine.view

    @Volatile private var answered = false
    private var mailboxTimer: Job? = null
    private var watcher: Job? = null

    init {
        rooms?.tell = { event -> machine.dispatch(event) }
    }

    fun start() {
        machine.dispatch(
            ev("start", "request" to request, "service_url" to serviceUrl, "relay" to relayUrl, "origin" to null),
        )
        watcher = scope.launch {
            machine.view.collect { raw ->
                val outcome = raw["outcome"] as? JsonObject
                if (!answered && outcome != null) {
                    answered = true
                    mailboxTimer?.cancel()
                    onOutcome(outcome)
                }
            }
        }
        // How often the mailbox is looked at for an approval made from the
        // link alone (r4C-2, the web's `MAILBOX_EVERY_MS`).
        mailboxTimer = scope.launch {
            while (isActive) {
                delay(MAILBOX_EVERY_MS)
                machine.dispatch(ev("mailbox_tick"))
            }
        }
    }

    /**
     * The page to open in a browser: with the invite for a new room, none for
     * one paired before — the members who may sign and the transaction, all
     * in the fragment (the core's `connectLink`).
     */
    val link: String?
        get() {
            val v = view.value
            val tx = v["tx"]?.let { tx ->
                val chain = tx["chainId"].int ?: return@let null
                val safe = tx["safe"].str ?: return@let null
                val hash = tx["safeTxHash"].str ?: return@let null
                Triple(chain, safe, hash)
            }
            return MsRules.connectLink(connectPage, v["invite"].str, v["members"].arr.mapNotNull { it.str }, tx)
        }

    val phase: String get() = view.value["phase"].str ?: "waiting"
    val failure: JsonElement? get() = (view.value as JsonElement)["failure"]

    fun askAgain() = machine.dispatch(ev("ask_again"))
    fun stopWaiting() = machine.dispatch(ev("stop_waiting"))
    fun anotherWallet(unpair: Boolean) = machine.dispatch(ev("another_wallet", "unpair" to unpair))
    fun retryRelay() = rooms?.retryAll()

    /**
     * The person closed the sheet: the machine says how it ended (cancelled,
     * unless it was answered), the rooms are left (the pairing kept).
     */
    fun close() {
        machine.dispatch(ev("close"))
        mailboxTimer?.cancel()
        if (!answered) {
            answered = true
            onOutcome(jo("kind" to "cancelled"))
        }
        rooms?.leaveAll()
        watcher?.cancel()
    }

    /** The app's ports, with a session's own rooms; a session routes nothing (its outcome is its view's). */
    private class ScopedPorts(private val base: MultisigPorts, override val peer: MultisigPeerTransport) : MultisigPorts by base {
        override suspend fun notice(notice: JsonObject) = Unit
    }

    companion object {
        const val MAILBOX_EVERY_MS = 8_000L
    }
}

/**
 * The PeerConnect rooms a session opens, as the session machine reads them:
 * open / send / receive / close, and what happens to the rooms told back as
 * the machine's own events ([tell]: `peer_state`, `paired`, `relay`).
 */
interface MultisigPeerRooms : MultisigPeerTransport {
    var tell: ((JsonObject) -> Unit)?
    fun retryAll()
    fun leaveAll()
}
