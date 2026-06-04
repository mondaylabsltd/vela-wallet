package com.velawallet.walletpairble

import android.bluetooth.*
import android.bluetooth.le.*
import android.content.Context
import android.util.Base64
import com.facebook.react.bridge.*
import com.facebook.react.modules.core.DeviceEventManagerModule
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class WalletPairBleModule(reactContext: ReactApplicationContext) : ReactContextBaseJavaModule(reactContext) {

    companion object {
        const val NAME = "WalletPairBle"
        private val CCC_DESCRIPTOR_UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
        private const val TAG = "WalletPairBle"
    }

    override fun getName() = NAME

    private var gattServer: BluetoothGattServer? = null
    private var advertiser: BluetoothLeAdvertiser? = null
    private var currentAdvCallback: AdvertiseCallback? = null
    private var connectedDevice: BluetoothDevice? = null
    private var notifyChar: BluetoothGattCharacteristic? = null
    private var notifyUuid: UUID? = null
    private var negotiatedMtu = 23

    // -------------------------------------------------------------------------
    // Event emission
    // -------------------------------------------------------------------------

    private fun emit(eventName: String, params: WritableMap) {
        reactApplicationContext
            .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
            .emit("WalletPairBle_$eventName", params)
    }

    // -------------------------------------------------------------------------
    // JS API
    // -------------------------------------------------------------------------

    @ReactMethod
    fun start(svcUuid: String, writeUuid: String, notifyUuidStr: String, name: String, promise: Promise) {
        try {
            val ctx = reactApplicationContext
            val manager = ctx.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
            val adapter = manager.adapter ?: throw Exception("Bluetooth not available")

            stopInternal(adapter)

            val svc = BluetoothGattService(UUID.fromString(svcUuid), BluetoothGattService.SERVICE_TYPE_PRIMARY)

            val writeCh = BluetoothGattCharacteristic(
                UUID.fromString(writeUuid),
                BluetoothGattCharacteristic.PROPERTY_WRITE or BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE,
                BluetoothGattCharacteristic.PERMISSION_WRITE)
            svc.addCharacteristic(writeCh)

            val nUuid = UUID.fromString(notifyUuidStr)
            this.notifyUuid = nUuid
            val notifyCh = BluetoothGattCharacteristic(
                nUuid,
                BluetoothGattCharacteristic.PROPERTY_NOTIFY or BluetoothGattCharacteristic.PROPERTY_READ,
                BluetoothGattCharacteristic.PERMISSION_READ)
            val cccDesc = BluetoothGattDescriptor(
                CCC_DESCRIPTOR_UUID,
                BluetoothGattDescriptor.PERMISSION_WRITE or BluetoothGattDescriptor.PERMISSION_READ)
            notifyCh.addDescriptor(cccDesc)
            svc.addCharacteristic(notifyCh)
            notifyChar = notifyCh

            gattServer = manager.openGattServer(ctx, gattCallback)
            gattServer?.addService(svc)

            adapter.name = name

            val adv = adapter.bluetoothLeAdvertiser ?: throw Exception("BLE advertising not supported")
            advertiser = adv

            val settings = AdvertiseSettings.Builder()
                .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
                .setConnectable(true).setTimeout(0).build()
            val data = AdvertiseData.Builder().setIncludeDeviceName(true).build()

            val latch = CountDownLatch(1)
            var advError: String? = null
            val callback = object : AdvertiseCallback() {
                override fun onStartSuccess(s: AdvertiseSettings?) { latch.countDown() }
                override fun onStartFailure(errorCode: Int) {
                    advError = "Advertising failed: code $errorCode"
                    latch.countDown()
                }
            }
            currentAdvCallback = callback
            adv.startAdvertising(settings, data, callback)

            if (!latch.await(5, TimeUnit.SECONDS)) throw Exception("Advertising timed out")
            if (advError != null) { stopInternal(adapter); throw Exception(advError!!) }

            promise.resolve(null)
        } catch (e: Exception) {
            promise.reject("BLE_ERROR", e.message, e)
        }
    }

    @ReactMethod
    fun stop(promise: Promise) {
        try {
            val ctx = reactApplicationContext
            val manager = ctx.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
            val adapter = manager.adapter
            if (adapter != null) stopInternal(adapter)
            promise.resolve(null)
        } catch (e: Exception) {
            promise.reject("BLE_ERROR", e.message, e)
        }
    }

    @ReactMethod
    fun sendBatch(base64Frames: ReadableArray, promise: Promise) {
        val ch = notifyChar
        val server = gattServer
        if (ch == null || server == null) { promise.resolve(null); return }

        for (i in 0 until base64Frames.size()) {
            val device = connectedDevice ?: break
            val bytes = Base64.decode(base64Frames.getString(i), Base64.NO_WRAP)
            ch.value = bytes
            try {
                server.notifyCharacteristicChanged(device, ch, false)
            } catch (e: Exception) {
                android.util.Log.e(TAG, "sendBatch failed: ${e.message}")
                break
            }
        }
        promise.resolve(null)
    }

    // -------------------------------------------------------------------------
    // Internal
    // -------------------------------------------------------------------------

    private fun stopInternal(adapter: BluetoothAdapter) {
        val cb = currentAdvCallback
        if (cb != null) {
            try { adapter.bluetoothLeAdvertiser?.stopAdvertising(cb) } catch (_: Exception) {}
            currentAdvCallback = null
        }
        try { gattServer?.close() } catch (_: Exception) {}
        gattServer = null; advertiser = null; connectedDevice = null; notifyChar = null
        negotiatedMtu = 23
        Thread.sleep(300)
    }

    private val gattCallback = object : BluetoothGattServerCallback() {

        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                connectedDevice = device
                val params = Arguments.createMap().apply { putString("address", device.address) }
                emit("onConnect", params)
            } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                if (connectedDevice?.address == device.address) {
                    connectedDevice = null
                    val params = Arguments.createMap().apply { putString("address", device.address) }
                    emit("onDisconnect", params)
                }
            }
        }

        override fun onMtuChanged(device: BluetoothDevice?, mtu: Int) {
            negotiatedMtu = mtu
            val params = Arguments.createMap().apply { putInt("mtu", mtu) }
            emit("onMtuChanged", params)
        }

        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice, requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean, responseNeeded: Boolean,
            offset: Int, value: ByteArray
        ) {
            if (responseNeeded) {
                gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, 0, null)
            }
            val b64 = Base64.encodeToString(value, Base64.NO_WRAP)
            val params = Arguments.createMap().apply {
                putString("characteristicUuid", characteristic.uuid.toString())
                putString("value", b64)
            }
            emit("onWrite", params)
        }

        override fun onDescriptorWriteRequest(
            device: BluetoothDevice, requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean, responseNeeded: Boolean,
            offset: Int, value: ByteArray
        ) {
            if (responseNeeded) {
                gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, 0, null)
            }
            if (descriptor.uuid == CCC_DESCRIPTOR_UUID) {
                val charUuid = descriptor.characteristic.uuid.toString()
                if (value.contentEquals(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)) {
                    val params = Arguments.createMap().apply { putString("characteristicUuid", charUuid) }
                    emit("onSubscribe", params)
                } else if (value.contentEquals(BluetoothGattDescriptor.DISABLE_NOTIFICATION_VALUE)) {
                    val params = Arguments.createMap().apply { putString("characteristicUuid", charUuid) }
                    emit("onUnsubscribe", params)
                }
            }
        }

        override fun onCharacteristicReadRequest(
            device: BluetoothDevice, requestId: Int,
            offset: Int, characteristic: BluetoothGattCharacteristic
        ) {
            gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset,
                characteristic.value ?: ByteArray(0))
        }
    }
}
