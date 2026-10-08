//
//  WalletLive.swift
//  VelaWallet
//
//  The person's own money, in the display models the drawn home screen already
//  consumes.
//
//  A **sibling** of `WalletFixtures`, and — like `SettingsLive` — a partial
//  one: `WalletHomeModel` carries the whole screen, and the balance hero, the
//  asset rows and (since phase 3) the activity section have machines behind
//  them. The pill and the sheet wait on their own. A field this file does not
//  touch is visibly still a fixture.
//
//  ## `rate: null` is not `1`, again, and now on the figure that matters
//
//  `display_total_usd == nil` means **no total can be stated**. It is not zero.
//  Rendering it as `$0.00` would tell somebody their wallet is empty when the
//  truth is that nobody could price it.
//
//  ## The figure is converted, or it is labelled USD — never relabelled
//
//  `display_total_usd` is USD by construction; `display_currency` owns the
//  rate. When it has one the hero converts and wears the person's code; when it
//  does not, the hero shows the **USD figure under USD**. Relabelling the same
//  digits with a ¥ is the lie the whole `rate: null` rule exists to prevent —
//  it tells somebody 1,234.56 dollars is 1,234.56 yuan.
//
//  ## A partial total is a floor, and has to say so
//
//  When some chains answered and some did not, the core sets `balance_partial`.
//  The figure is then "at least this much", and the drawn warning status is what
//  says so. Silently showing the sum as if it were complete is the same defect
//  as showing zero, one step subtler.
//

import SwiftUI

enum WalletLive {

    /// Which currency the money on this screen is stated in, and what to
    /// multiply a USD figure by to get there.
    ///
    /// Built from the `display_currency` view, and it applies the same rule
    /// `SettingsLive.currencyRowValue` does — one rule, two surfaces.
    struct Display {
        let code: String
        let rate: Double

        static let usd = Display(code: "USD", rate: 1)

        /// The badge the hero's figure wears. Decoration, not identity — the
        /// code itself is already stated in the line above the figure, so a
        /// currency the catalog has never heard of shows the number bare rather
        /// than borrowing somebody else's `$`.
        var glyph: String { CurrencyCatalog.entry(code)?.glyph ?? "" }

        /// A currency without a rate, or one the person has not committed to,
        /// degrades to USD. `rate: nil` is **not** 1: the difference is
        /// whether the digits get relabelled.
        static func from(_ view: CurrencyViewWire?) -> Display {
            guard let view, view.committed, let rate = view.rate, rate > 0, rate.isFinite
            else { return .usd }
            return Display(code: view.code, rate: rate)
        }
    }

    /// Swap the balance hero and the asset rows onto the drawn home.
    ///
    /// `now` ages the refresh control's "Updated 2m"; `spinning` is the
    /// store's held spin (`WalletStore.spin`), the core's `refreshing` when
    /// absent.
    static func apply(
        _ view: BalanceViewWire,
        currency: CurrencyViewWire? = nil,
        feed: FeedViewWire? = nil,
        feedRead: Bool = false,
        on model: WalletHomeModel,
        loc: Loc,
        now: Date = Date(),
        spinning: Bool? = nil,
        networks: WalletNetworks = .builtin
    ) -> WalletHomeModel {
        var copy = model
        let display = Display.from(currency)
        copy.balance = balance(view, display: display, fallback: model.balance, loc: loc,
                               networks: networks)
        copy.balance.refresh = refresh(view, loc: loc, now: now, spinning: spinning)
        copy.assetRows = assetRows(view, display: display, networks: networks)
        copy.assetsSection = assetsSection(view, rows: copy.assetRows, fallback: model.assetsSection)
        if let feed {
            copy.activityGroups = activityGroups(feed, loc: loc, hidden: view.hidden, networks: networks)
            copy.activitySection = section(copy.activityGroups, read: feedRead,
                                           fallback: model.activitySection,
                                           empty: homeEmpty(feed, fallback: model.activitySection, loc: loc))
        }
        return copy
    }

