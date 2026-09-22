//
//  ClearSignerBleTests.swift
//  VelaWalletTests
//
//  Spec 075 T041: the phone as the Clear Signer's BLE peripheral.
//
//  **No radio was used, and none could be.** A simulator has no usable BLE
//  peripheral stack — `CBPeripheralManager` there never reaches `.poweredOn`,
//  so there is nothing to advertise from and nothing to connect to it. Every
//  test below drives `ClearSignerBleConversation` through the same entry
//  points its `CBPeripheralManagerDelegate` methods forward to, with
//  `StubBleRadio` standing in for the controller. Those delegate methods take
//  `CBPeripheralManager`, `CBATTRequest` and `CBCentral`, none of which can be
//  constructed outside CoreBluetooth; they are four-line adapters, and the
//  real-radio pass (T043) is where they are exercised.
//
//  What IS real here: the framing and the session. Both are the core's, and
//  both are held to the vectors the page's own JavaScript reads —
//  `tests/clear-signer/ble-frames.json` and the `ble-a` case of
//  `tests/clear-signer/secure-session.json`. A shell that drove the session in
//  the wrong role, under the relay's label, or with the counter instead of the
//  frame's `msgId` would compile perfectly and produce a channel no page can
//  read; these tests are what catches that.
//

import CoreBluetooth
import Foundation
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

// MARK: - A controller that is not there

/// `CBPeripheralManager`, stubbed. It records what the peripheral asked of the
/// radio and — when `automatic` — plays the part of a central that connects,
/// subscribes and writes.
@MainActor
final class StubBleRadio: ClearSignerBleRadio {
    var radioState: CBManagerState
    /// Set through the conversation's `makeRadio`, so the stub can answer it.
    weak var conversation: ClearSignerBleConversation?

    private(set) var services: [CBMutableService] = []
    private(set) var adverts: [[String: Any]] = []
    /// Every frame handed to the controller, in order.
    private(set) var notified: [Data] = []
    private(set) var stopAdvertisingCalls = 0
    private(set) var removeServicesCalls = 0

    /// Whether the stub answers `add`/`startAdvertising` with the callbacks a
    /// real controller would. A test that wants a failure drives them itself.
    var automatic = true
    /// What `central.maximumUpdateValueLength` would say — the NOTIFICATION
    /// capacity, which is the ATT MTU less the opcode and handle. 185 is the
    /// ATT MTU 188 that a Mac and an iPhone commonly settle on.
    var maximumUpdateValueLength = 185
    /// Frames the "page" writes as soon as it has subscribed.
    var script: [Data] = []
    /// How many notifications the controller takes before it reports its queue
    /// full. `nil` is an endless queue.
    var acceptsBeforeFull: Int?

    init(state: CBManagerState = .poweredOn) { radioState = state }

    func add(_ service: CBMutableService) {
        services.append(service)
        guard automatic else { return }
        conversation?.serviceAdded(error: nil)
    }

    func removeAllServices() {
        removeServicesCalls += 1
        services.removeAll()
    }

    func startAdvertising(_ advertisementData: [String: Any]?) {
        adverts.append(advertisementData ?? [:])
        guard automatic else { return }
        conversation?.advertisingStarted(error: nil)
        conversation?.subscribed(maximumUpdateValueLength: maximumUpdateValueLength)
        for frame in script {
            conversation?.received([ClearSignerBleWrite(value: frame)])
        }
    }

    func stopAdvertising() { stopAdvertisingCalls += 1 }

    func updateValue(
        _ value: Data, for characteristic: CBMutableCharacteristic,
        onSubscribedCentrals centrals: [CBCentral]?
    ) -> Bool {
        if let cap = acceptsBeforeFull, notified.count >= cap { return false }
        notified.append(value)
        return true
    }

    /// What the peripheral sent, put back together by the core's own
    /// reassembler — which is exactly what the page does with it.
    func messages() -> [ClearSignerBleMessage] {
        let reassembler = ClearSignerReassembler()
        return notified.compactMap { reassembler.accept(frame: $0, nowMs: 0) }
    }
}

// MARK: - The shared vectors

/// The `ble-a` case of `tests/clear-signer/secure-session.json` — the same
/// bytes `node samples/secure-vectors.mjs` runs the page against.
struct BleVector {
    let code: String
    let signerHello: String
    let requesterHello: String
    let requesterSecret: Data
    let requesterNonce: Data
    let requesterPublicKey: Data
    let app: String
    /// `(fromRequester, msgId, plaintext, sealed)` in session order.
    let messages: [(fromRequester: Bool, msgId: UInt8, plaintext: Data, sealed: Data)]

