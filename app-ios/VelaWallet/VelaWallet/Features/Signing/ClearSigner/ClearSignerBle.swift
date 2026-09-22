//
//  ClearSignerBle.swift
//  VelaWallet
//
//  The wallet's end of the Clear Signer's BLE channel (spec 075 T041,
//  `app-web/clearsigning/PROTOCOL.md` §1–4 and §11): the phone as a GATT
//  PERIPHERAL, and a Chrome page on a nearby computer as the central.
//
//  A browser can only ever be a central, and Web Bluetooth exists only in a
//  document — so this channel is always "our native app advertising, their
//  open tab connecting". Unlike the relay there is no server, no room and no
//  pairing link: what stands in for them is PROXIMITY (the attacker has to be
//  in radio range) and the six digits the person compares.
//
//  ## What is here, and what is emphatically not
//
//  Not here: the framing and the session. Both are the CORE's —
//  `ClearSignerFramer` / `ClearSignerReassembler` (the six-byte header, the
//  `msgId` that wraps, the out-of-order reassembly and its sweep) and
//  `ClearSignerHandshake` / `ClearSignerSession` (P-256 ECDH, HKDF-SHA256,
//  AES-GCM, the six digits). Three shells run this peripheral side and the
//  page is the one central they all talk to; three hand-written reassemblers
//  would be three chances to disagree about a wrapped id. Nothing in this file
//  builds a frame header, derives a key or computes a code.
//
//  Here: the radio and the order of things —
//
//  - the GATT service and its two characteristics, built from
//    `clearSignerBleUuids()`, and an advertisement that carries the service
//    uuid (Chrome's chooser filters on it) and a human-readable local name;
//  - the handshake's order: the central writes its hello in the CLEAR, we
//    answer ours in the clear, and only then is there a session;
//  - the rule that **nothing sealed goes out until the person says the two
//    screens show the same six digits**. On BLE that comparison is the only
//    thing standing between this wallet and a page in the middle;
//  - a session of several requests, ending in `bye` (§11);
//  - the outbound queue: an intent is a dozen notifications and
//    `updateValue` refuses when the controller's queue is full, so frames wait
//    for `peripheralManagerIsReady`.
//
//  ## The foreground, honestly
//
//  PROTOCOL.md §1: once an iOS app is backgrounded its advertisement loses the
//  local name and the service uuid moves into the overflow area, where a
//  desktop scanner effectively cannot see it. There is no flag that fixes
//  this. So a session needs the app on screen, and this class says so rather
//  than letting the page look broken: it watches the scene notifications,
//  reports `backgrounded` while the app is away, and re-advertises on the way
//  back so the name returns for anybody still looking.
//
//  ## Testing without a radio
//
//  A simulator has no usable BLE peripheral stack. So `CBPeripheralManager`
//  sits behind `ClearSignerBleRadio`, and the delegate callbacks that carry
//  un-constructible CoreBluetooth types (`CBATTRequest`, `CBCentral`) are
//  one-line adapters onto `received(_:nowMs:)` and
//  `subscribed(maximumUpdateValueLength:)`, which
//  take plain values a test can build.
//
//  ## The MTU, and why the default chunk is not it
//
//  `DEFAULT_CHUNK` (244) is what the PAGE writes with, and an oversized write
//  from a central is split into a long write by the OS. A peripheral's notify
//  cannot be split: `updateValue` truncates at the link's maximum, and a
//  truncated frame is a message that never completes. So this end calls the
//  core's `fitToMtu` the moment a central subscribes rather than trusting the
//  default — `subscribed(maximumUpdateValueLength:)`, which is also where
//  CoreBluetooth's units are converted to the ATT MTU the core asks for.
//

import CoreBluetooth
import Foundation
import UIKit
import VelaCore

/// What this conversation needs of `CBPeripheralManager`. The seam exists so
/// a session can be driven end to end with no radio at all.
protocol ClearSignerBleRadio: AnyObject {
    var radioState: CBManagerState { get }
    func add(_ service: CBMutableService)
    func removeAllServices()
    func startAdvertising(_ advertisementData: [String: Any]?)
    func stopAdvertising()
    /// `false` when the controller's queue is full — the frame must wait for
    /// `peripheralManagerIsReady(toUpdateSubscribers:)`.
    func updateValue(
        _ value: Data, for characteristic: CBMutableCharacteristic,
        onSubscribedCentrals centrals: [CBCentral]?
    ) -> Bool
}