    /// The home Activity's empty line, as the core chose it (spec 082 RG5):
    /// "nothing yet" with its caption, or — the feed narrowed to one chain —
    /// "nothing on this network" alone.
    static func homeEmpty(_ feed: FeedViewWire, fallback: SectionModel, loc: Loc) -> SectionEmptyModel {
        let key = feed.homeEmptyKey
        return SectionEmptyModel(
            title: loc.t(key),
            caption: key == "home.emptyNoActivity" ? (fallback.empty?.caption ?? loc.t("home.emptySubtitle")) : ""
        )
    }

    /// The section header, kept as drawn except for its mode.
    ///
    /// Three states, and the difference between the last two is the point: rows
    /// when there are rows, the drawn **empty** treatment once the store has
    /// been read and held nothing, and the **skeleton** until then. "Nothing has
    /// happened here" is a claim, and it must not be made before anybody looked.
    private static func section(
        _ groups: [ActivityGroupModel], read: Bool, fallback: SectionModel,
        empty: SectionEmptyModel? = nil
    ) -> SectionModel {
        SectionModel(
            title: fallback.title,
            action: fallback.action,
            mode: !groups.isEmpty ? .rows : (read ? .empty : .loading),
            empty: empty ?? fallback.empty
        )
    }

    /// The assets section, kept as drawn except for its mode (087 F03).
    ///
    /// The drawn home's mode stayed `.rows` whatever the core said, so an
    /// account holding nothing showed 资产 over a blank area. The same three
    /// states as the web's `assetsMode` and the desktop's `assets_strip_empty`:
    /// rows when there are rows, the skeleton while the first read is out or
    /// the balance is unknown — "nothing here" is a claim, never made before
    /// anybody looked — and the drawn empty state (存入您的第一笔资产) once the
    /// core has looked and found nothing.
    static func assetsSection(_ view: BalanceViewWire, rows: [AssetRowModel], fallback: SectionModel) -> SectionModel {
        SectionModel(
            title: fallback.title,
            action: fallback.action,
            mode: !rows.isEmpty ? .rows : (view.holdingsLoading || view.balanceUnknown ? .loading : .empty),
            empty: fallback.empty
        )
    }

    // MARK: - The hero

    static func balance(
        _ view: BalanceViewWire,
        display: Display = .usd,
        fallback: BalanceModel,
        loc: Loc? = nil,
        networks: WalletNetworks = .builtin
    ) -> BalanceModel {
        var model = BalanceModel(
            label: fallback.label,
            currency: display.code,
            state: state(view),
            integer: nil,
            decimals: nil,
            liveText: nil,
            status: status(view, fallback: fallback, loc: loc, networks: networks),
            a11yHide: fallback.a11yHide,
            a11yShow: fallback.a11yShow
        )

        // Hidden and loading both draw without a figure, so there is nothing to
        // split — and building one anyway is how a concealed balance leaks into
        // an accessibility label.
        guard model.state == .normal || model.state == .zeroLive,
              let total = view.displayTotalUsd ?? view.cachedTotalUsd
        else { return model }

        let (integer, decimals) = split(total * display.rate)
        model.integer = display.glyph + integer
        model.decimals = decimals
        return model
    }

    /// `nil` total is **not** zero — it is "no total can be stated", and the
    /// drawn loading treatment is what says that. `zeroLive` is for a wallet
    /// that genuinely holds nothing, which the core distinguishes.
    private static func state(_ view: BalanceViewWire) -> BalanceStateKind {
        if view.hidden { return .hidden }
        if view.balanceUnknown { return .loading }
        guard let total = view.displayTotalUsd ?? view.cachedTotalUsd else { return .loading }
        return total == 0 ? .zeroLive : .normal
    }