    static let ble: BleVector? = {
        guard let json = load("secure-session.json"),
              let item = (json["cases"] as? [[String: Any]] ?? [])
                  .first(where: { $0["label"] as? String == "vela-ble/1" })
        else { return nil }
        let requester = item["requester"] as? [String: Any] ?? [:]
        let signer = item["signer"] as? [String: Any] ?? [:]
        return BleVector(
            code: item["code"] as? String ?? "",
            signerHello: signer["hello"] as? String ?? "",
            requesterHello: requester["hello"] as? String ?? "",
            requesterSecret: unhex(requester["secretHex"] as? String ?? ""),
            requesterNonce: unhex(requester["nonceHex"] as? String ?? ""),
            requesterPublicKey: unhex(requester["publicKeyHex"] as? String ?? ""),
            app: requester["app"] as? String ?? "",
            messages: (item["messages"] as? [[String: Any]] ?? []).map { message in
                (
                    fromRequester: message["from"] as? String == "requester",
                    msgId: UInt8((message["msgId"] as? NSNumber)?.intValue ?? 0),
                    plaintext: Data((message["plaintext"] as? String ?? "").utf8),
                    sealed: unhex(message["sealedHex"] as? String ?? "")
                )
            }
        )
    }()

    /// The page's messages, in order — what a central writes on `c2p`.
    var fromPage: [(msgId: UInt8, sealed: Data)] {
        messages.filter { !$0.fromRequester }.map { ($0.msgId, $0.sealed) }
    }

    /// Its own, rather than `UserOpSpine.unhex`: these vectors load in a
    /// static initialiser, which is nonisolated, and that one is not.
    static func unhex(_ text: String) -> Data {
        let digits = Array(text.hasPrefix("0x") ? String(text.dropFirst(2)) : text)
        guard digits.count.isMultiple(of: 2) else { return Data() }
        return Data(stride(from: 0, to: digits.count, by: 2).compactMap {
            UInt8(String(digits[$0...$0 + 1]), radix: 16)
        })
    }

    /// One of the repository's shared Clear Signer vector files.
    static func load(_ name: String) -> [String: Any]? {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // VelaWalletTests
            .deletingLastPathComponent()   // app-ios/VelaWallet
            .deletingLastPathComponent()   // app-ios
            .deletingLastPathComponent()   // repo root
            .appendingPathComponent("rust/crates/vela-core/tests/clear-signer/\(name)")
        guard let data = try? Data(contentsOf: url) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }
}

/// `tests/clear-signer/ble-frames.json` — the framing the page's JS is pinned
/// to as well (`samples/ble-vectors-test.mjs`).
struct BleFrameVector {
    let name: String
    let chunk: Int
    let sealed: Bool
    let msgId: UInt8
    let payload: Data
    let frames: [Data]

    static let all: [BleFrameVector] = {
        guard let json = BleVector.load("ble-frames.json") else { return [] }
        return (json["cases"] as? [[String: Any]] ?? []).map { item in
            BleFrameVector(
                name: item["name"] as? String ?? "",
                chunk: (item["chunk"] as? NSNumber)?.intValue ?? 0,
                sealed: (item["flags"] as? NSNumber)?.intValue == 1,
                msgId: UInt8((item["msgId"] as? NSNumber)?.intValue ?? 0),
                payload: BleVector.unhex(item["payload"] as? String ?? ""),
                frames: (item["frames"] as? [String] ?? []).map(BleVector.unhex)
            )
        }
    }()
}

// MARK: - The framing, which is the core's

@MainActor
struct ClearSignerBleFramingTests {

    /// The uuids this peripheral serves are the ones the core hands out, and
    /// the ones the page filters its chooser on. Three constants, one place: a
    /// wrong digit is a wallet that never appears in the chooser, which is the
    /// hardest possible way to find a typo.
    @Test func theGattIdentifiersAreTheCores() {
        let uuids = clearSignerBleUuids()
        #expect(uuids.count == 3)
        #expect(uuids[0] == "76656c61-0001-4000-8000-00805f9b34fb")
        #expect(uuids[1] == "76656c61-0002-4000-8000-00805f9b34fb")
        #expect(uuids[2] == "76656c61-0003-4000-8000-00805f9b34fb")
        // `76656c61` is `vela` in ASCII — what makes them readable in a packet
        // capture, and what a typo quietly destroys.
        #expect(uuids.allSatisfy { $0.hasPrefix("76656c61-") })
    }

