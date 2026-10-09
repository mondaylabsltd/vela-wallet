package app.getvela.wallet.feature.onboarding.core

import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.ParcelUuid
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.suspendCancellableCoroutine
import uniffi.vela_core_uniffi.CableAdvert
import uniffi.vela_core_uniffi.cableTryDecryptAdvert
import java.util.UUID
import kotlin.coroutines.resume

/**
 * Finds the phone that scanned OUR QR, by its BLE proximity advert — a port of
 * the founder's proven demo (`apppasskeysdemo/.../HybridBleClient.findResponder`),
 * with the crypto moved into vela-core: every candidate goes through
 * `cableTryDecryptAdvert`, which answers with the decrypted EID and the L2CAP
 * PSM in one step.
 *
 * Two discovery shapes, because the platforms advertise differently:
 *
 *  * **service data** under 0xFFF9/0xFDE2 — Android authenticators (GMS,
 *    securitykeys) broadcast the sealed EID directly;
 *  * **bare service UUID** — iOS cannot broadcast service data, so an iOS
 *    authenticator advertises just 0xFFF9 and publishes the EID + PSM on GATT
 *    characteristic 0xFFFA, read after a short connect.
 */
class HybridCableScanner(context: Context) {
    private val appContext = context.applicationContext
    private val adapter: BluetoothAdapter? =
        (context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager)?.adapter

    fun bluetoothReady(): Boolean = adapter?.isEnabled == true

    /** The matched phone: where to connect, and what its advert said. */
    data class AdvertHit(
        val device: BluetoothDevice,
        val advert: CableAdvert,
    )

    @SuppressLint("MissingPermission")
    suspend fun findResponder(qrSecret: ByteArray, timeoutMs: Long): AdvertHit? =
        suspendCancellableCoroutine { cont ->
            val scanner = adapter?.bluetoothLeScanner
            if (scanner == null) {
                VelaLog.event("cable.scan", "bluetooth unavailable")
                if (cont.isActive) cont.resume(null)
                return@suspendCancellableCoroutine
            }
            val handler = Handler(Looper.getMainLooper())
            var finished = false
            var gattStarted = false
            // The iOS fallback's link, so a scan cancelled mid-read (the code
            // dismissed, issue #459) does not leave the radio busy for a retry.
            val gattLink = java.util.concurrent.atomic.AtomicReference<BluetoothGatt?>(null)
            lateinit var cb: ScanCallback
            fun finish(result: AdvertHit?) {
                if (finished) return
                finished = true
                handler.removeCallbacksAndMessages(null)
                runCatching { scanner.stopScan(cb) }
                if (cont.isActive) cont.resume(result)
            }
            cb = object : ScanCallback() {
                override fun onScanResult(callbackType: Int, result: ScanResult) {
                    val rec = result.scanRecord ?: return
                    for (uuid in SERVICE_UUIDS) {
                        val sd = rec.getServiceData(uuid) ?: continue
                        if (sd.size < 20) continue
                        // The WHOLE payload: bytes past 20 are the CTAP 2.3 BLE
                        // suffix the core reads the PSM from.
                        val advert = cableTryDecryptAdvert(qrSecret, sd) ?: continue
                        VelaLog.event(
                            "cable.scan",
                            "matched this QR",
                            "rssi" to result.rssi,
                            "psm" to (advert.psm ?: "none"),
                        )
                        finish(AdvertHit(result.device, advert))
                        return
                    }
                    // No service data but our service UUID: an iOS authenticator.
                    // Read the EID/PSM from its GATT proximity characteristic.
                    if (!gattStarted && rec.serviceUuids?.any { it in SERVICE_UUIDS } == true) {
                        gattStarted = true
                        runCatching { scanner.stopScan(cb) }
                        VelaLog.event("cable.scan", "bare 0xFFF9 UUID (iOS authenticator?) — reading GATT 0xFFFA")
                        gattLink.set(readAdvertViaGatt(result.device, qrSecret) { hit -> finish(hit) })
                        // Cancelled while the link was being made: close it here.
                        if (!cont.isActive) closeGatt(gattLink.getAndSet(null))
                    }
                }

                override fun onScanFailed(errorCode: Int) {
                    VelaLog.event("cable.scan", "scan failed", "code" to errorCode)
                    finish(null)
                }
            }
            // MIUI silently throttles UNFILTERED scans (results never arrive);
            // one filter per shape makes them get delivered. Device-found in the
            // demo era on a Xiaomi, kept because that lesson was expensive.
            val settings = ScanSettings.Builder()
                .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
                .build()
            val filters = SERVICE_UUIDS.map {
                ScanFilter.Builder().setServiceData(it, ByteArray(0)).build()
            } + SERVICE_UUIDS.map {
                ScanFilter.Builder().setServiceUuid(it).build()
            }
            VelaLog.event("cable.scan", "scanning 0xFFF9/0xFDE2")
            scanner.startScan(filters, settings, cb)
            handler.postDelayed({ finish(null) }, timeoutMs)
            cont.invokeOnCancellation {
                finish(null)
                closeGatt(gattLink.getAndSet(null))
            }
        }