    /// The line under the figure. A partial total says so; a figure the core
    /// is still reading over says that instead — in `home.balanceStale`, as
    /// the other three clients say it. The drawn fixture's own status was nil on the
    /// live home, which left a ⚠ › line with no words (082 X-DEADPROXY).
    private static func status(
        _ view: BalanceViewWire,
        fallback: BalanceModel,
        loc: Loc?,
        networks: WalletNetworks
    ) -> BalanceStatusModel? {
        // Spec 092: networks the wallet cannot reach come first — every one,
        // held or not, said without "RPC"; the line opens their list.
        if let loc, let line = unreachableLine(view, loc: loc, networks: networks) {
            return BalanceStatusModel(kind: .warning, text: line)
        }
        let text = loc?.t("home.balanceStale") ?? fallback.status?.text ?? ""
        if view.balancePartial || !view.failedChainIds.isEmpty {
            return BalanceStatusModel(kind: .warning, text: text)
        }
        // A refresh the person asked for (`view.refreshing`) is NOT a reason
        // for this line (issue 462): the refresh control under it turns and
        // says "Updating…" instead. When it was one, every pull inserted
        // "Some balances are still updating." above the control and pushed it
        // out from under the finger.
        if view.notice == .stillUpdating {
            return BalanceStatusModel(kind: .refreshing, text: text)
        }
        return nil
    }

    /// The hero's refresh control (issue 462): when the figure was last read —
    /// "Updated 2m", the core's `last_refreshed_at_ms` in the core's relative
    /// words — and whether a refresh the person asked for is out. The home
    /// re-reads the clock at least every 30 s, so the label ages on screen.
    static func refresh(
        _ view: BalanceViewWire, loc: Loc, now: Date = Date(), spinning: Bool? = nil
    ) -> BalanceRefreshModel {
        BalanceRefreshModel(
            updated: view.lastRefreshedAtMs.map { at in
                loc.t("home.lastUpdated", vars: ["ago": loc.relativeTime(atMs: at, now: now)])
            },
            updating: loc.t("home.updating"),
            refreshing: spinning ?? view.refreshing
        )
    }

    /// The line over the networks the wallet cannot reach (spec 092) — the
    /// hero's status line and the title of the list it opens. The core
    /// chooses the sentence (`unreachableKey`: one network named, several
    /// counted); this only fills it. `nil` when every network answered. A
    /// network is named from the wallet's list, the person's own included.
    static func unreachableLine(
        _ view: BalanceViewWire, loc: Loc, networks: WalletNetworks = .builtin
    ) -> String? {
        let k = I18nKeys.SettingsUi.self
        guard let first = view.unreachableNetworks.first else { return nil }
        switch view.unreachableKey {
        case k.unreachableOne?:
            let name = networks.meta(first.chainId)?.displayName
                ?? loc.t(I18nKeys.SettingsUi.chainId, vars: ["chainId": String(first.chainId)])
            return loc.t(k.unreachableOne, vars: ["name": name])
        case k.unreachableMany?:
            return loc.t(k.unreachableMany, vars: ["n": String(view.unreachableNetworks.count)])
        default:
            return nil
        }
    }

    /// `1234.56` → `("1,234", "56")`. The drawing splits the figure so the
    /// decimals can be subordinated, which is the design language's rule about
    /// money: the magnitude reads first.
    static func split(_ total: Double) -> (String, String) {
        // The PERSON's preset, not the device's idea of a locale (spec 056).
        // The decimal mark is whatever they chose, so the split looks for that
        // and not for a dot.
        let text = Formats.number(
            total, minimumFractionDigits: 2, maximumFractionDigits: 2
        )
        let mark = Formats.separators(Formats.current.number).decimal
        guard let separator = text.range(of: mark, options: .backwards) else {
            return (text, "00")
        }
        return (String(text[..<separator.lowerBound]), String(text[separator.upperBound...]))
    }

    // MARK: - The assets