    /// The exported framer cuts a message exactly as the vectors say, at the
    /// default chunk and at the floor. These are the bytes the page's own
    /// `samples/ble-vectors-test.mjs` checks, so a difference here is a
    /// difference with the page.
    @Test func theFramerMatchesTheSharedVectors() {
        #expect(!BleFrameVector.all.isEmpty, "the shared frame vectors did not load")
        for vector in BleFrameVector.all {
            let framer = ClearSignerFramer()
            while Int(framer.chunk()) > vector.chunk {
                #expect(framer.halve(), "\(vector.name): the chunk could not reach \(vector.chunk)")
            }
            let frames = framer.frames(
                msgId: vector.msgId, payload: vector.payload, sealed: vector.sealed
            )
            #expect(frames == vector.frames, "\(vector.name) is not framed the way the page frames it")
        }
    }

    /// And the reassembler puts every one of them back.
    @Test func theReassemblerPutsTheVectorsBackTogether() {
        #expect(!BleFrameVector.all.isEmpty)
        for vector in BleFrameVector.all {
            let reassembler = ClearSignerReassembler()
            var whole: ClearSignerBleMessage?
            for frame in vector.frames {
                whole = reassembler.accept(frame: frame, nowMs: 0) ?? whole
            }
            #expect(whole?.payload == vector.payload, "\(vector.name) did not survive reassembly")
            #expect(whole?.msgId == vector.msgId)
            #expect(whole?.sealed == vector.sealed)
            #expect(reassembler.pending() == 0)
        }
    }
}

// MARK: - The session, which is also the core's

@MainActor
struct ClearSignerBleSessionTests {

    /// The wallet's side of the BLE session, byte for byte against the vectors
    /// the page is pinned to: the requester's key, the six digits both screens
    /// show, and every message sealed and opened under the `msgId` its FRAMES
    /// carry rather than the relay's counter.
    @Test func theBleSessionMatchesTheSharedVectors() throws {
        let vector = try #require(BleVector.ble, "the shared session vectors did not load")
        let handshake = try ClearSignerHandshake(
            secret: vector.requesterSecret, nonce: vector.requesterNonce
        )
        #expect(handshake.publicKey() == vector.requesterPublicKey)
        let ours = try CoreJSON.object(handshake.hello(app: vector.app))
        let theirs = try CoreJSON.object(vector.requesterHello)
        #expect(ours["pk"] as? String == theirs["pk"] as? String)
        #expect(ours["nonce"] as? String == theirs["nonce"] as? String)
        #expect(ours["role"] as? String == "requester", "the wallet asks; the page signs")

        // `relay: false` is the whole difference: it picks `vela-ble/1` rather
        // than `vela-relay/1`, and the two derive different keys from the same
        // ECDH. A shell that passed `true` here would show six digits the page
        // never shows, and the person would be told to compare them.
        let session = try handshake.complete(peerHello: vector.signerHello, relay: false)
        #expect(session.code() == vector.code, "the two screens would show different digits")
        for message in vector.messages {
            if message.fromRequester {
                #expect(
                    session.seal(plaintext: message.plaintext, msgId: message.msgId) == message.sealed
                )
            } else {
                #expect(
                    try session.open(sealed: message.sealed, msgId: message.msgId) == message.plaintext
                )
            }
        }
    }

    /// Sealing under the WRONG id will not open — which is what makes the
    /// `msgId` more than bookkeeping. It is in the AAD, so the id a message is
    /// framed under and the id it is sealed under have to be the same one,
    /// which is why `nextId()` is taken before sealing rather than during
    /// framing.
    @Test func anIdThatDoesNotMatchTheFramesWillNotOpen() throws {
        let vector = try #require(BleVector.ble)
        let handshake = try ClearSignerHandshake(
            secret: vector.requesterSecret, nonce: vector.requesterNonce
        )
        let session = try handshake.complete(peerHello: vector.signerHello, relay: false)
        let page = try #require(vector.fromPage.first)
        #expect(throws: (any Error).self) {
            try session.open(sealed: page.sealed, msgId: page.msgId &+ 1)
        }
    }
}

// MARK: - The peripheral

@MainActor
struct ClearSignerBlePeripheralTests {

    // MARK: The GATT layout and the advert