/// `CBPeripheralManager` on the main queue — which is where this class lives,
/// so every callback lands where its state already is.
final class LiveBleRadio: ClearSignerBleRadio {
    private let manager: CBPeripheralManager

    init(delegate: CBPeripheralManagerDelegate) {
        // `queue: nil` is the main queue. Asking for the peripheral role is
        // what triggers the system's Bluetooth prompt, which is why
        // `NSBluetoothAlwaysUsageDescription` has to name this use too.
        manager = CBPeripheralManager(delegate: delegate, queue: nil)
    }

    var radioState: CBManagerState { manager.state }
    func add(_ service: CBMutableService) { manager.add(service) }
    func removeAllServices() { manager.removeAllServices() }
    func startAdvertising(_ advertisementData: [String: Any]?) {
        manager.startAdvertising(advertisementData)
    }
    func stopAdvertising() { manager.stopAdvertising() }
    func updateValue(
        _ value: Data, for characteristic: CBMutableCharacteristic,
        onSubscribedCentrals centrals: [CBCentral]?
    ) -> Bool {
        manager.updateValue(value, for: characteristic, onSubscribedCentrals: centrals)
    }
}

/// Why a nearby session cannot run. Each one is a CARD that names what is
/// missing — a person who turned Bluetooth off, or said no to the prompt,
/// must not be shown a spinner that never resolves.
enum ClearSignerBleTrouble: Equatable {
    /// The person refused Bluetooth, or a restriction refuses it for them.
    case notAuthorized
    /// Bluetooth is switched off.
    case poweredOff
    /// No peripheral role here at all — a simulator, mostly.
    case unsupported
    /// The service or the advertisement would not come up.
    case unavailable

    /// What the card says. A person whose Bluetooth is merely switched off
    /// must not be told they refused a permission — they would go looking for
    /// a settings screen with nothing in it to change — so the two cases keep
    /// their own sentence, and the permission one says WHY it is wanted rather
    /// than only that it is missing.
    var titleKey: String { I18nKeys.ClearSigner.nearby }

    var bodyKey: String {
        self == .notAuthorized
            ? I18nKeys.ClearSigner.bluetoothNeeded
            : I18nKeys.ClearSigner.bluetoothOff
    }
}

/// One write the central made on `c2p`, as this class needs it — a plain
/// value, so a test can build one without a `CBATTRequest`.
struct ClearSignerBleWrite: Equatable {
    let central: UUID
    let value: Data

    init(central: UUID = UUID(), value: Data) {
        self.central = central
        self.value = value
    }
}

/// One nearby session: the advert, the handshake, the six digits, and the
/// requests that follow them.
final class ClearSignerBleConversation: NSObject, ClearSignerConversation, CBPeripheralManagerDelegate {

    /// The page every answer's origin is checked against. On BLE the person
    /// opens the page themselves, so this is the address Settings names.
    let signerUrl: String
    /// What the device chooser shows beside the wallet — a person has to
    /// recognise their own phone in it.
    let localName: String

    private let app: String
    private let framer = ClearSignerFramer()
    private let reassembler = ClearSignerReassembler()
    private let handshake: ClearSignerHandshake
    private let makeRadio: (CBPeripheralManagerDelegate) -> ClearSignerBleRadio
    private let nextRequestId: () -> String
    private let clock: () -> UInt64

    private var radio: ClearSignerBleRadio?
    private var session: ClearSignerSession?
    private let c2p: CBMutableCharacteristic
    private let p2c: CBMutableCharacteristic