    /// One row per holding, priced or honestly unpriced.
    ///
    /// **`tokens` only.** `unpricedTokens` is not the complement of `tokens` —
    /// the core's own doc calls it "the detail sheet's 'couldn't be priced'
    /// list", a **subset** built for a different surface. Concatenating the two
    /// renders every unpriceable holding twice, which is what the first live
    /// run of this screen showed: one address, one chain, two identical xDAI
    /// rows.
    ///
    /// Invisible until the balance was real — with a fixture there was nothing
    /// to duplicate.
    ///
    /// The chain is named from the wallet's networks, the person's own
    /// included — a token on a network they added had a blank chain line.
    static func assetRows(
        _ view: BalanceViewWire,
        display: Display = .usd,
        networks: WalletNetworks = .builtin
    ) -> [AssetRowModel] {
        view.tokens.map { token in
            AssetRowModel(
                ticker: token.symbol,
                chain: networks.meta(token.chainId)?.displayName ?? "",
                badgeColor: chainColor(token.chainId),
                // The one token-amount rule every shell prints (spec 078,
                // `tokenAmountText`): the core's ladder, so this row, the Send
                // picker, the token card and the figure Max writes agree digit
                // for digit. The core's full precision ("0.00067035411363817")
                // pushed the ticker and chain out of the row on a phone.
                balance: tokenAmountText(token.balance),
                fiat: fiat(token, hidden: view.hidden, display: display),
                masked: view.hidden,
                // The real logo, with the lettermark behind it (058). The
                // badge disappears where it would repeat the coin — XDAI on
                // Gnosis is one picture, not the same picture twice.
                mark: TokenMarkModel.of(
                    chainId: token.chainId,
                    symbol: token.symbol,
                    tokenAddress: token.tokenAddress,
                    color: chainColor(token.chainId)
                )
            )
        }
    }

    /// Which way `tokenAmountText` leaves the last place.
    enum TokenRounding {
        /// Every figure that is READ: a balance, an amount, a receipt.
        case halfUp
        /// A CEILING the person may type back ("you can send up to"): rounded
        /// up, it would be a figure the balance cannot cover.
        case down
    }

    /// A token amount as every money surface reads it — the asset list's rows,
    /// the Send picker and token card, the confirm and the receipt — ONE call,
    /// so the balance on the home row and the balance beside the token on Send
    /// are the same digits, and the figure `Max` writes (the core's
    /// `send::max_figure`) is the figure the confirm repeats.
    ///
    /// The core's ladder, digit for digit (`l10n::number::format_token_amount`,
    /// the desktop's rows; the web's `tokenAmountText`): 6 places under 1, 4
    /// under 1000, 2 from 1000, rounded HALF UP, trailing zeros dropped; an
    /// amount too small to survive six places keeps two significant digits
    /// (cut) instead of reading `0`. String arithmetic only — a uint256 must
    /// never pass through a `Double` — and the decimal mark is the person's
    /// preset. Ungrouped, like the field `Max` fills. Anything that is not a
    /// plain decimal passes through as written rather than gaining digits.
    ///
    /// It replaces `trimBalance`'s six places, TRUNCATED, at every magnitude:
    /// `1.22456789` read "1.224567" on the row while Max wrote "1.2246".
    static func tokenAmountText(_ amount: String, rounding: TokenRounding = .halfUp) -> String {
        let exact = amount.trimmingCharacters(in: .whitespaces)
        let parts = exact.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
        let intRaw = parts.first.map(String.init) ?? ""
        let fracRaw = parts.count > 1 ? String(parts[1]) : ""
        let isDigits = { (text: String) in text.unicodeScalars.allSatisfy { $0.value >= 48 && $0.value <= 57 } }
        guard !(intRaw.isEmpty && fracRaw.isEmpty), isDigits(intRaw), isDigits(fracRaw) else {
            return exact
        }
        let intDigits = String(intRaw.drop { $0 == "0" })
        let places = intDigits.isEmpty ? 6 : (intDigits.count <= 3 ? 4 : 2)
        let fracDigits = Array(fracRaw.utf8).map { Int($0) - 48 }
        var digits = Array(intDigits.utf8).map { Int($0) - 48 }
            + (0..<places).map { $0 < fracDigits.count ? fracDigits[$0] : 0 }
        if rounding == .halfUp, places < fracDigits.count, fracDigits[places] >= 5 {
            var index = digits.count
            while true {
                if index == 0 {
                    digits.insert(1, at: 0)
                    break
                }
                index -= 1
                if digits[index] == 9 {
                    digits[index] = 0
                } else {
                    digits[index] += 1
                    break
                }
            }
        }
        let decimal = Formats.separators(Formats.current.number).decimal
        let split = digits.count - places
        let intPart = digits[..<split].map(String.init).joined()
        let whole = intPart.isEmpty ? "0" : intPart
        var fraction = digits[split...].map(String.init).joined()
        while fraction.hasSuffix("0") { fraction.removeLast() }
        if whole == "0" && fraction.isEmpty {
            // Below the ladder's last place: two significant digits, cut.
            let lead = fracRaw.prefix { $0 == "0" }.count
            if lead == fracRaw.count { return "0" }
            var kept = String(fracRaw.prefix(min(lead + 2, fracRaw.count)))
            while kept.hasSuffix("0") { kept.removeLast() }
            return "0" + decimal + kept
        }
        return fraction.isEmpty ? whole : whole + decimal + fraction
    }