    /// The service, its two characteristics and what goes on the air
    /// (PROTOCOL.md §1). The properties are not decoration: a `c2p` without
    /// `writeWithoutResponse` breaks the page's fast path, and an advert
    /// without the service uuid never reaches Chrome's chooser at all.
    @Test func theServiceAndTheAdvertAreTheProtocols() async throws {
        let uuids = clearSignerBleUuids()
        let radio = StubBleRadio()
        radio.automatic = false
        let conversation = try #require(make(radio: radio))
        async let pairing = conversation.advertise()
        await Task.yield()

        let service = try #require(radio.services.first)
        #expect(service.uuid == CBUUID(string: uuids[0]))
        #expect(service.isPrimary)
        #expect(radio.removeServicesCalls >= 1, "a service left over from a past session would linger")

        let characteristics = service.characteristics as? [CBMutableCharacteristic] ?? []
        #expect(characteristics.count == 2)
        let c2p = characteristics.first { $0.uuid == CBUUID(string: uuids[1]) }
        let p2c = characteristics.first { $0.uuid == CBUUID(string: uuids[2]) }
        #expect(c2p?.properties.contains(.write) == true)
        #expect(c2p?.properties.contains(.writeWithoutResponse) == true)
        #expect(c2p?.permissions.contains(.writeable) == true)
        #expect(p2c?.properties.contains(.notify) == true)
        #expect(p2c?.properties.contains(.read) == true)
        #expect(p2c?.permissions.contains(.readable) == true)

        // Nothing is on the air until the service is actually in the database:
        // a central that arrived first would find nothing to talk to.
        #expect(radio.adverts.isEmpty)
        conversation.serviceAdded(error: nil)
        let advert = try #require(radio.adverts.first)
        #expect(advert[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID] == [CBUUID(string: uuids[0])],
                "Chrome's chooser filters on this")
        let name = advert[CBAdvertisementDataLocalNameKey] as? String
        #expect(name == conversation.localName)
        #expect(name?.isEmpty == false, "a person has to recognise their own phone in the chooser")

