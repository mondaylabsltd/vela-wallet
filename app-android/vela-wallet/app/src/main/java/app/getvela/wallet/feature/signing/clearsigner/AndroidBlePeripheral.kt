package app.getvela.wallet.feature.signing.clearsigner

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelUuid
import androidx.core.content.ContextCompat
import app.getvela.wallet.core.diagnostics.VelaLog
import java.util.UUID
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit

/**
 * The phone as a GATT peripheral for the Clear Signer's BLE channel
 * (PROTOCOL.md §1): the core's service, `c2p` for what the page writes, `p2c`
 * for what the wallet notifies, and an advertisement carrying the service uuid
 * plus this phone's own Bluetooth name, so the person can pick their device
 * out of Chrome's chooser.
 *
 * The name is the phone's, not one this app invents. Renaming the adapter
 * would rename it for every app and every paired car, and the chooser shows
 * the adapter's name whatever we would rather it showed — so the card above
 * says the same name instead, and the person knows what to look for.
 *
 * Permissions are the caller's business ([AndroidClearSignerBleHost] asks
 * before this is built); everything here runs only once they are held, which
 * is what the `MissingPermission` suppression rests on.
 */
@SuppressLint("MissingPermission")
class AndroidBlePeripheral(private val context: Context) : BlePeripheral {

    private val manager: BluetoothManager? =
        ContextCompat.getSystemService(context, BluetoothManager::class.java)

    override val deviceName: String
        get() = runCatching { manager?.adapter?.name }.getOrNull()?.takeIf { it.isNotBlank() }
            ?: Build.MODEL

    @Volatile
    private var server: BluetoothGattServer? = null

    @Volatile
    private var central: BluetoothDevice? = null

    @Volatile
    private var events: BlePeripheralEvents? = null

    @Volatile
    private var stopped = false

    private lateinit var p2c: BluetoothGattCharacteristic

    /**
     * One notification at a time. The stack drops a second
     * `notifyCharacteristicChanged` queued before `onNotificationSent`, and a
     * dropped frame is a message that never completes — the failure the
     * reassembler's ten-second sweep exists to REPORT rather than to hide.
     */
    private val sent = ArrayBlockingQueue<Boolean>(1)

    private val advertiseCallback = object : AdvertiseCallback() {
        override fun onStartFailure(errorCode: Int) {
            VelaLog.event("clearsigner.ble", "advertising refused", "code" to errorCode.toString())
            events?.onGone("advertising refused ($errorCode)")
        }
    }

    @Volatile
    private var started = false

    override fun start(events: BlePeripheralEvents): Boolean {
        // One peripheral, one session. `events` is a single field, and a
        // second wire attaching over the first would take delivery of frames
        // the first is still waiting for — which is exactly how an answer goes
        // missing while both ends believe they are talking (T043).
        if (started || stopped) {
            VelaLog.event("clearsigner.ble", "a second start on a used peripheral was refused")
            return false
        }
        started = true
        this.events = events
        val manager = this.manager ?: return false
        val adapter = manager.adapter ?: return false
        if (!adapter.isEnabled) return false
        val advertiser = adapter.bluetoothLeAdvertiser ?: return false

        val opened = runCatching { manager.openGattServer(context, gattCallback) }.getOrNull() ?: return false
        server = opened

        val service = BluetoothGattService(BleGatt.service, BluetoothGattService.SERVICE_TYPE_PRIMARY)
        // c2p: the page writes here. Both write forms, because Web Bluetooth
        // prefers `writeValueWithoutResponse` and falls back to `writeValue`.
        service.addCharacteristic(
            BluetoothGattCharacteristic(
                BleGatt.c2p,
                BluetoothGattCharacteristic.PROPERTY_WRITE or
                    BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE,
                BluetoothGattCharacteristic.PERMISSION_WRITE,
            ),
        )
        // p2c: the wallet notifies here. The CCCD is not optional — without a
        // client-configuration descriptor the central has nothing to write to
        // and `startNotifications()` fails.
        p2c = BluetoothGattCharacteristic(
            BleGatt.p2c,
            BluetoothGattCharacteristic.PROPERTY_NOTIFY or BluetoothGattCharacteristic.PROPERTY_READ,
            BluetoothGattCharacteristic.PERMISSION_READ,
        )
        p2c.addDescriptor(
            BluetoothGattDescriptor(
                CCCD,
                BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE,
            ),
        )
        service.addCharacteristic(p2c)
        if (!runCatching { opened.addService(service) }.getOrDefault(false)) {
            stop()
            return false
        }

        val settings = AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
            .setConnectable(true)
            // The session's own five-minute clock ends this, not the radio's.
            .setTimeout(0)
            .build()
        // A 128-bit service uuid takes 18 of the advertisement's 31 bytes, so
        // the name goes in the SCAN RESPONSE — where it arrives whole rather
        // than truncated to whatever was left over.
        val advertisement = AdvertiseData.Builder()
            .setIncludeDeviceName(false)
            .addServiceUuid(ParcelUuid(BleGatt.service))
            .build()
        val scanResponse = AdvertiseData.Builder().setIncludeDeviceName(true).build()
        val asked = runCatching {
            advertiser.startAdvertising(settings, advertisement, scanResponse, advertiseCallback)
            true
        }.getOrDefault(false)
        if (!asked) {
            stop()
            return false
        }
        return true
    }