    /// Frames handed to the controller but not yet accepted by it.
    private var outbound: [Data] = []
    /// The session's `n`: the larger of what we sent and what we saw, plus one
    /// (PROTOCOL.md §4).
    private var counter: UInt64 = 0
    /// The person has said the two screens agree. Until then this end sends
    /// nothing sealed — that is the whole defence on this channel.
    private(set) var confirmed = false
    private(set) var advertising = false
    private(set) var ended = false
    /// Why there is no session, when there is none.
    private(set) var trouble: ClearSignerBleTrouble?
    /// The app is off screen, so the advert has lost its local name and the
    /// service uuid has moved to the overflow area (PROTOCOL.md §1).
    private(set) var backgrounded = false
    /// Told when `backgrounded` changes, so the sheet can say so.
    var onForegroundChanged: ((Bool) -> Void)?

    /// The request in flight, and the id its answer must carry.
    private var inflight: (id: String, ask: ClearSignerAsk)?
    private var answering: CheckedContinuation<ClearSignerChannel.Ending, Never>?
    /// A verdict that arrived before anybody was waiting for it.
    private var pendingEnding: ClearSignerChannel.Ending?
    private var pairingWaiter: CheckedContinuation<String?, Never>?
    private var observers: [NSObjectProtocol] = []

    /// `nil` when the handshake key could not be drawn — a ~2⁻³² event the
    /// caller retries, or falls back to another route.
    init?(
        signerUrl: String,
        app: String = ClearSignerRelayConversation.appName,
        localName: String = ClearSignerBleConversation.deviceName(),
        random: (Int) -> Data = ClearSignerRelayConversation.randomBytes,
        makeRadio: @escaping (CBPeripheralManagerDelegate) -> ClearSignerBleRadio = { LiveBleRadio(delegate: $0) },
        nextId: @escaping () -> String = { UUID().uuidString.lowercased() },
        clock: @escaping () -> UInt64 = ClearSignerBleConversation.nowMs
    ) {
        guard let handshake = try? ClearSignerHandshake(secret: random(32), nonce: random(16))
        else { return nil }
        // The uuids are the CORE's, read rather than retyped: a wrong digit is
        // a wallet that never appears in Chrome's chooser, which is the
        // hardest possible way to find a typo.
        let uuids = clearSignerBleUuids()
        guard uuids.count == 3 else { return nil }
        self.signerUrl = signerUrl
        self.app = app
        self.localName = localName
        self.handshake = handshake
        self.makeRadio = makeRadio
        self.nextRequestId = nextId
        self.clock = clock
        self.serviceUuid = CBUUID(string: uuids[0])
        self.c2p = CBMutableCharacteristic(
            type: CBUUID(string: uuids[1]),
            properties: [.write, .writeWithoutResponse],
            value: nil,
            permissions: [.writeable]
        )
        self.p2c = CBMutableCharacteristic(
            type: CBUUID(string: uuids[2]),
            properties: [.notify, .read],
            value: nil,
            permissions: [.readable]
        )
        super.init()
    }

    private let serviceUuid: CBUUID

    /// `Vela · <device>`. iOS 16 and later hand every app a generic
    /// `UIDevice.name` ("iPhone") unless it holds the user-assigned-device-name
    /// entitlement, so this is often just "Vela · iPhone" — still enough to
    /// tell our advert from the rest of the chooser, and never a name we made
    /// up for somebody's phone.
    static func deviceName() -> String {
        let name = UIDevice.current.name
        // The advertisement packet is 31 bytes and the service uuid takes 18
        // of them, so a long name would simply be cut by the controller.
        return "Vela · " + String(name.prefix(12))
    }

    /// Milliseconds, for the reassembler's clock. The core has none of its
    /// own — every machine in that crate takes time as an argument.
    static func nowMs() -> UInt64 {
        UInt64(Date().timeIntervalSince1970 * 1000)
    }

    // MARK: - Advertising, and the handshake

    /// Bring the radio up, advertise, and wait for the page to connect and say
    /// hello. Answers the six digits both screens must show, or `nil` with
    /// `trouble` saying why there are none.
    func advertise() async -> String? {
        guard !ended else { return nil }
        watchTheForeground()
        radio = makeRadio(self)
        return await withCheckedContinuation { continuation in
            pairingWaiter = continuation
            // The radio may already be powered on, in which case no state
            // callback is coming and this is the only place that starts.
            start()
        }
    }