        conversation.cancel()
        _ = await pairing
    }

    /// The chunk is sized to the link by the CORE, through the door that takes
    /// CoreBluetooth's own number: `fitToValueLen`. A frame that did not fit
    /// would be TRUNCATED by the controller rather than refused — a message
    /// that never completes and is swept ten seconds later — so nothing about
    /// this may be approximate.
    ///
    /// The table also pins the two doors to each other. `fitToMtu` wants the
    /// ATT MTU and `fitToValueLen` the notification capacity, which is three
    /// bytes smaller; feeding one the other's number costs three bytes on
    /// every frame and shows up nowhere. That is a comment in the core and an
    /// assertion here.
    @Test func theChunkIsSizedToTheLinkByTheCore() async throws {
        let vector = try #require(BleVector.ble)
        // (what CoreBluetooth reports, the ATT MTU it implies, the chunk).
        for (capacity, attMtu, expected) in [
            (509, 512, 244),   // clamped at the default
            (244, 247, 238),   // the MTU the page's 244 was written for
            (182, 185, 176),
            (64, 67, 58),
            (20, 23, 20),      // the floor
        ] {
            let radio = StubBleRadio()
            radio.maximumUpdateValueLength = capacity
            radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
            let conversation = try #require(make(radio: radio, vector: vector))
            #expect(await conversation.advertise() == vector.code)

            let framer = ClearSignerFramer()
            #expect(Int(framer.fitToValueLen(valueLen: UInt32(capacity))) == expected,
                    "the core does not size a notification of \(capacity) the way this expects")
            #expect(framer.fitToValueLen(valueLen: UInt32(capacity))
                        == ClearSignerFramer().fitToMtu(mtu: UInt32(attMtu)),
                    "the two doors disagree at \(attMtu): one of them is three bytes out")
            // What we sent, re-cut at that chunk: identical frames mean the
            // peripheral asked the core the same question we just did.
            let ours = try #require(radio.messages().first).payload
            #expect(radio.notified == framer.frames(msgId: 1, payload: ours, sealed: false),
                    "the hello was not cut to a link of \(capacity)")
            // And, above the floor, every frame actually fits the link.
            if expected > 20 {
                #expect(radio.notified.allSatisfy { $0.count <= capacity },
                        "a frame would have been truncated on a link of \(capacity)")
            }
        }
    }

    // MARK: The handshake

    /// The page connects, writes its hello in the clear, and both screens end
    /// up showing the same six digits. Ours goes back in the clear too — those
    /// two are the only messages on this channel that are not sealed.
    @Test func thePagesHelloIsAnsweredAndTheCodeIsTheVectors() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector))

        #expect(await conversation.advertise() == vector.code)

        let sent = radio.messages()
        #expect(sent.count == 1, "exactly one message answers the hello")
        let hello = try #require(sent.first)
        #expect(!hello.sealed, "the handshake travels in the clear; everything after it does not")
        let json = try CoreJSON.object(String(decoding: hello.payload, as: UTF8.self))
        #expect(json["t"] as? String == "hello")
        #expect(json["role"] as? String == "requester")
        #expect(json["app"] as? String == vector.app)
        let theirs = try CoreJSON.object(vector.requesterHello)
        #expect(json["pk"] as? String == theirs["pk"] as? String)
    }

    /// A hello cut across a dozen frames, delivered backwards and with one
    /// frame repeated, still becomes the same hello. The reassembler is the
    /// core's; what this proves is that the peripheral hands it every write
    /// and waits, rather than acting on a fragment.
    @Test func framesMayArriveOutOfOrderAndTwice() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.automatic = false
        let conversation = try #require(make(radio: radio, vector: vector))

        let framer = ClearSignerFramer()
        while framer.chunk() > 20 { _ = framer.halve() }
        var pieces = framer.frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        #expect(pieces.count > 3, "this case needs a message worth shuffling")

        async let pairing = conversation.advertise()
        await Task.yield()
        conversation.serviceAdded(error: nil)
        conversation.subscribed(maximumUpdateValueLength: 20)
        let repeated = pieces.removeFirst()
        conversation.received([ClearSignerBleWrite(value: repeated)])
        // The same frame again is not a second frame.
        conversation.received([ClearSignerBleWrite(value: repeated)])
        for frame in pieces.reversed() {
            conversation.received([ClearSignerBleWrite(value: frame)])
        }
        #expect(await pairing == vector.code)
    }

    /// A message that loses a frame is dropped rather than waited out (§2),
    /// and the missing frame, arriving late, starts a new message instead of
    /// completing one nobody is waiting for. Ten seconds is the core's window;
    /// the clock is the shell's, which is why it can be handed over.
    @Test func aDroppedFrameIsSweptAndDoesNotComeBack() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.automatic = false
        var now: UInt64 = 1_000
        let conversation = try #require(make(radio: radio, vector: vector, clock: { now }))

        let framer = ClearSignerFramer()
        while framer.chunk() > 20 { _ = framer.halve() }
        let pieces = framer.frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        #expect(pieces.count > 2)

        async let pairing = conversation.advertise()
        await Task.yield()
        conversation.serviceAdded(error: nil)
        conversation.subscribed(maximumUpdateValueLength: 20)

        // Everything but the last frame arrives…
        for frame in pieces.dropLast() {
            conversation.received([ClearSignerBleWrite(value: frame)])
        }
        #expect(radio.notified.isEmpty, "a fragment must never be answered")

        // …the window passes, and the half-arrived message goes with it…
        now += 11_000
        conversation.received([])
        // …so its last frame cannot complete it.
        conversation.received([ClearSignerBleWrite(value: pieces[pieces.count - 1])])
        #expect(radio.notified.isEmpty, "a swept message was answered anyway")

        conversation.cancel()
        _ = await pairing
    }

    // MARK: The code, and what it guards

    /// **Nothing sealed leaves this phone until the person says the two
    /// screens agree.** On BLE there is no pairing link and so no `rk`;
    /// proximity and these six digits are the entire defence against a page in
    /// the middle, and a wallet that put its intent on the air first would
    /// have none at all.
    @Test func aCodeThatWasNeverConfirmedSendsNothing() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector))
        #expect(await conversation.advertise() == vector.code)

        // The person looked at the two screens, saw different digits, and
        // never tapped "the codes match".
        let ending = await conversation.begin(anAsk())
        #expect(ending == .outcome(.refused(refusal: .declined)))
        #expect(radio.messages().allSatisfy { !$0.sealed },
                "an intent reached the air before anybody vouched for the page")
    }

    /// And a page that seals something at us before the confirmation is not
    /// even opened.
    @Test func aSealedMessageBeforeTheConfirmationIsIgnored() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector))
        #expect(await conversation.advertise() == vector.code)

        let page = try #require(vector.fromPage.first)
        for frame in frames(msgId: page.msgId, payload: page.sealed, sealed: true) {
            conversation.received([ClearSignerBleWrite(value: frame)])
        }
        #expect(radio.messages().allSatisfy { !$0.sealed })
    }

    // MARK: A session of several requests (§11)

    /// One connection, two sealed requests, and the page's `bye`. The first
    /// answer is the vectors' own `result`, which the CORE refuses because it
    /// carries no signature — and that refusal is the proof it was opened at
    /// all: a wrong key, label, direction or `msgId` would have left this end
    /// waiting instead.
    @Test func oneConnectionCarriesSeveralRequestsAndEndsWithBye() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        // The vectors' own request ids, so the vectors' own answers are the
        // ones these requests are waiting for.
        var ids = ["e6f3", "a1b2"]
        let conversation = try #require(
            make(radio: radio, vector: vector, nextId: { ids.isEmpty ? "x" : ids.removeFirst() })
        )
        #expect(await conversation.advertise() == vector.code)
        conversation.confirm()

        let page = vector.fromPage
        #expect(page.count == 2, "this case needs a result and then a bye")

        let first = await withRequest(conversation, radio: radio, answer: page[0])
        guard case .outcome(.refused(let refusal)) = first else {
            Issue.record("the page's sealed answer did not reach the core: \(first)")
            return
        }
        #expect(ClearSignerNotice(refusal) == .mismatch)

        // What the page saw of request one: sealed, in the peripheral's own
        // direction, with the method nowhere in the clear.
        let intent = try #require(radio.messages().first { $0.sealed })
        #expect(intent.payload.prefix(4) == Data("P2C.".utf8))
        #expect(!hex(intent.payload).contains(hex(Data("vela_signIn".utf8))),
                "the air must never carry the method in the clear")

        // Request two, answered by the page's `bye`: it hung up holding the
        // request, which is a page closed without signing.
        let second = await withRequest(conversation, radio: radio, answer: page[1])
        #expect(second == .outcome(.refused(refusal: .declined)))
        #expect(radio.stopAdvertisingCalls >= 1, "the bye ended the session but left the radio up")
    }

    /// An ANSWER that loses a frame ends its request rather than leaving a
    /// spinner up for ever. Nothing else on this channel has a clock: there is
    /// no socket to die and no listener to time out, so the reassembler's own
    /// ten seconds are what the person is waiting on.
    @Test func anAnswerThatLosesAFrameEndsTheRequest() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        var now: UInt64 = 1_000
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector, clock: { now }))
        #expect(await conversation.advertise() == vector.code)
        conversation.confirm()

        // The page's answer, cut small enough to take several frames — one of
        // which never arrives.
        let page = try #require(vector.fromPage.first)
        let framer = ClearSignerFramer()
        while framer.chunk() > 20 { _ = framer.halve() }
        let pieces = framer.frames(msgId: page.msgId, payload: page.sealed, sealed: true)
        #expect(pieces.count > 2, "this case needs an answer worth losing a frame of")

        let before = radio.notified.count
        async let ending = conversation.send(anAsk())
        var turns = 0
        while radio.notified.count == before, turns < 1_000 {
            await Task.yield()
            turns += 1
        }
        for frame in pieces.dropLast() {
            conversation.received([ClearSignerBleWrite(value: frame)])
        }
        now += 11_000
        conversation.received([])
        #expect(await ending == .timedOut)
    }

    /// The wallet's own `bye` when the flow finishes: sealed, in the same
    /// direction, and then the radio comes down.
    @Test func theWalletsOwnByeIsSealedAndThenTheRadioStops() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector))
        #expect(await conversation.advertise() == vector.code)
        conversation.confirm()

        conversation.end()
        let farewell = try #require(radio.messages().last)
        #expect(farewell.sealed)
        #expect(farewell.payload.prefix(4) == Data("P2C.".utf8))
        // The teardown waits a turn, so the farewell reaches the controller
        // before the service is removed from under it.
        await Task.yield()
        #expect(radio.stopAdvertisingCalls >= 1)
    }

    /// A tab that closes mid-request is a page closed without signing, not a
    /// channel that broke — and those two get different sentences on screen.
    @Test func aCentralThatUnsubscribesMidRequestIsADecline() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector))
        #expect(await conversation.advertise() == vector.code)
        conversation.confirm()

        let before = radio.notified.count
        async let ending = conversation.send(anAsk())
        var turns = 0
        while radio.notified.count == before, turns < 1_000 {
            await Task.yield()
            turns += 1
        }
        conversation.unsubscribed()
        #expect(await ending == .outcome(.refused(refusal: .declined)))
    }

    /// A controller whose queue is full does not lose frames: they wait for
    /// `peripheralManagerIsReady`. A batch intent is a dozen notifications and
    /// the queue is shallow, so this is the ordinary case, not the edge.
    @Test func framesWaitForAControllerThatIsFull() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        // At this size the hello alone is a dozen frames, and the controller
        // takes only the first.
        radio.maximumUpdateValueLength = 20
        radio.acceptsBeforeFull = 1
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector))
        #expect(await conversation.advertise() == vector.code)
        #expect(radio.notified.count == 1)
        #expect(conversation.queued > 0, "the rest of the hello was dropped rather than queued")

        radio.acceptsBeforeFull = nil
        conversation.ready()
        #expect(conversation.queued == 0)
        let sent = radio.messages()
        #expect(sent.count == 1, "the hello did not survive being paused half way")
        #expect(sent.first?.sealed == false)
    }

    // MARK: When the radio cannot carry a session

    /// A refused permission is a card that names what is missing — and WHY it
    /// is wanted — rather than a spinner, or a bare "permission needed" that
    /// leaves a person guessing what it is for.
    @Test func aRefusedPermissionNamesWhatIsMissing() async throws {
        let radio = StubBleRadio(state: .unauthorized)
        let conversation = try #require(make(radio: radio))
        #expect(await conversation.advertise() == nil)
        #expect(conversation.trouble == .notAuthorized)
        #expect(radio.adverts.isEmpty, "nothing should have gone on the air")

        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let trouble = ClearSignerBleTrouble.notAuthorized
        let body = loc.t(trouble.bodyKey)
        #expect(body != trouble.bodyKey, "the card's sentence has nothing behind it")
        #expect(body.lowercased().contains("bluetooth"))
        #expect(body.lowercased().contains("permission"))
    }

    /// Bluetooth switched off, and a device with no peripheral role at all,
    /// get their OWN sentence. Never "you refused a permission", which would
    /// send somebody to a settings screen with nothing in it to change — and
    /// never "turn Bluetooth on" for a device that has no peripheral role,
    /// which is advice that cannot work however carefully it is followed.
    @Test func theThreeWaysTheRadioCannotCarryASessionEachSayTheirOwnThing() async throws {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        for (state, expected) in [
            (CBManagerState.poweredOff, ClearSignerBleTrouble.poweredOff),
            (.unsupported, .unsupported),
        ] {
            let conversation = try #require(make(radio: StubBleRadio(state: state)))
            #expect(await conversation.advertise() == nil)
            #expect(conversation.trouble == expected)
        }

        // Three states, three sentences, no two of them the same.
        let sentences = [
            ClearSignerBleTrouble.notAuthorized,
            .poweredOff,
            .unsupported,
        ].map { loc.t($0.bodyKey) }
        #expect(Set(sentences).count == 3, "two of the three cards say the same thing")
        for (trouble, sentence) in zip(
            [ClearSignerBleTrouble.notAuthorized, .poweredOff, .unsupported], sentences
        ) {
            #expect(sentence != trouble.bodyKey, "\(trouble) has no sentence behind it")
        }
        // A device that cannot do this at all is told so, and pointed at the
        // routes that still work, rather than at the Bluetooth switch.
        #expect(!sentences[2].lowercased().contains("turn bluetooth on"))
        #expect(ClearSignerBleTrouble.unavailable.bodyKey
                    == ClearSignerBleTrouble.unsupported.bodyKey,
                "an advert that would not come up is the same dead end to the person")
    }

    /// Every sentence this route puts on screen resolves — a missing key
    /// renders as the key itself (`Loc`, FR-005), which on a sheet somebody is
    /// mid-ceremony on would be worse than a wrong word. The hint carries the
    /// foreground rule (PROTOCOL.md §1), so that is asserted rather than
    /// assumed: it is the one sentence that keeps a person from thinking the
    /// channel is broken when they switch apps.
    @Test func everySentenceOnTheNearbyRouteHasWordsBehindIt() throws {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        for key in [
            I18nKeys.ClearSigner.nearby,
            I18nKeys.ClearSigner.nearbyHint,
            I18nKeys.ClearSigner.bluetoothNeeded,
            I18nKeys.ClearSigner.bluetoothOff,
            I18nKeys.ClearSigner.bluetoothUnsupported,
        ] {
            #expect(loc.t(key) != key, "\(key) has no sentence behind it")
        }
        #expect(loc.t(I18nKeys.ClearSigner.nearbyHint).lowercased().contains("keep vela open"),
                "the hint no longer says the app must stay on screen")
        // The name is interpolated, not concatenated: a person matches what is
        // on this screen against a row in the browser's chooser.
        let named = loc.t(I18nKeys.ClearSigner.nearbyName, vars: ["name": "Vela · iPhone"])
        #expect(named.contains("Vela · iPhone"))
        #expect(!named.contains("{{"), "the name was never substituted")
    }

    /// An advertisement that will not start is reported rather than waited on.
    @Test func anAdvertThatNeverStartsIsReported() async throws {
        let radio = StubBleRadio()
        radio.automatic = false
        let conversation = try #require(make(radio: radio))
        async let pairing = conversation.advertise()
        await Task.yield()
        conversation.serviceAdded(error: nil)
        conversation.advertisingStarted(
            error: NSError(domain: CBErrorDomain, code: CBError.Code.unknown.rawValue)
        )
        #expect(await pairing == nil)
        #expect(conversation.trouble == .unavailable)
    }

    /// PROTOCOL.md §1: backgrounded, the advert loses its local name and the
    /// service uuid moves into the overflow area, where a desktop scanner
    /// effectively cannot see it. Nothing here can prevent that — so it is
    /// REPORTED, and the advert is put back on the way in, so the name returns
    /// for somebody still staring at a chooser.
    @Test func goingOffScreenIsReportedAndTheAdvertIsPutBack() async throws {
        let radio = StubBleRadio()
        radio.automatic = false
        let conversation = try #require(make(radio: radio))
        var reported: [Bool] = []
        conversation.onForegroundChanged = { reported.append($0) }

        async let pairing = conversation.advertise()
        await Task.yield()
        conversation.serviceAdded(error: nil)
        #expect(radio.adverts.count == 1)

        NotificationCenter.default.post(
            name: UIApplication.didEnterBackgroundNotification, object: nil
        )
        #expect(conversation.backgrounded)
        #expect(reported == [true])

        NotificationCenter.default.post(
            name: UIApplication.willEnterForegroundNotification, object: nil
        )
        #expect(!conversation.backgrounded)
        #expect(reported == [true, false])
        #expect(radio.adverts.count == 2, "the advert was not restarted, so the name never came back")
        #expect(radio.stopAdvertisingCalls >= 1, "two adverts at once, rather than one replacing the other")

        conversation.cancel()
        _ = await pairing
    }

    /// Once the session is over the page is told so, rather than having more
    /// of its bytes quietly swallowed.
    @Test func writesAfterTheEndAreRefusedRatherThanSwallowed() async throws {
        let vector = try #require(BleVector.ble)
        let radio = StubBleRadio()
        radio.script = frames(msgId: 1, payload: Data(vector.signerHello.utf8), sealed: false)
        let conversation = try #require(make(radio: radio, vector: vector))
        _ = await conversation.advertise()
        conversation.end()
        #expect(
            conversation.received([ClearSignerBleWrite(value: Data([0, 1, 0, 0, 0, 1]))])
                == .requestNotSupported
        )
    }
}