    override fun notify(frame: ByteArray): Boolean {
        val target = central ?: return false
        val live = server ?: return false
        sent.clear()
        val queued = runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                live.notifyCharacteristicChanged(target, p2c, false, frame) == BluetoothStatusCodes.SUCCESS
            } else {
                @Suppress("DEPRECATION")
                p2c.value = frame
                @Suppress("DEPRECATION")
                live.notifyCharacteristicChanged(target, p2c, false)
            }
        }.getOrDefault(false)
        if (!queued) return false
        return sent.poll(NOTIFY_TIMEOUT_MS, TimeUnit.MILLISECONDS) ?: false
    }

    override fun stop() {
        if (stopped) return
        stopped = true
        VelaLog.event("clearsigner.ble", "the peripheral is stopping")
        // Detached BEFORE the teardown: a disconnection raised by closing the
        // server is this end's own doing, and a wire that has finished with
        // the radio should not be told the page left.
        events = null
        runCatching { manager?.adapter?.bluetoothLeAdvertiser?.stopAdvertising(advertiseCallback) }
        val live = server
        server = null
        central = null
        runCatching { live?.close() }
        // Anything blocked on a notification is not getting one.
        sent.offer(false)
    }

    private val gattCallback = object : BluetoothGattServerCallback() {
        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            when (newState) {
                BluetoothProfile.STATE_CONNECTED ->
                    // One conversation at a time: a second central would share
                    // this session's `msgId`s and its counters, which the core
                    // would (rightly) refuse to open.
                    if (central == null) {
                        central = device
                        VelaLog.event("clearsigner.ble", "a central connected")
                    }
                BluetoothProfile.STATE_DISCONNECTED ->
                    if (central?.address == device.address) {
                        central = null
                        sent.offer(false)
                        events?.onGone("the page disconnected")
                    }
            }
        }

        override fun onMtuChanged(device: BluetoothDevice, mtu: Int) {
            VelaLog.event("clearsigner.ble", "the central negotiated", "mtu" to mtu.toString())
            events?.onMtu(mtu)
        }

        override fun onNotificationSent(device: BluetoothDevice, status: Int) {
            sent.offer(status == BluetoothGatt.GATT_SUCCESS)
        }

        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray?,
        ) {
            if (characteristic.uuid != BleGatt.c2p) {
                if (responseNeeded) {
                    server?.sendResponse(device, requestId, BluetoothGatt.GATT_FAILURE, offset, null)
                }
                return
            }
            // A long (prepared) write would arrive in pieces this end never
            // reassembles. It cannot happen — every frame is at most the
            // negotiated chunk — and answering "not supported" says so out
            // loud rather than dropping half a frame into the reassembler.
            if (preparedWrite) {
                if (responseNeeded) {
                    server?.sendResponse(
                        device, requestId, BluetoothGatt.GATT_REQUEST_NOT_SUPPORTED, offset, null,
                    )
                }
                return
            }
            if (responseNeeded) {
                server?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, null)
            }
            if (central == null) central = device
            val listening = events
            if (listening == null) {
                VelaLog.event("clearsigner.ble", "a write arrived with nothing listening")
                return
            }
            value?.let(listening::onFrame)
        }

        override fun onCharacteristicReadRequest(
            device: BluetoothDevice,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic,
        ) {
            // `p2c` is readable so the profile matches PROTOCOL §1, but the
            // page subscribes; there is never a value sitting here to fetch.
            server?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, ByteArray(0))
        }

        override fun onDescriptorWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray?,
        ) {
            if (responseNeeded) {
                server?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, value)
            }
        }

        override fun onDescriptorReadRequest(
            device: BluetoothDevice,
            requestId: Int,
            offset: Int,
            descriptor: BluetoothGattDescriptor,
        ) {
            server?.sendResponse(
                device,
                requestId,
                BluetoothGatt.GATT_SUCCESS,
                offset,
                BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE,
            )
        }
    }

    private companion object {
        /** The client-characteristic-configuration descriptor, as every profile spells it. */
        val CCCD: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

        /**
         * How long one frame may take to leave the stack. Generous — a phone
         * under load is slow, not broken — but finite, because a notification
         * that never completes would otherwise hold the whole session.
         */
        const val NOTIFY_TIMEOUT_MS = 5_000L
    }
}

