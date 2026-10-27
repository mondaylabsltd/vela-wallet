package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import uniffi.vela_core_uniffi.networkSpaceChainIds
import uniffi.vela_core_uniffi.networkSpaceRead

/**
 * The mainnet space and the testnet space (spec 105).
 *
 * The mainnet space holds networks whose assets have real value; the testnet
 * space, testnets and developer networks with nothing of real value. Which
 * one a network belongs to is the core's (`NetNetworkRow.is_testnet`).
 */
enum class NetworkSpace(val wire: String) {
    Mainnet("mainnet"),
    Testnet("testnet");

    val testnet: Boolean get() = this == Testnet

    /** Where the switch goes from here. */
    val other: NetworkSpace get() = if (testnet) Mainnet else Testnet

    companion object {
        /** The core's spelling read back; anything else is the mainnet space (as the core reads it). */
        fun of(wire: String?): NetworkSpace = entries.firstOrNull { it.wire == wire } ?: Mainnet
    }
}

/**
 * Which networks a space holds — the core's one function
 * (`network_space_chain_ids`) over the network machine's rows, so the four
 * shells cannot disagree on what a space holds. Pure.
 */
object NetworkSpaces {

    /** The chain ids of [space]'s networks, in the rows' order. Rows the core cannot read are none. */
    fun chainIds(space: NetworkSpace, rows: List<NetNetworkRow>): List<Int> {
        if (rows.isEmpty()) return emptyList()
        return runCatching {
            val json = Wire.json.encodeToString(ListSerializer(NetNetworkRow.serializer()), rows)
            Wire.json.decodeFromString(ListSerializer(Int.serializer()), networkSpaceChainIds(space.wire, json))
        }.onFailure { VelaLog.failure("network.space", "the core could not read the network rows", it) }
            .getOrDefault(emptyList())
    }

    /**
     * [view] with only [space]'s networks — what every list a person sees is
     * drawn from: the balances read and the total, assets, receive, send's
     * chains and tokens, adding a token, the chains a dApp and the signing
     * sheet may use. Everything else in the view (the wizard, the endpoints,
     * the providers) is the whole device's and passes through.
     */
    fun view(space: NetworkSpace, view: NetView): NetView {
        if (view.networks.isEmpty()) return view
        val ids = chainIds(space, view.networks).toSet()
        return view.copy(networks = view.networks.filter { it.chain_id.toInt() in ids })
    }
}

/**
 * The moment after a switch of space, when the screen beneath has just been
 * rebuilt for the other space and nothing on it is where it was (spec 105).
 *
 * A second tap of a quick double tap on the switch's confirm outlived the
 * sheet: the sheet closed on the first, and the system handed the second to
 * the app beneath — on Settings → Networks, to the row that had just moved
 * under the finger (S22, 2026-10-27, the dispatcher's log). For [WINDOW_MS]
 * after a switch the app beneath takes no new touch. The clock is monotonic
 * (`System.nanoTime`, which a JVM test has too); the tests pass their own.
 */
class SpaceSwitchSettle(
    private val windowMs: Long = WINDOW_MS,
    private val clock: () -> Long = { System.nanoTime() / 1_000_000 },
) {
    @Volatile
    private var switchedAt: Long? = null

    /** The space switched at [nowMs] (uptime). */
    fun switched(nowMs: Long) {
        switchedAt = nowMs
    }

    /** Whether a touch that starts at [nowMs] belongs to the switch, not to the screen. */
    fun blocks(nowMs: Long = clock()): Boolean = switchedAt?.let { nowMs - it in 0 until windowMs } ?: false

    /** The space switched now. */
    fun switchedNow() = switched(clock())

    companion object {
        /** Longer than a double tap's gap, shorter than a person reaching for the next thing. */
        const val WINDOW_MS = 500L
    }
}

/**
 * The device's space (spec 105): one value, `vela.networkSpace`, shared by
 * every wallet on the device. Read and validated by the core
 * (`network_space_read`) — anything missing or unreadable is the mainnet
 * space, where a device starts. A user setting: clearing caches keeps it
 * (the core's storage catalog lists it with no cache row), an erase does not.
 *
 * App-resident, like [app.getvela.wallet.core.data.Preferences]: the home's
 * label and the testnet bar are drawn from the first frame, and every list's
 * filter follows [space].
 */
class NetworkSpacePreference(
    private val store: KeyValueStore,
    private val scope: CoroutineScope,
) {
    private val _space = MutableStateFlow(NetworkSpace.Mainnet)

    /** The space in front. */
    val space: StateFlow<NetworkSpace> = _space

    /**
     * The switch prompt is up — opened by the home's label, the testnet bar
     * or Settings → Networks; drawn once, over every route.
     */
    val switcherOpen = MutableStateFlow(false)

    /** The touches that belong to a switch just made, not to the screen beneath. */
    val settle = SpaceSwitchSettle()

    private val loaded = CompletableDeferred<Unit>()

    /** A switch made before the launch read landed: that read must not undo it. */
    @Volatile
    private var chosen = false

    /** Read the stored space through the core. Once, at launch. */
    fun load() {
        scope.launch {
            val stored = runCatching { store.read(KEY) }.getOrNull()
            if (!chosen) _space.value = NetworkSpace.of(networkSpaceRead(stored))
            loaded.complete(Unit)
        }
    }

    /** Wait for [load]: a first balance read must not count the wrong space. */
    suspend fun ready() = loaded.await()

    /**
     * The switch — the person's (through the `set_network_space` intent) or
     * an approved agent's. Shown at once; stored best-effort, as every
     * preference is.
     */
    fun set(space: NetworkSpace) {
        VelaLog.event("network.space", "set", "space" to space.wire)
        if (space != _space.value) settle.switchedNow()
        chosen = true
        _space.value = space
        scope.launch {
            if (!store.write(KEY, space.wire)) VelaLog.event("network.space", "not stored")
        }
    }

    /** The store changed under this value — an erase removed every `vela.` key: read it back. */
    fun reload() {
        scope.launch { _space.value = NetworkSpace.of(networkSpaceRead(store.read(KEY))) }
    }

    companion object {
        /** The core's `network_space::KEY`. */
        const val KEY = "vela.networkSpace"
    }
}
