package app.getvela.wallet.feature.signing.clearsigner

import uniffi.vela_core_uniffi.clearSignerBleUuids
import java.util.UUID

/**
 * The GATT identifiers this peripheral serves and advertises (PROTOCOL.md §1),
 * read from the core rather than retyped here.
 *
 * A wrong digit is a phone that never appears in the page's chooser, which is
 * the hardest way there is to find a typo — and the page filters its scan on
 * the service uuid, so the three of them have to agree byte for byte with
 * `vela_core::clear_signer::ble`.
 */
object BleGatt {
    private val uuids: List<String> by lazy { clearSignerBleUuids() }

    /** The service a wallet advertises; Chrome's device chooser filters on it. */
    val service: UUID by lazy { UUID.fromString(uuids[0]) }

    /** Central → peripheral: `write`, `writeWithoutResponse`. */
    val c2p: UUID by lazy { UUID.fromString(uuids[1]) }

    /** Peripheral → central: `notify`, `read`. */
    val p2c: UUID by lazy { UUID.fromString(uuids[2]) }
}

/**
 * The radio, as the Clear Signer's BLE channel needs it (spec 075 T040,
 * PROTOCOL.md §1).
 *
 * The wallet is the PERIPHERAL and the Clear Signer page is the central: a
 * browser can only ever be a central, and Web Bluetooth exists only inside a
 * document. So this end advertises the service, holds the two characteristics
 * and moves frames — nothing more. The framing above it is the core's
 * (`ClearSignerFramer` / `ClearSignerReassembler`), the session is the core's
 * (`ClearSignerHandshake` / `ClearSignerSession`), and every verdict is the
 * core's too.
 *
 * It is an interface for the same reason [TunnelSockets] is: a peripheral can
 * only be exercised on real hardware, and the whole of the glue above it — the
 * handshake, the code gate, reassembly, `bye` — has to be testable without a
 * radio. The JVM suite drives a fake that calls these callbacks directly.
 */
interface BlePeripheral {
    /**
     * The name the browser's device chooser will show. The person picks their
     * own phone out of a list by it, so the card says it before they look.
     */
    val deviceName: String

    /**
     * Stand the GATT service up and start advertising. `false` when the radio
     * would not — a server that would not open, an advert the chipset refused
     * outright. A refusal that arrives later comes back through
     * [BlePeripheralEvents.onGone].
     */
    fun start(events: BlePeripheralEvents): Boolean

    /**
     * Push one frame to the connected central as a notification on `p2c`.
     * `false` when it did not go out — the caller halves the chunk and sends
     * the whole message again (PROTOCOL §2).
     */
    fun notify(frame: ByteArray): Boolean

    /** Stop advertising, close the server, forget the central. Idempotent. */
    fun stop()
}

/** What the radio tells the channel above it. */
interface BlePeripheralEvents {
    /** One frame arrived on `c2p`, exactly as the central wrote it. */
    fun onFrame(bytes: ByteArray)

    /**
     * The central negotiated this ATT MTU; a frame can carry three bytes less.
     * Until this arrives, the default of 23 is all that is promised — though
     * every modern phone raises it in the first breath of the connection.
     */
    fun onMtu(mtu: Int)

    /** The central left, the advert failed, or the radio went away. */
    fun onGone(reason: String)
}

/**
 * Whether this phone can be a Clear Signer peripheral at all — asked before
 * the route is taken, so a refusal is a card that says what is missing rather
 * than a wait that never ends.
 */
sealed interface BleReadiness {
    data object Ready : BleReadiness

    /** No BLE, or a chipset that cannot advertise (plenty cannot). */
    data object Unsupported : BleReadiness

    /** The adapter is off. The person can fix this in one tap. */
    data object AdapterOff : BleReadiness

    /** Android 12+: `BLUETOOTH_ADVERTISE` / `BLUETOOTH_CONNECT` were refused. */
    data object Refused : BleReadiness
}

/**
 * What the Bluetooth route needs from the platform. The activity owns it,
 * because a runtime permission needs a launcher registered before STARTED and
 * a GATT server needs a context.
 */
interface ClearSignerBleHost {
    /**
     * Hardware, adapter and runtime permissions — asking for whatever is
     * missing, and answering with whatever is still in the way.
     */
    suspend fun ready(): BleReadiness

    /** A peripheral for one session. [BlePeripheral.stop] ends its life. */
    fun peripheral(): BlePeripheral
}