    /// A balance to glance at (the web's `trimBalance`): at most `maxDecimals`
    /// places, cut not rounded, trailing zeros dropped, the person's decimal
    /// mark, no grouping.
    static func trimBalance(_ balance: String, maxDecimals: Int = 6) -> String {
        guard let dot = balance.firstIndex(of: ".") else { return balance }
        let whole = String(balance[..<dot])
        var fraction = String(balance[balance.index(after: dot)...].prefix(maxDecimals))
        while fraction.hasSuffix("0") { fraction.removeLast() }
        if fraction.isEmpty { return whole }
        return whole + Formats.separators(Formats.current.number).decimal + fraction
    }

    private static func fiat(
        _ token: BalanceTokenWire,
        hidden: Bool,
        display: Display
    ) -> AssetFiatModel {
        if hidden { return .masked }
        guard let price = token.priceUsd, let amount = Double(token.balance) else {
            // Held, unpriceable. The drawn `noPrice` treatment says exactly
            // that rather than showing a `$0.00` nobody should read as a value.
            return .noPrice("")
        }
        // The person's own preset, with the currency's glyph in front of it —
        // a system currency formatter would put both the marks and the symbol
        // wherever the DEVICE's locale says, which is not what they chose.
        let value = amount * price * display.rate
        return .value(display.glyph + Formats.number(
            value, minimumFractionDigits: 2, maximumFractionDigits: 2
        ))
    }

    /// What the account holds on each network, in the display currency, from
    /// the home screen's own figures (spec 079 — the browser's network picker:
    /// "需要能看到这个网络上的余额吧"). Never fetched: a chain whose read
    /// failed, an unpriced or spam token, a total under half a cent, or a
    /// hidden balance shows NOTHING rather than a made-up zero.
    static func networkHoldings(_ balance: BalanceViewWire?, display: Display) -> [Int: String] {
        guard let balance, !balance.hidden else { return [:] }
        var usd: [Int: Double] = [:]
        for token in balance.tokens where !token.spam && !balance.failedChainIds.contains(token.chainId) {
            guard let price = token.priceUsd, let amount = Double(token.balance) else { continue }
            usd[token.chainId, default: 0] += amount * price
        }
        return usd.filter { $0.value >= 0.005 }.mapValues { value in
            display.glyph + Formats.number(
                value * display.rate, minimumFractionDigits: 2, maximumFractionDigits: 2
            )
        }
    }

    /// The same brand colours the settings list uses, and the same neutral for a
    /// chain nobody drew.
    private static func chainColor(_ chainId: Int) -> Color {
        SettingsLive.mark(chainId: chainId, name: "").color
    }
}

// MARK: - The activity feed

extension WalletLive {

