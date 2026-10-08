//
//  HybridCableScanner.swift
//  VelaWallet
//
//  Finds the phone that scanned OUR QR, by its BLE proximity advert — a port of
//  the founder's proven demo (apppasskeysdemo-ios/HybridBleClient.swift), with
//  the crypto moved into vela-core: every candidate goes through
//  cableTryDecryptAdvert, which answers with the decrypted EID and the L2CAP
//  PSM in one step.
//
//  iOS constraint (vs Android's connection-less createInsecureL2capChannel):
//  openL2CAPChannel needs a prior central.connect(peripheral), which requires
//  the responder to advertise CONNECTABLY — Android responders (securitykeys,
//  the demo) do.
//

import Foundation
import CoreBluetooth
import VelaCore

@MainActor
final class HybridCableScanner: NSObject, CBCentralManagerDelegate, CBPeripheralDelegate {

    private static let serviceUUIDs = [CBUUID(string: "FFF9"), CBUUID(string: "FDE2")]

    /// The matched phone: where to connect, and what its advert said.
    struct AdvertHit {
        let peripheral: CBPeripheral
        let advert: CableAdvert
    }

    private var central: CBCentralManager!
    private var qrSecret = Data()

    private var scanCont: CheckedContinuation<AdvertHit?, Never>?
    private var scanWatchdog: Task<Void, Never>?

    private var connectCont: CheckedContinuation<Void, Error>?
    private var openCont: CheckedContinuation<CBL2CAPChannel, Error>?
    private var connectWatchdog: Task<Void, Never>?
    private var connectingPeripheral: CBPeripheral?
    /// `cancel()` was called. One scanner serves one ceremony, so a cancel
    /// that lands between two of its steps still ends it: a scan or a
    /// connect asked for afterwards finds the flag rather than waiting out
    /// its own timeout (issue #459).
    private var cancelled = false

    override init() {
        super.init()
        central = CBCentralManager(delegate: self, queue: nil)
    }

    var isBluetoothReady: Bool { central.state == .poweredOn }

    // MARK: scan

    func findResponder(qrSecret: Data, timeoutMs: Int) async -> AdvertHit? {
        if cancelled { return nil }
        self.qrSecret = qrSecret
        return await withCheckedContinuation { (cont: CheckedContinuation<AdvertHit?, Never>) in
            self.scanCont = cont
            self.startScanIfReady()
            self.scanWatchdog = Task { @MainActor [weak self] in
                try? await Task.sleep(nanoseconds: UInt64(timeoutMs) * 1_000_000)
                self?.finishScan(nil)
            }
        }
    }

    private func startScanIfReady() {
        // Wait for didUpdateState when the radio is still warming up.
        guard central.state == .poweredOn else {
            print("[vela-cable] bluetooth not ready (state=\(central.state.rawValue)); waiting")
            return
        }
        print("[vela-cable] scan started (no service filter, trial-decrypting service data)")
        // `withServices: nil`, NOT the FIDO UUIDs: the caBLE proximity advert
        // carries them inside Service Data (AD type 0x16), and CoreBluetooth's
        // service filter matches only the advertised service-UUID list — it
        // would drop a service-data-only responder. Every advert is delivered
        // and the per-advert trial-decrypt is the real filter.
        central.scanForPeripherals(
            withServices: nil,
            options: [CBCentralManagerScanOptionAllowDuplicatesKey: true]
        )
    }

    private func finishScan(_ hit: AdvertHit?) {
        guard let cont = scanCont else { return }
        scanCont = nil
        scanWatchdog?.cancel(); scanWatchdog = nil
        if central.state == .poweredOn { central.stopScan() }
        cont.resume(returning: hit)
    }

    nonisolated func centralManagerDidUpdateState(_ c: CBCentralManager) {
        MainActor.assumeIsolated {
            print("[vela-cable] bluetooth state -> \(c.state.rawValue)")
            switch c.state {
            case .poweredOn: if scanCont != nil { startScanIfReady() }
            case .unauthorized, .poweredOff:
                print("[vela-cable] bluetooth unavailable/denied — scan over")
                finishScan(nil)
            default: break
            }
        }
    }

