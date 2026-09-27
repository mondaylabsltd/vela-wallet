//
//  ShareCardRenderTests.swift
//  VelaWalletTests
//
//  The saved share card, rendered the way 保存图片 renders it and read back.
//
//  The card is a picture that leaves the app, so the tests look at the
//  PICTURE: its code must decode to the address printed beside it, it must be
//  the web's 960 pixels wide on any phone, and its layout must be the web's
//  (`share-image.ts`) — the numbers below are the web's own.
//
//  With `VELA_SHARE_CARD_REVIEW` set to a directory (pass it to xcodebuild as
//  `TEST_RUNNER_VELA_SHARE_CARD_REVIEW=<dir>`; a relative one is inside the
//  app's tmp), the review set — the web's own five cases, real logos fetched —
//  is written there as PNGs to be LOOKED at beside
//  `app-web/vela-wallet/.share-card-review/`.
//

import Foundation
import SwiftUI
import Testing
@testable import VelaWallet

@MainActor
struct ShareCardRenderTests {
    private let address = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
    private let logos = "https://ethereum-data.getvela.app/chainlogos"

    private func card(
        headline: String = "扫码向我转账",
        note: String = "仅支持 Ethereum 网络付款",
        name: String = "大表哥",
        mark: TokenMarkModel = TokenMarkModel(ticker: "ETH", badgeColor: ChainPalette.ethereum)
    ) -> ShareCardModel {
        ShareCardModel(
            headline: headline,
            name: name,
            lines: AddressText.lines(address),
            networkNote: note,
            networkMark: mark,
            identiconSeed: address,
            wordmark: "Vela Wallet",
            modules: QrCode.shareModules(address)
        )
    }

    // MARK: - The picture