    /// The radio's state settled (or was already settled). Everything the
    /// peripheral does begins here.
    func start() {
        guard let radio, !advertising, !ended, session == nil else { return }
        switch radio.radioState {
        case .poweredOn:
            break
        case .unauthorized:
            return finishPairing(nil, trouble: .notAuthorized)
        case .poweredOff:
            return finishPairing(nil, trouble: .poweredOff)
        case .unsupported:
            return finishPairing(nil, trouble: .unsupported)
        default:
            // `.unknown` / `.resetting`: the state callback comes back.
            return
        }
        advertising = true
        let service = CBMutableService(type: serviceUuid, primary: true)
        service.characteristics = [c2p, p2c]
        radio.removeAllServices()
        // Advertising waits for `didAdd`: a central that arrived first would
        // find a service that is not there yet.
        radio.add(service)
    }

    private func beginAdvertising() {
        guard let radio, !ended else { return }
        radio.startAdvertising([
            // Chrome's chooser filters on the service uuid, so it MUST be in
            // the advert; the name is what a person recognises.
            CBAdvertisementDataServiceUUIDsKey: [serviceUuid],
            CBAdvertisementDataLocalNameKey: localName,
        ])
    }

    /// The service is in the GATT database — or would not go in, and there is
    /// nothing for a central to connect to.
    func serviceAdded(error: (any Error)?) {
        guard error == nil else { return finishPairing(nil, trouble: .unavailable) }
        beginAdvertising()
    }

    /// The advertisement is on the air, or it is not.
    func advertisingStarted(error: (any Error)?) {
        guard error != nil else { return }
        advertising = false
        finishPairing(nil, trouble: .unavailable)
    }

    /// The central stopped listening — the tab closed, or the link dropped.
    func unsubscribed() {
        // A page that went away holding a request is a page closed without
        // signing, not a channel fault.
        guard let inflight else { return }
        deliver(ClearSignerChannel.declined(for: inflight.ask))
    }

    /// The controller will take more notifications.
    func ready() { pump() }

    private func finishPairing(_ code: String?, trouble: ClearSignerBleTrouble?) {
        self.trouble = trouble
        guard let continuation = pairingWaiter else { return }
        pairingWaiter = nil
        continuation.resume(returning: code)
    }

    /// The person says the two screens show the same six digits. Nothing
    /// sealed has left this phone before this.
    func confirm() {
        confirmed = true
    }

    // MARK: - The foreground (PROTOCOL.md §1)

    private func watchTheForeground() {
        guard observers.isEmpty else { return }
        let centre = NotificationCenter.default
        observers = [
            centre.addObserver(
                forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated { self?.foreground(false) }
            },
            centre.addObserver(
                forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated { self?.foreground(true) }
            },
        ]
    }

    /// Backgrounded, the advert keeps neither its local name nor a scannable
    /// service uuid. Nothing here can change that — so the state is reported,
    /// and on the way back the advert is restarted so the name returns for
    /// anybody still looking at a chooser.
    private func foreground(_ onScreen: Bool) {
        guard backgrounded == onScreen else { return }
        backgrounded = !onScreen
        onForegroundChanged?(backgrounded)
        guard onScreen, advertising, !ended, let radio else { return }
        radio.stopAdvertising()
        beginAdvertising()
    }

    // MARK: - Frames in

    /// Writes on `c2p`, in the order the controller delivered them. The answer
    /// is what the peripheral responds to the central.
    @discardableResult
    func received(_ writes: [ClearSignerBleWrite], nowMs: UInt64? = nil) -> CBATTError.Code {
        // A session that is over does not quietly swallow more bytes: the page
        // is told, and stops.
        guard !ended else { return .requestNotSupported }
        let now = nowMs ?? clock()
        for write in writes {
            guard let message = reassembler.accept(frame: write.value, nowMs: now) else { continue }
            handle(message)
        }
        // §2: a message that never completes is dropped rather than waited
        // out. The only thing this end is ever waiting for is the answer to
        // its own request, so a dropped one ENDS that request — otherwise a
        // page whose last frame went missing would leave a spinner up with no
        // clock behind it, since nothing else on this channel has one.
        if !reassembler.sweep(nowMs: now).isEmpty, inflight != nil {
            deliver(.timedOut)
        }
        return .success
    }