    /// The feed the core already grouped, in the drawn day sections.
    ///
    /// **The headers are the core's.** It emits them interleaved with the items
    /// in render order, precisely so a header can never inter-sort with a row
    /// (invariant ⑥). This walks that list; it never re-groups, re-sorts or
    /// decides what belongs in a day.
    ///
    /// What is the shell's: the day's WORDING (今天 / 昨天 / a date), the
    /// counterparty's label, and how an amount reads.
    static func activityGroups(
        _ feed: FeedViewWire, loc: Loc, hidden: Bool, networks: WalletNetworks = .builtin
    ) -> [ActivityGroupModel] {
        var groups: [(label: String, rows: [ActivityRowModel])] = []
        for row in feed.rows {
            switch row {
            case .header(_, let dayStartMs, let timestamp):
                groups.append((dayLabel(dayStartMs: dayStartMs, timestamp: timestamp, loc: loc), []))
            case .item(let item):
                let built = activityRow(item, loc: loc, hidden: hidden, networks: networks)
                if groups.isEmpty {
                    // A row before any header — the core does not emit that, so
                    // this is a build reading a shape it does not know. The row
                    // still shows, under its own date, rather than vanishing.
                    groups.append((dayLabel(dayStartMs: item.dayStartMs,
                                            timestamp: item.timestamp, loc: loc), [built]))
                } else {
                    groups[groups.count - 1].rows.append(built)
                }
            }
        }
        return groups
            .filter { !$0.rows.isEmpty }
            .map { ActivityGroupModel(label: $0.label, rows: $0.rows) }
    }

    /// One row, in the core's words (spec 082 RG1, RG2; spec 093) — nothing
    /// here guesses what a record is, how it ended or what to call it.
    ///
    /// The second line is the core's `subtitle`, every part worded here and
    /// joined with " · " — a pending or failed operation says so first. A
    /// dApp row (a transaction or a signature) is titled by the core's verb
    /// and place — 「在 Uniswap 兑换」 — and carries money as before ("≈"
    /// when the core calls it the simulation's, with the coin that came
    /// back beside it), else the allowance it granted, 无限额 in the danger
    /// tone; a signature that granted nothing has no figure at all.
    static func activityRow(
        _ item: FeedItemWire, loc: Loc, hidden: Bool, networks: WalletNetworks = .builtin
    ) -> ActivityRowModel {
        let subtitle = subtitleText(item.subtitle, loc: loc, networks: networks)
        if let dapp = item.dapp {
            return dappRow(item, dapp: dapp, subtitle: subtitle, loc: loc, hidden: hidden)
        }
        let incoming = item.direction == .in
        let figure = (incoming ? "+" : "\u{2212}") + compactAmount(item.value, batch: item.batch)
        return ActivityRowModel(
            kind: incoming ? .received : .sent,
            title: loc.t(incoming ? "history.labelReceived" : "history.labelSent"),
            subtitle: subtitle,
            amount: hidden ? WalletFixtures.mask : figure,
            unit: item.symbol,
            positive: incoming,
            masked: hidden,
            badgeColor: chainColor(item.chainId),
            badgeLogoURL: Marks.chainLogoURL(item.chainId),
            itemId: item.id
        )
    }

    /// A dApp's row — its title, figure and allowance as the core decided them.
    private static func dappRow(
        _ item: FeedItemWire, dapp: FeedDappWire, subtitle: String, loc: Loc, hidden: Bool
    ) -> ActivityRowModel {
        let money = dappFigure(item, dapp: dapp)
        let allowance = money == nil ? dapp.allowance.flatMap { allowanceText($0, loc: loc) } : nil
        // Privacy masks a figure and keeps its unit; "无限额" is a risk to see,
        // not an amount, so it is never masked. Nothing to show, nothing to mask.
        let masked = hidden && (money != nil || (allowance.map { !$0.unlimited } ?? false))
        let amount: String
        let unit: String
        if let money {
            amount = masked ? WalletFixtures.mask : money
            unit = item.symbol
        } else if let allowance {
            amount = masked ? WalletFixtures.mask : allowance.amount
            unit = allowance.unit
        } else {
            amount = ""
            unit = ""
        }
        return ActivityRowModel(
            kind: .dapp,
            title: dappTitle(dapp, loc: loc),
            subtitle: subtitle,
            amount: amount,
            unit: unit,
            positive: false,
            masked: masked,
            badgeColor: chainColor(item.chainId),
            badgeLogoURL: Marks.chainLogoURL(item.chainId),
            itemId: item.id,
            danger: money == nil && allowance?.unlimited == true,
            received: dapp.received.map { change in
                [changeFigure(change, hidden: hidden), change.symbol]
                    .filter { !$0.isEmpty }.joined(separator: " ")
            }
        )
    }

