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

    private func fresh() -> (VelaStore, UserDefaults, DisplayCurrencyExecutor) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        return (store, defaults, DisplayCurrencyExecutor(store: store))
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

    /// Fail-closed until 051 owns the price path — and `null`, never 1.
    @Test func anUnresolvableRateIsUnknownRatherThanOne() async {
        let (_, _, executor) = fresh()
        let reply = await answer(executor, ["type": "resolve_rate", "code": "CNY"])
        #expect(reply["type"] as? String == "rate_resolved")
        #expect(reply["rate"] is NSNull, "a fabricated rate reached the core")
    }

    // MARK: - The rule

    /// **The defect this machine exists to prevent.**
    ///
    /// With no rate, the row must degrade to the USD figure under a USD symbol
    /// — not the same digits relabelled with a ¥, which would tell somebody
    /// 1,234.56 dollars is 1,234.56 yuan.
    @Test func aMissingRateDegradesRatherThanRelabelling() {
        let unpriced = CurrencyViewWire(code: "CNY", rate: nil, committed: true)
        let value = SettingsLive.currencyRowValue(unpriced)
        #expect(value.hasPrefix("USD"), "showed \(value) for an unpriceable currency")
        #expect(!value.contains("CNY"))
        #expect(!value.contains("¥"))
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
    /// must not crash or invent a symbol.
    @Test func anUnknownCodeDegradesInsteadOfInventingASymbol() {
        let exotic = CurrencyViewWire(code: "XPF", rate: 110, committed: true)
        let value = SettingsLive.currencyRowValue(exotic)
        #expect(value.hasPrefix("USD"))
    }
}