    /// A central subscribed to `p2c`, and the link's size is finally known.
    ///
    /// CoreBluetooth reports the NOTIFICATION capacity — the ATT MTU less the
    /// opcode and the handle — and the core's `fitToMtu` takes the MTU itself,
    /// so the three bytes go back on here. This conversion is the whole reason
    /// the method is named in CoreBluetooth's units: getting it wrong by three
    /// makes `updateValue` truncate the last frame, and a truncated frame is a
    /// message that never completes and is swept ten seconds later.
    func subscribed(maximumUpdateValueLength: Int) {
        _ = framer.fitToMtu(mtu: UInt32(clamping: maximumUpdateValueLength + 3))
        pump()
    }

    private func handle(_ message: ClearSignerBleMessage) {
        guard message.sealed else { return hello(message) }
        opened(message)
    }

    /// The page's hello, in the clear (§3 step 1). Ours goes back in the clear
    /// (step 2), and the session derives from the two.
    private func hello(_ message: ClearSignerBleMessage) {
        guard session == nil, !ended else { return }
        let text = String(decoding: message.payload, as: UTF8.self)
        guard let json = ClearSignerAnswer.json(text), json["t"] as? String == "hello" else { return }
        // Ours FIRST: `complete` consumes the handshake, and the page cannot
        // derive anything without our key.
        emit(Data(handshake.hello(app: app).utf8), sealed: false)
        guard let session = try? handshake.complete(peerHello: text, relay: false) else {
            return finishPairing(nil, trouble: .unavailable)
        }
        self.session = session
        finishPairing(session.code(), trouble: nil)
    }

    /// A sealed message. Nothing is opened before the person vouched for the
    /// code: until then the only thing that could be on this wire is a page
    /// nobody has checked.
    private func opened(_ message: ClearSignerBleMessage) {
        guard confirmed, let session else { return }
        guard let plain = try? session.open(sealed: message.payload, msgId: message.msgId),
              let json = ClearSignerAnswer.json(String(decoding: plain, as: UTF8.self))
        else { return }
        if let n = (json["n"] as? NSNumber)?.uint64Value, n > counter { counter = n }
        if json["t"] as? String == "bye" {
            // The page hung up. Anything it was holding was closed without
            // signing (§11).
            if let inflight { deliver(ClearSignerChannel.declined(for: inflight.ask)) }
            ended = true
            teardown()
            return
        }
        guard let inflight, json["id"] as? String == inflight.id else { return }
        if let verdict = ClearSignerAnswer.verdict(json, ask: inflight.ask, signerUrl: signerUrl) {
            deliver(verdict)
        }
    }

    // MARK: - Frames out

    /// One message, framed by the core and queued for the controller.
    private func emit(_ payload: Data, sealed: Bool) {
        // The id is taken BEFORE sealing: the session seals it into the AAD
        // (`Tail::MsgId`) and the frames must carry the very same one.
        let msgId = framer.nextId()
        let body = sealed ? (session?.seal(plaintext: payload, msgId: msgId) ?? Data()) : payload
        outbound.append(contentsOf: framer.frames(msgId: msgId, payload: body, sealed: sealed))
        pump()
    }

    /// Hand the controller as many frames as it will take. The rest wait for
    /// `peripheralManagerIsReady` — a batch intent is a dozen notifications
    /// and the queue is shallow.
    private func pump() {
        guard let radio else { return }
        while let frame = outbound.first {
            guard radio.updateValue(frame, for: p2c, onSubscribedCentrals: nil) else { return }
            outbound.removeFirst()
        }
    }

    /// Frames still waiting for the controller. For a test's own assertions.
    var queued: Int { outbound.count }

    // MARK: - A session of several requests (§11)

    func begin(_ ask: ClearSignerAsk) async -> ClearSignerChannel.Ending {
        await send(ask)
    }