    /// The headline verb (spec 093): `componentsUi.signing.<intent_term>`,
    /// else the recorded text — the signing sheet's own words for the same
    /// request (`SigningLive.localizedTerms`).
    static func dappVerb(_ dapp: FeedDappWire, loc: Loc) -> String {
        if let term = dapp.intentTerm {
            let key = "componentsUi.signing.\(term)"
            let word = loc.t(key)
            if word != key, !word.isEmpty { return word }
        }
        if let intent = dapp.intent?.trimmingCharacters(in: .whitespaces), !intent.isEmpty {
            return intent
        }
        // The core always names a verb (spec 093); a view without one is a
        // build older than its core, and still reads as a dApp's transaction.
        return loc.t("history.txLabelDappTx")
    }

    /// 「在 Uniswap 兑换」 — the verb at its place, or the verb alone.
    static func dappTitle(_ dapp: FeedDappWire, loc: Loc) -> String {
        let verb = dappVerb(dapp, loc: loc)
        guard let place = dapp.place, !place.isEmpty else { return verb }
        return loc.t("history.dappRowTitle", vars: ["intent": verb, "place": place])
    }

    /// The money a dApp row moved: "−0.001", or "≈ −100" when the core says
    /// the figure is the simulation's. `nil` when it moved none.
    static func dappFigure(_ item: FeedItemWire, dapp: FeedDappWire) -> String? {
        guard let value = item.value else { return nil }
        let sign = item.direction == .in ? "+" : "\u{2212}"
        return (dapp.estimated ? "≈ " : "") + sign + compactAmount(value)
    }

    /// One of a dApp's balance changes as a figure, in the signing sheet's
    /// own form plus "≈" on every line the wallet cannot vouch for: "≈ +0.03",
    /// "−100". An unverified token keeps its direction and never a number.
    static func changeFigure(_ change: FeedDappChangeWire, hidden: Bool) -> String {
        let sign = change.direction == .in ? "+" : "\u{2212}"
        guard change.verified, let value = change.value else { return sign }
        let digits = hidden ? WalletFixtures.mask : compactAmount(value)
        return (change.exact ? "" : "≈ ") + sign + digits
    }

    /// An allowance, split into the figure and its unit: 「无限额」 + "USDC"
    /// (danger), or "100" + "USDC". `nil` for one that states nothing.
    static func allowanceText(
        _ allowance: FeedAllowanceWire, loc: Loc
    ) -> (amount: String, unit: String, unlimited: Bool)? {
        if allowance.unlimited {
            return (loc.t("componentsUi.signingApprove.unlimitedValue"), allowance.symbol, true)
        }
        guard let value = allowance.value, !value.isEmpty else { return nil }
        return (compactAmount(value), allowance.symbol, false)
    }

