//
//  DisplayCurrencyTests.swift
//  VelaWalletTests
//
//  The third machine, and the one rule it exists to protect.
//
//  `rate: null` is not `1`. Every other test in this file is ordinary; that one
//  is the reason the machine has a `rate: Option<f64>` at all, and the reason
//  the core's own doc calls a defaulted 1 "a real 7x mispayment".
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct DisplayCurrencyTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// No rate sources, so nothing reaches the network: these tests are about
    /// the machine's answers, and a suite that priced CNY by asking Ethereum
    /// would be measuring somebody's wifi.
    private func fresh(sources: [any FiatRateSource] = []) -> (VelaStore, UserDefaults, DisplayCurrencyExecutor) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        return (store, defaults, DisplayCurrencyExecutor(store: store, sources: sources))
    }

    /// A rung that answers a fixed map — the waterfall's shape without its
    /// network.
    private struct StubSource: FiatRateSource {
        let rates: [String: Double]
        func rate(_ code: String) async -> Double? { rates[code] }
    }

    private func answer(_ executor: DisplayCurrencyExecutor, _ op: [String: Any]) async -> [String: Any] {
        (try? CoreJSON.object(await executor.perform(op))) ?? [:]
    }

    // MARK: - The executor

    @Test func everyOperationIsAnswered() async {
        let (_, _, executor) = fresh()
        #expect(DisplayCurrencyExecutor.operations.count == 4)
        for name in DisplayCurrencyExecutor.operations {
            let reply = await answer(executor, ["type": name, "code": "CNY"])
            #expect(!(reply["type"] as? String ?? "").isEmpty, "no answer for `\(name)`")
        }
    }

    /// Absent means "never chose", which the core reads differently from
    /// "chose USD" — the device-region seed depends on the difference.
    @Test func anUnwrittenCurrencyReadsAsNeverChosenRatherThanUsd() async {
        let (_, _, executor) = fresh()
        let reply = await answer(executor, ["type": "read_stored_code"])
        #expect(reply["code"] is NSNull)
    }

    @Test func aChosenCurrencyRoundTrips() async {
        let (_, defaults, executor) = fresh()
        _ = await executor.perform(["type": "write_stored_code", "code": "CNY"])
        #expect(defaults.string(forKey: VelaStore.Key.displayCurrency) == "CNY")
        let reply = await answer(executor, ["type": "read_stored_code"])
        #expect(reply["code"] as? String == "CNY")
    }

    /// A carried debt on the desktop, a platform call here.
    @Test func theDeviceCurrencyComesFromTheSystem() async {
        let (_, _, executor) = fresh()
        let reply = await answer(executor, ["type": "read_device_currency"])
        #expect(reply["type"] as? String == "device_currency")
        // `null` is a real answer for a regionless locale, so only the shape is
        // asserted — a specific code would be a fact about this machine.
        #expect(reply["code"] is String || reply["code"] is NSNull)
    }

    /// A code no source can price is `null`, never 1.
    @Test func anUnresolvableRateIsUnknownRatherThanOne() async {
        let (_, _, executor) = fresh()
        let reply = await answer(executor, ["type": "resolve_rate", "code": "CNY"])
        #expect(reply["type"] as? String == "rate_resolved")
        #expect(reply["rate"] is NSNull, "a fabricated rate reached the core")
    }

    /// USD is the identity, and it is answered without asking anybody — a
    /// wallet that cannot reach the network still knows one dollar is one
    /// dollar.
    @Test func usdResolvesToOneWithNoSourceAtAll() async {
        let (_, _, executor) = fresh()
        let reply = await answer(executor, ["type": "resolve_rate", "code": "USD"])
        #expect((reply["rate"] as? NSNumber)?.doubleValue == 1)
    }

    /// The waterfall takes the FIRST rung that can price the code. Chainlink's
    /// feeds are asked before the configurable endpoint, and a source that
    /// answers `nil` hands on rather than ending the search.
    @Test func theFirstSourceThatCanPriceItWins() async {
        let (_, _, executor) = fresh(sources: [
            StubSource(rates: ["JPY": 150]),
            StubSource(rates: ["JPY": 999, "VND": 25_000]),
        ])
        let first = await answer(executor, ["type": "resolve_rate", "code": "JPY"])
        #expect((first["rate"] as? NSNumber)?.doubleValue == 150)

        let second = await answer(executor, ["type": "resolve_rate", "code": "VND"])
        #expect((second["rate"] as? NSNumber)?.doubleValue == 25_000)
    }

    /// A source answering zero, a negative or an infinity has not priced
    /// anything. Passing one through would put a `0` where the core expects
    /// "no rate" — and `0` multiplies every figure on the screen to nothing.
    @Test func aNonsensicalRateIsRefusedRatherThanForwarded() async {
        let (_, _, executor) = fresh(sources: [
            StubSource(rates: ["EUR": 0, "GBP": -1, "CHF": .infinity]),
        ])
        for code in ["EUR", "GBP", "CHF"] {
            let reply = await answer(executor, ["type": "resolve_rate", "code": code])
            #expect(reply["rate"] is NSNull, "\(code) forwarded a nonsense rate")
        }
    }

    // MARK: - The rule

    /// **The defect this machine exists to prevent.**
    ///
    /// With no rate, the row must never show the same digits relabelled with a
    /// ¥, which would tell somebody 1,234.56 dollars is 1,234.56 yuan. Since
    /// 072 it names the currency CHOSEN, with no figure — "USD" there told
    /// somebody who had picked CNY that their choice had not taken.
    @Test func aMissingRateDegradesRatherThanRelabelling() {
        let unpriced = CurrencyViewWire(code: "CNY", rate: nil, committed: true)
        let value = SettingsLive.currencyRowValue(unpriced)
        #expect(value == "CNY", "showed \(value) for a chosen, unpriceable currency")
        #expect(!value.contains("¥"))
        #expect(!value.contains("USD"))
    }

    @Test func aPricedCurrencyConvertsAndWearsItsOwnSymbol() {
        let priced = CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)
        let value = SettingsLive.currencyRowValue(priced)
        #expect(value.hasPrefix("CNY · ¥"))
        // 1,234.56 × 7.1 — converted, not relabelled.
        #expect(value.contains("8,765"))
    }

    /// An uncommitted choice is the USD placeholder, whatever code the model
    /// happens to be carrying.
    @Test func anUncommittedChoiceShowsThePlaceholder() {
        let seeded = CurrencyViewWire(code: "CNY", rate: 7.1, committed: false)
        #expect(SettingsLive.currencyRowValue(seeded).hasPrefix("USD"))
    }

    /// The picker marks the core's code, and offers the catalog the fixture
    /// offers — one list, two builders.
    @Test func thePickerSelectsTheCoresCode() {
        let base = SettingsFixtures.build(.st5, loc: loc)
        let model = SettingsLive.withCurrency(
            CurrencyViewWire(code: "JPY", rate: nil, committed: true), on: base, loc: loc
        )
        let selected = model.currencySheet.rows.filter(\.selected)
        #expect(selected.count == 1)
        #expect(selected.first?.id == "JPY")
        #expect(model.currencySheet.rows.count == CurrencyCatalog.entries.count)
    }

    /// A code the catalog has never heard of — a device region can produce one —
    /// must not crash or invent a symbol. It is named, as chosen.
    @Test func anUnknownCodeDegradesInsteadOfInventingASymbol() {
        let exotic = CurrencyViewWire(code: "XPF", rate: 110, committed: true)
        let value = SettingsLive.currencyRowValue(exotic)
        #expect(value == "XPF")
    }
}