// MARK: - Helpers

/// A conversation over a stubbed controller. Given a vector, the handshake is
/// the vectors' own, so the session it derives is theirs too.
@MainActor
private func make(
    radio: StubBleRadio,
    vector: BleVector? = nil,
    nextId: @escaping () -> String = { "e6f3" },
    clock: @escaping () -> UInt64 = { 0 }
) -> ClearSignerBleConversation? {
    var draws = vector.map { [$0.requesterSecret, $0.requesterNonce] }
        ?? [Data(repeating: 0x22, count: 32), Data(repeating: 0xB2, count: 16)]
    let conversation = ClearSignerBleConversation(
        signerUrl: "https://sign.getvela.app/",
        app: vector?.app ?? "vela-test/1",
        localName: "Vela · iPhone",
        random: { _ in draws.removeFirst() },
        makeRadio: { delegate in
            radio.conversation = delegate as? ClearSignerBleConversation
            return radio
        },
        nextId: nextId,
        clock: clock
    )
    radio.conversation = conversation
    return conversation
}

/// Frames, cut by the core at the default chunk.
@MainActor
private func frames(msgId: UInt8, payload: Data, sealed: Bool) -> [Data] {
    ClearSignerFramer().frames(msgId: msgId, payload: payload, sealed: sealed)
}