    /// A row's second line, from the core's parts (spec 093): status in the
    /// words this shell already says it with, "至 / 来自" somebody (named, or
    /// their short address), a site verbatim, a network by its name, a day as
    /// the date headers say it.
    static func subtitleText(
        _ lines: [FeedLineWire], loc: Loc, networks: WalletNetworks = .builtin
    ) -> String {
        lines.compactMap { line -> String? in
            switch line {
            case .status(let status): statusPrefix(status, loc: loc)
            case .to(let address, let name):
                loc.t("history.toName", vars: ["name": name ?? AddressText.short(address)])
            case .from(let address, let name):
                loc.t("history.fromName", vars: ["name": name ?? AddressText.short(address)])
            case .site(let site): site
            case .network(let chainId): chainName(chainId, networks: networks)
            // A contact's row has no headers over it: its day is worded as
            // a header is (its midnight names the date).
            case .day(let dayStartMs): dayLabel(dayStartMs: dayStartMs, timestamp: dayStartMs / 1000, loc: loc)
            case .unknown: nil
            }
        }
        .filter { !$0.isEmpty }
        .joined(separator: " · ")
    }

    /// A network as people name it — from the wallet's list, so a network
    /// the person added is its own name, not its chain id.
    static func chainName(_ chainId: Int, networks: WalletNetworks = .builtin) -> String {
        networks.meta(chainId)?.displayName ?? String(chainId)
    }

    /// "处理中" / "失败" / "未知" for a row that is not confirmed; `nil` for one
    /// that is. "未知" (087 F04) is a pending record nothing will settle — the
    /// corpus's bare "Unknown", never "failed": it may have been sent.
    static func statusPrefix(_ status: FeedTxStatusWire, loc: Loc) -> String? {
        switch status {
        case .confirmed: nil
        case .pending: loc.t("componentsTx.detail.statusPending")
        case .failed: loc.t("componentsTx.detail.statusFailed")
        case .unknown: loc.t("componentsUi.signing.intentUnknown")
        }
    }

    /// 今天 / 昨天 / a date.
    ///
    /// The wording is the shell's because it depends on the device's clock and
    /// locale; the DAY itself is the core's `day_start_ms`, computed from the
    /// same device timezone when the record was read.
    static func dayLabel(dayStartMs: Double, timestamp: Double, loc: Loc) -> String {
        let calendar = Calendar.current
        let day = Date(timeIntervalSince1970: dayStartMs / 1000)
        if calendar.isDateInToday(day) { return loc.t("componentsUi.dayGroup.today") }
        if calendar.isDateInYesterday(day) { return loc.t("componentsUi.dayGroup.yesterday") }
        // The person's own DATE preset (spec 056). Until then this read the
        // device's locale, which is the thing the presets exist to override:
        // one wallet, one order, everywhere they open it.
        return Formats.date(Date(timeIntervalSince1970: timestamp))
    }

    /// The amount as a row shows it — grouped, and never rounded UP.
    ///
    /// Done in string space, like every other amount in this client: a receipt
    /// can carry more significant digits than a `Double` holds, and a glance
    /// view that quietly rounds is how somebody's history stops matching the
    /// chain. Long fractions are TRUNCATED at six places (the row is the
    /// glance; the detail sheet is where the exact figure belongs), so the
    /// number shown is never larger than the number received.
    static func compactAmount(_ value: String?, batch: FeedBatchWire? = nil) -> String {
        // A multi-token batch has no single amount to state — mixed tokens
        // cannot be summed — so the row states how many assets moved.
        guard let value, !value.isEmpty else { return "\(batch?.count ?? 0)" }
        let parts = value.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
        let whole = String(parts.first ?? "0")
        var fraction = parts.count > 1 ? String(parts[1]) : ""
        if fraction.count > 6 { fraction = String(fraction.prefix(6)) }
        while fraction.hasSuffix("0") { fraction.removeLast() }
        let grouped = grouped(whole)
        return fraction.isEmpty ? grouped : "\(grouped).\(fraction)"
    }

    /// Thousands separators inserted into a digit string, without the string
    /// ever becoming a number.
    private static func grouped(_ digits: String) -> String {
        guard digits.count > 3, digits.allSatisfy(\.isNumber) else { return digits }
        var out = ""
        for (offset, character) in digits.reversed().enumerated() {
            if offset > 0, offset % 3 == 0 { out.append(",") }
            out.append(character)
        }
        return String(out.reversed())
    }
}