/**
 * The activity's side of the Bluetooth route: the hardware, the adapter, and
 * the two runtime permissions Android 12 introduced — one to advertise, one to
 * talk to whatever connects.
 *
 * [askPermissions] is the activity's launcher. A permission dialog needs one,
 * and a launcher must be registered before the activity is STARTED, so it is
 * passed in rather than built here.
 */
class AndroidClearSignerBleHost(
    private val context: Context,
    private val askPermissions: suspend (Array<String>) -> Boolean,
    /**
     * `ACTION_REQUEST_ENABLE` — the system's own localized "turn Bluetooth on"
     * dialog. Asked rather than reported, because an adapter that is merely
     * off is one tap from being on and a card that says so is a dead end the
     * caBLE flow already learned to avoid.
     */
    private val enableAdapter: suspend () -> Boolean = { false },
) : ClearSignerBleHost {

    override suspend fun ready(): BleReadiness {
        if (!context.packageManager.hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE)) {
            return BleReadiness.Unsupported
        }
        val manager = ContextCompat.getSystemService(context, BluetoothManager::class.java)
        val adapter = manager?.adapter ?: return BleReadiness.Unsupported
        val missing = permissions().filter {
            ContextCompat.checkSelfPermission(context, it) != PackageManager.PERMISSION_GRANTED
        }
        if (missing.isNotEmpty() && !askPermissions(missing.toTypedArray())) {
            return BleReadiness.Refused
        }
        // Asked AFTER the permissions: on API 31+ reading the adapter's state
        // and its name is itself gated by BLUETOOTH_CONNECT, and
        // ACTION_REQUEST_ENABLE requires it too.
        if (!runCatching { adapter.isEnabled }.getOrDefault(false)) {
            if (!enableAdapter()) return BleReadiness.AdapterOff
            if (!runCatching { adapter.isEnabled }.getOrDefault(false)) return BleReadiness.AdapterOff
        }
        // Plenty of chipsets can scan but not advertise. Finding that out here
        // is a sentence; finding it out later is a wait that never ends.
        val canAdvertise = runCatching {
            adapter.isMultipleAdvertisementSupported && adapter.bluetoothLeAdvertiser != null
        }.getOrDefault(false)
        if (!canAdvertise) return BleReadiness.Unsupported
        return BleReadiness.Ready
    }

    /**
     * One radio at a time, whatever the flow above it did.
     *
     * Every peripheral opens its own GATT server and its own advertiser, and
     * an abandoned one keeps both — a second service with the same uuid for
     * the page's chooser to find, and an advert nobody is watching that a
     * stranger can still connect to. A flow that ended cleanly stops its own;
     * this is for the ones that did not.
     */
    @Volatile
    private var current: BlePeripheral? = null

    override fun peripheral(): BlePeripheral {
        current?.let { previous ->
            VelaLog.event("clearsigner.ble", "stopping the peripheral the last flow left behind")
            runCatching { previous.stop() }
        }
        return AndroidBlePeripheral(context).also { current = it }
    }

    private fun permissions(): List<String> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            listOf(
                android.Manifest.permission.BLUETOOTH_ADVERTISE,
                android.Manifest.permission.BLUETOOTH_CONNECT,
            )
        } else {
            // API ≤30 grants BLUETOOTH and BLUETOOTH_ADMIN at install time, and
            // advertising needs no location permission — only scanning does.
            emptyList()
        }
}