    nonisolated func centralManager(
        _ c: CBCentralManager,
        didDiscover p: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        MainActor.assumeIsolated {
            guard let sdMap = advertisementData[CBAdvertisementDataServiceDataKey] as? [CBUUID: Data] else { return }
            for uuid in Self.serviceUUIDs {
                guard let sd = sdMap[uuid], sd.count >= 20 else { continue }
                // The WHOLE payload: bytes past 20 are the CTAP 2.3 BLE suffix
                // the core reads the PSM from.
                guard let advert = cableTryDecryptAdvert(qrSecret: qrSecret, candidate: sd) else {
                    print("[vela-cable] caBLE advert for a DIFFERENT QR (\(sd.count) bytes)")
                    continue
                }
                print("[vela-cable] matched this QR; PSM=\(advert.psm.map(String.init) ?? "none") rssi=\(RSSI)")
                connectingPeripheral = p
                finishScan(AdvertHit(peripheral: p, advert: advert))
                return
            }
        }
    }

    // MARK: connect + open the CTAP 2.3 BLE channel

    /// GATT-connect the matched peripheral and open the L2CAP CoC at `psm`.
    func openL2cap(_ hit: AdvertHit, psm: UInt16, timeoutMs: Int) async throws -> L2capCableConn {
        if cancelled { throw CableConnError.closed }
        let p = hit.peripheral
        p.delegate = self
        connectingPeripheral = p

        // Connect with a watchdog — iOS never times out connect() by itself.
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            self.connectCont = cont
            self.central.connect(p, options: nil)
            self.connectWatchdog = Task { @MainActor [weak self] in
                try? await Task.sleep(nanoseconds: UInt64(timeoutMs) * 1_000_000)
                guard let self, let c = self.connectCont else { return }
                self.connectCont = nil
                self.central.cancelPeripheralConnection(p)
                c.resume(throwing: CableConnError.timeout)
            }
        }
        connectWatchdog?.cancel(); connectWatchdog = nil
        if cancelled { throw CableConnError.closed }
        print("[vela-cable] GATT connected; opening L2CAP CoC (PSM \(psm))")

        let channel = try await withCheckedThrowingContinuation { (cont: CheckedContinuation<CBL2CAPChannel, Error>) in
            self.openCont = cont
            p.openL2CAPChannel(CBL2CAPPSM(psm))
        }
        print("[vela-cable] L2CAP channel open")
        return L2capCableConn(channel, peripheral: p)
    }

    nonisolated func centralManager(_ c: CBCentralManager, didConnect p: CBPeripheral) {
        MainActor.assumeIsolated {
            guard let cont = connectCont else { return }
            connectCont = nil
            cont.resume(returning: ())
        }
    }

    nonisolated func centralManager(_ c: CBCentralManager, didFailToConnect p: CBPeripheral, error: Error?) {
        MainActor.assumeIsolated {
            guard let cont = connectCont else { return }
            connectCont = nil
            cont.resume(throwing: CableConnError.connectFailed(error?.localizedDescription ?? "didFailToConnect"))
        }
    }

    nonisolated func centralManager(_ c: CBCentralManager, didDisconnectPeripheral p: CBPeripheral, error: Error?) {
        MainActor.assumeIsolated {
            if let cont = connectCont { connectCont = nil; cont.resume(throwing: CableConnError.closed) }
            if let cont = openCont { openCont = nil; cont.resume(throwing: CableConnError.closed) }
        }
    }

    nonisolated func peripheral(_ p: CBPeripheral, didOpen channel: CBL2CAPChannel?, error: Error?) {
        MainActor.assumeIsolated {
            guard let cont = openCont else { return }
            openCont = nil
            if let ch = channel {
                cont.resume(returning: ch)
            } else {
                cont.resume(throwing: CableConnError.connectFailed(error?.localizedDescription ?? "openL2CAPChannel failed"))
            }
        }
    }

    /// Stop whatever this scanner is doing — the scan, a connect, an L2CAP
    /// open — and fail what waits on it now rather than at its timeout. A
    /// dismissed code (issue #459) and the ceremony's own cleanup both end
    /// here.
    func cancel() {
        cancelled = true
        finishScan(nil)
        connectWatchdog?.cancel(); connectWatchdog = nil
        if let cont = connectCont { connectCont = nil; cont.resume(throwing: CableConnError.closed) }
        if let cont = openCont { openCont = nil; cont.resume(throwing: CableConnError.closed) }
        if let p = connectingPeripheral { central.cancelPeripheralConnection(p) }
    }
}