/// A signing request with nothing surprising in it. What it asks for does not
/// matter to this file — every verdict on it is the core's.
private func anAsk() -> ClearSignerAsk {
    .signature(
        request: #"{"id":"e6f3","intent":{"method":"vela_signIn","params":[{}],"origin":""},"context":{}}"#,
        digest: Data(repeating: 9, count: 32),
        keys: [WalletKeyRecord(credentialId: "aa", publicKeyHex: "04" + String(repeating: "11", count: 64))]
    )
}

/// Put a request, then write the page's sealed answer back while it is in
/// flight — which is when a real central would answer it.
@MainActor
private func withRequest(
    _ conversation: ClearSignerBleConversation,
    radio: StubBleRadio,
    answer: (msgId: UInt8, sealed: Data)
) async -> ClearSignerChannel.Ending {
    let before = radio.notified.count
    async let ending = conversation.send(anAsk())
    // Wait for the request to be ON THE AIR rather than for a fixed number of
    // turns: a page answers what it has received, and a test that answered
    // before the last frame left would be testing a race, not the channel.
    var turns = 0
    while radio.notified.count == before, turns < 1_000 {
        await Task.yield()
        turns += 1
    }
    for frame in frames(msgId: answer.msgId, payload: answer.sealed, sealed: true) {
        conversation.received([ClearSignerBleWrite(value: frame)])
    }
    return await ending
}

private func hex(_ data: Data) -> String {
    data.map { String(format: "%02x", $0) }.joined()
}
