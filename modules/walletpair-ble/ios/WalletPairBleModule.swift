import Foundation
import CoreBluetooth
import React

@objc(WalletPairBle)
class WalletPairBleModule: RCTEventEmitter, CBPeripheralManagerDelegate {

    private var peripheralManager: CBPeripheralManager?
    private var notifyChar: CBMutableCharacteristic?
    private var subscribedCentral: CBCentral?
    private var sendQueue: [Data] = []
    private let lock = NSLock()

    private var svcUuid: CBUUID?
    private var writeUuid: CBUUID?
    private var notifyUuid: CBUUID?
    private var deviceName = "WalletPair"
    private var startResolve: RCTPromiseResolveBlock?
    private var startReject: RCTPromiseRejectBlock?

    override static func moduleName() -> String! { "WalletPairBle" }

    override static func requiresMainQueueSetup() -> Bool { false }

    override func supportedEvents() -> [String]! {
        ["WalletPairBle_onWrite", "WalletPairBle_onSubscribe", "WalletPairBle_onUnsubscribe",
         "WalletPairBle_onConnect", "WalletPairBle_onDisconnect", "WalletPairBle_onMtuChanged"]
    }

    // MARK: - JS API

    @objc func start(_ svcUuidStr: String, writeUuid writeUuidStr: String, notifyUuid notifyUuidStr: String, name: String, resolve: @escaping RCTPromiseResolveBlock, reject: @escaping RCTPromiseRejectBlock) {
        stopInternal()

        self.svcUuid = CBUUID(string: svcUuidStr)
        self.writeUuid = CBUUID(string: writeUuidStr)
        self.notifyUuid = CBUUID(string: notifyUuidStr)
        self.deviceName = name
        self.startResolve = resolve
        self.startReject = reject

        peripheralManager = CBPeripheralManager(delegate: self, queue: nil)
    }

    @objc func stop(_ resolve: RCTPromiseResolveBlock, reject: RCTPromiseRejectBlock) {
        stopInternal()
        resolve(nil)
    }

    @objc func sendBatch(_ base64Frames: [String], resolve: RCTPromiseResolveBlock, reject: RCTPromiseRejectBlock) {
        lock.lock()
        for b64 in base64Frames {
            if let data = Data(base64Encoded: b64) {
                sendQueue.append(data)
            }
        }
        lock.unlock()
        drainQueue()
        resolve(nil)
    }

    // MARK: - Internal

    private func stopInternal() {
        if let pm = peripheralManager {
            if pm.isAdvertising { pm.stopAdvertising() }
            pm.removeAllServices()
            pm.delegate = nil
        }
        peripheralManager = nil
        subscribedCentral = nil
        notifyChar = nil
        startResolve = nil
        startReject = nil
        sendQueue.removeAll()
    }

    private func drainQueue() {
        lock.lock()
        defer { lock.unlock() }
        guard let pm = peripheralManager, let ch = notifyChar, let central = subscribedCentral else { return }
        while !sendQueue.isEmpty {
            let data = sendQueue.first!
            if pm.updateValue(data, for: ch, onSubscribedCentrals: [central]) {
                sendQueue.removeFirst()
            } else {
                return // peripheralManagerIsReady will resume
            }
        }
    }

    // MARK: - CBPeripheralManagerDelegate

    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        guard peripheral.state == .poweredOn else {
            let msg: String
            switch peripheral.state {
            case .unauthorized: msg = "unauthorized"
            case .unsupported: msg = "unsupported"
            case .poweredOff: msg = "powered off"
            default: msg = "unavailable (\(peripheral.state.rawValue))"
            }
            startReject?("BLE_ERROR", "Bluetooth is \(msg)", nil)
            startResolve = nil; startReject = nil
            return
        }

        guard let svc = svcUuid, let wUuid = writeUuid, let nUuid = notifyUuid else { return }

        let writeCh = CBMutableCharacteristic(type: wUuid, properties: [.write, .writeWithoutResponse], value: nil, permissions: [.writeable])
        let notifyCh = CBMutableCharacteristic(type: nUuid, properties: [.notify, .read], value: nil, permissions: [.readable])
        self.notifyChar = notifyCh

        let service = CBMutableService(type: svc, primary: true)
        service.characteristics = [writeCh, notifyCh]
        peripheral.add(service)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        if let error = error {
            startReject?("BLE_ERROR", error.localizedDescription, error)
            startResolve = nil; startReject = nil
            return
        }
        peripheral.startAdvertising([
            CBAdvertisementDataLocalNameKey: deviceName,
            CBAdvertisementDataServiceUUIDsKey: [svcUuid!]
        ])
    }

    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        if let error = error {
            startReject?("BLE_ERROR", error.localizedDescription, error)
        } else {
            startResolve?(nil)
        }
        startResolve = nil; startReject = nil
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didSubscribeTo characteristic: CBCharacteristic) {
        if characteristic.uuid == notifyUuid {
            subscribedCentral = central
            let mtu = central.maximumUpdateValueLength
            sendEvent(withName: "WalletPairBle_onSubscribe", body: ["characteristicUuid": characteristic.uuid.uuidString.lowercased()])
            sendEvent(withName: "WalletPairBle_onMtuChanged", body: ["mtu": mtu])
        }
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didUnsubscribeFrom characteristic: CBCharacteristic) {
        if characteristic.uuid == notifyUuid {
            subscribedCentral = nil
            sendEvent(withName: "WalletPairBle_onUnsubscribe", body: ["characteristicUuid": characteristic.uuid.uuidString.lowercased()])
        }
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        for request in requests {
            peripheral.respond(to: request, withResult: .success)
            if let value = request.value {
                sendEvent(withName: "WalletPairBle_onWrite", body: [
                    "characteristicUuid": request.characteristic.uuid.uuidString.lowercased(),
                    "value": value.base64EncodedString()
                ])
            }
        }
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveRead request: CBATTRequest) {
        request.value = notifyChar?.value ?? Data()
        peripheral.respond(to: request, withResult: .success)
    }

    func peripheralManagerIsReady(toUpdateSubscribers peripheral: CBPeripheralManager) {
        drainQueue()
    }
}