    @SuppressLint("MissingPermission")
    private fun closeGatt(gatt: BluetoothGatt?) {
        if (gatt == null) return
        runCatching { gatt.disconnect() }
        runCatching { gatt.close() }
    }

    /**
     * iOS fallback: the EID (+ suffix) lives on GATT characteristic 0xFFFA.
     * Calls [onDone] exactly once, with a matching hit or null; answers the
     * link it opened, so a cancelled scan can close it.
     */
    @SuppressLint("MissingPermission")
    private fun readAdvertViaGatt(
        device: BluetoothDevice,
        qrSecret: ByteArray,
        onDone: (AdvertHit?) -> Unit,
    ): BluetoothGatt? {
        var settled = false
        fun settle(hit: AdvertHit?, gatt: BluetoothGatt?) {
            if (settled) return
            settled = true
            runCatching { gatt?.disconnect() }
            runCatching { gatt?.close() }
            onDone(hit)
        }
        val cb = object : BluetoothGattCallback() {
            override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
                if (newState == BluetoothProfile.STATE_CONNECTED) {
                    // Android caches the GATT database across connections; a
                    // service the peer only just published would look absent
                    // from a stale cache. The hidden-but-ubiquitous refresh().
                    runCatching { g.javaClass.getMethod("refresh").invoke(g) }
                    if (!g.requestMtu(185)) g.discoverServices()
                } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                    settle(null, g)
                }
            }

            override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) {
                g.discoverServices()
            }

            override fun onServicesDiscovered(g: BluetoothGatt, status: Int) {
                val ch = SERVICE_UUIDS.firstNotNullOfOrNull {
                    g.getService(it.uuid)?.getCharacteristic(PROX_CHAR_UUID)
                }
                if (ch == null || !g.readCharacteristic(ch)) settle(null, g)
            }

            @Suppress("DEPRECATION")
            override fun onCharacteristicRead(g: BluetoothGatt, ch: BluetoothGattCharacteristic, status: Int) {
                if (status != BluetoothGatt.GATT_SUCCESS) {
                    settle(null, g)
                    return
                }
                val bytes = ch.value ?: ByteArray(0)
                if (bytes.size < 20) {
                    settle(null, g)
                    return
                }
                val advert = cableTryDecryptAdvert(qrSecret, bytes)
                if (advert == null) {
                    settle(null, g)
                    return
                }
                VelaLog.event("cable.scan", "GATT matched this QR", "psm" to (advert.psm ?: "none"))
                settle(AdvertHit(device, advert), g)
            }
        }
        return device.connectGatt(appContext, false, cb, BluetoothDevice.TRANSPORT_LE)
    }

    private companion object {
        val SERVICE_UUIDS = listOf(
            ParcelUuid(UUID.fromString("0000fff9-0000-1000-8000-00805f9b34fb")),
            ParcelUuid(UUID.fromString("0000fde2-0000-1000-8000-00805f9b34fb")),
        )
        val PROX_CHAR_UUID: UUID = UUID.fromString("0000fffa-0000-1000-8000-00805f9b34fb")
    }
}