    func send(_ ask: ClearSignerAsk) async -> ClearSignerChannel.Ending {
        // A person who never said the codes match has declined, whatever else
        // has happened to the radio since.
        guard confirmed else { return ClearSignerChannel.declined(for: ask) }
        guard !ended, session != nil else { return .unavailable }
        let id = nextRequestId()
        guard let intent = ClearSignerAnswer.intent(ask, id: id, n: next()) else { return .unavailable }
        // In flight BEFORE the first frame goes out, not after: the answer can
        // arrive the instant the last one lands, and a request that was not
        // yet "in flight" would have its own answer dropped on the floor.
        inflight = (id, ask)
        emit(intent, sealed: true)
        return await withCheckedContinuation { continuation in
            if let ending = pendingEnding {
                pendingEnding = nil
                continuation.resume(returning: ending)
            } else {
                answering = continuation
            }
        }
    }

    /// The flow is over: a sealed `bye`, then the radio comes down.
    func end() {
        guard !ended else { return }
        ended = true
        if session != nil, let bye = ClearSignerAnswer.body([
            "v": 1, "t": "bye", "n": next(), "reason": "done",
        ]) {
            emit(bye, sealed: true)
        }
        // The teardown waits a turn so the farewell reaches the controller —
        // removing the service first would take the notification with it.
        Task { @MainActor [weak self] in self?.teardown() }
    }

    /// The person closed the sheet, or left the flow.
    func cancel() { end() }

    private func teardown() {
        radio?.stopAdvertising()
        radio?.removeAllServices()
        radio = nil
        advertising = false
        outbound.removeAll()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers.removeAll()
        finishPairing(nil, trouble: trouble)
        let waiter = answering
        answering = nil
        waiter?.resume(returning: .unavailable)
    }

    private func next() -> UInt64 {
        counter += 1
        return counter
    }

    private func deliver(_ ending: ClearSignerChannel.Ending) {
        inflight = nil
        if let continuation = answering {
            answering = nil
            continuation.resume(returning: ending)
        } else {
            pendingEnding = ending
        }
    }

    // MARK: - CBPeripheralManagerDelegate
    //
    // Adapters, all of them: each one turns a CoreBluetooth type into a plain
    // value and calls the method above, which is the one a test drives.

    nonisolated func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        MainActor.assumeIsolated { self.start() }
    }

    nonisolated func peripheralManager(
        _ peripheral: CBPeripheralManager, didAdd service: CBService, error: (any Error)?
    ) {
        MainActor.assumeIsolated { self.serviceAdded(error: error) }
    }

    nonisolated func peripheralManagerDidStartAdvertising(
        _ peripheral: CBPeripheralManager, error: (any Error)?
    ) {
        MainActor.assumeIsolated { self.advertisingStarted(error: error) }
    }

    nonisolated func peripheralManager(
        _ peripheral: CBPeripheralManager, central: CBCentral,
        didSubscribeTo characteristic: CBCharacteristic
    ) {
        MainActor.assumeIsolated {
            self.subscribed(maximumUpdateValueLength: central.maximumUpdateValueLength)
        }
    }

    nonisolated func peripheralManager(
        _ peripheral: CBPeripheralManager, central: CBCentral,
        didUnsubscribeFrom characteristic: CBCharacteristic
    ) {
        MainActor.assumeIsolated { self.unsubscribed() }
    }

    nonisolated func peripheralManager(
        _ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]
    ) {
        MainActor.assumeIsolated {
            let verdict = self.received(requests.map {
                ClearSignerBleWrite(central: $0.central.identifier, value: $0.value ?? Data())
            })
            // One response for the batch, on the first request — CoreBluetooth
            // requires exactly that.
            if let first = requests.first { peripheral.respond(to: first, withResult: verdict) }
        }
    }

    nonisolated func peripheralManager(
        _ peripheral: CBPeripheralManager, didReceiveRead request: CBATTRequest
    ) {
        MainActor.assumeIsolated {
            // `p2c` is `read` as well as `notify` because PROTOCOL.md §1 says
            // so, but the page subscribes and never reads; a reader gets the
            // frame at the head of the queue rather than a lie.
            request.value = self.outbound.first ?? Data()
            peripheral.respond(to: request, withResult: .success)
        }
    }

    nonisolated func peripheralManagerIsReady(toUpdateSubscribers peripheral: CBPeripheralManager) {
        MainActor.assumeIsolated { self.ready() }
    }
}