    /// The picture IS the address: its code decodes to it, with the lettered
    /// disc on the plate in the code's centre, and it is 960 pixels wide
    /// whatever this device's scale.
    @Test func thePictureDecodesToTheAddress() async throws {
        let model = card()
        let image = try #require(await ShareCardExport.render(model), "the card did not render")
        let pixels = try #require(image.cgImage)
        #expect(pixels.width == 960)
        #expect(pixels.height == Int((ShareCardLayout(model).height * 2).rounded()))
        #expect(QrDecoder.decode(image: image) == address,
                "the picture must BE the address, not resemble one")
    }

    /// A logo that cannot be fetched leaves the lettered disc, and the save
    /// still happens — no logo is not a failed save.
    @Test func anUnreachableLogoFallsBackToTheDisc() async throws {
        let unreachable = TokenMarkModel(ticker: "ETH", badgeColor: ChainPalette.ethereum,
                                         logoURLs: ["not a url", "\(logos)/eip155-0.png"])
        #expect(await ShareCardExport.logo(unreachable.logoURLs) == nil)
        let image = try #require(await ShareCardExport.render(card(mark: unreachable)))
        #expect(QrDecoder.decode(image: image) == address)
    }

    // MARK: - The layout, in the web's numbers

    /// A one-line headline puts every block where the web does: the sheet at
    /// 150, the code at 198, the curve's edge at 599, the card 743 tall.
    @Test func aOneLineCardIsTheWebsCard() {
        let layout = ShareCardLayout(card())
        #expect(layout.headlineSize == 32)
        #expect(layout.headline.count == 1)
        #expect(layout.sheet == CGRect(x: 80, y: 150, width: 320, height: 397))
        #expect(layout.qr == CGRect(x: 128, y: 198, width: 224, height: 224))
        #expect(layout.centre == CGPoint(x: 240, y: 310))
        #expect(layout.edge == 599)
        #expect(layout.height == 743)
        // The brand line is centred in the 112 below the curve's lowest point.
        #expect(layout.icon.midY == CGFloat(599 + 32 + 56))
    }

    /// A headline too long for one line at 26 goes onto two, balanced, each
    /// inside the 400 the orange allows — and the card grows to hold it.
    @Test func aLongHeadlineTakesTwoLinesAndTheCardGrows() {
        let long = ShareCardLayout(card(headline: "Отсканируйте, чтобы отправить мне крипто"))
        #expect(long.headline.count == 2)
        #expect(long.headlineSize <= ShareCardGeometry.headlineSize)
        for line in long.headline {
            #expect(ShareCardType.width(line.text, ShareCardType.sans(long.headlineSize, .bold))
                    <= ShareCardGeometry.textWidth)
        }
        let short = ShareCardLayout(card())
        #expect(long.height - short.height == long.headlineSize * 1.25 * 2 - 40)
    }

    /// The name is cut to its room; the address never is.
    @Test func aLongNameIsCutAndTheAddressIsNot() {
        let layout = ShareCardLayout(card(name: "Gemeinsames Haushaltskonto der Familie"))
        #expect(layout.name.text.hasSuffix("\u{2026}"))
        let room = ShareCardGeometry.qr - ShareCardGeometry.identicon - ShareCardGeometry.identityTextGap
        #expect(ShareCardType.width(layout.name.text,
                                    ShareCardType.sans(ShareCardGeometry.nameSize, .bold)) <= room)
        #expect(layout.address.map(\.text) == AddressText.lines(address))
        #expect(layout.address.map(\.text).joined() == address)
    }

    /// The identicon and the text beside it are centred on the card as a pair.
    @Test func theAccountRowIsCentred() {
        let layout = ShareCardLayout(card())
        let mono = ShareCardType.mono(ShareCardGeometry.addressSize)
        let right = layout.textX + AddressText.lines(address)
            .map { ShareCardType.width($0, mono) }.max()!
        #expect(abs((layout.identicon.minX + right) / 2 - 240) < 0.01)
    }

    // MARK: - The review set

    /// The web's five review cases, real logos fetched — two of them WebP
    /// behind a `.png` name — each decoded and written out to be looked at.
    @Test(.enabled(if: ProcessInfo.processInfo.environment["VELA_SHARE_CARD_REVIEW"] != nil))
    func theReviewSetIsWrittenOut() async throws {
        let path = try #require(ProcessInfo.processInfo.environment["VELA_SHARE_CARD_REVIEW"])
        // A relative path lands in the app's own tmp — how a PHONE's set gets
        // out (`devicectl device copy from --domain-type appDataContainer`).
        let dir = path.hasPrefix("/")
            ? URL(fileURLWithPath: path)
            : FileManager.default.temporaryDirectory.appendingPathComponent(path)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let cases: [(file: String, card: ShareCardModel, logo: Bool)] = [
            ("zh-ethereum", card(mark: TokenMarkModel(
                ticker: "ETH", badgeColor: ChainPalette.ethereum,
                logoURLs: ["\(logos)/eip155-1.png"])), true),
            ("en-gnosis-webp", card(
                headline: "Scan to Send Me Crypto", note: "Gnosis payments only", name: "MultiTest",
                mark: TokenMarkModel(ticker: "XDAI", badgeColor: ChainPalette.gnosis,
                                     logoURLs: ["\(logos)/eip155-100.png"])), true),
            ("ru-long-headline-bnb", card(
                headline: "Отсканируйте, чтобы отправить мне крипто",
                note: "Только платежи в сети BNB Smart Chain", name: "Основной кошелёк",
                mark: TokenMarkModel(ticker: "BNB", badgeColor: ChainPalette.bnb,
                                     logoURLs: ["\(logos)/eip155-56.png"])), true),
            ("de-long-name-tempo", card(
                headline: "Scannen, um mir Krypto zu senden", note: "Nur Zahlungen über Tempo",
                name: "Gemeinsames Haushaltskonto der Familie",
                mark: TokenMarkModel(ticker: "USD", badgeColor: ChainPalette.ethereum,
                                     logoURLs: ["\(logos)/eip155-4217.png"])), true),
            ("zh-no-logo", card(), false),
        ]
        for entry in cases {
            if entry.logo {
                #expect(await ShareCardExport.logo(entry.card.networkMark.logoURLs) != nil,
                        "\(entry.file): the logo did not load (WebP?)")
            }
            let image = try #require(await ShareCardExport.render(entry.card), "\(entry.file)")
            #expect(image.cgImage?.width == 960, "\(entry.file)")
            #expect(QrDecoder.decode(image: image) == address, "\(entry.file) did not decode")
            let png = try #require(image.pngData())
            try png.write(to: dir.appendingPathComponent("\(entry.file).png"))
        }
    }
}
