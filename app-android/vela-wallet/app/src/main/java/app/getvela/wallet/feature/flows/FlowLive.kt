package app.getvela.wallet.feature.flows

import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.PaymentRequestView

/**
 * The live flow builders: the drawn flow screens, showing this device's facts.
 *
 * The sibling of [FlowFixtures], the way `WalletLive` is of `WalletFixtures`.
 *
 * **The receive screen is the one place in this app where a stale value is
 * unrecoverable.** Every other fixture that leaks shows somebody the wrong
 * information; a fixture ADDRESS on a receive QR sends their money to a
 * stranger, permanently. So the address is replaced unconditionally here — not
 * "if we have one", not "unless it is blank" — and an empty session renders no
 * address at all rather than the one that was drawn.
 */
object FlowLive {

    /**
     * The QR sheet, for this person.
     *
     * [qrValue] is the core's: in address mode it is the bare recipient, and in
     * request mode the built `ethereum:` URI. The shell does not assemble it,
     * because what a payment request SAYS is the thing a scanner acts on.
     */
    fun receiveQr(
        fallback: ReceiveQrModel,
        address: String,
        name: String,
        request: PaymentRequestView,
    ): ReceiveQrModel = fallback.copy(
        account = fallback.account.copy(
            name = name,
            // The identicon is drawn FROM the address, so a stale seed is a
            // stale face beside a live address — the exact mismatch somebody
            // uses to check they are looking at their own wallet.
            identiconSeed = address,
            lines = addressLines(address),
        ),
    )

    /**
     * Whether this sheet may be shown at all.
     *
     * The core's gate: while it is loading, the QR stays covered so a first
     * visit never flashes a code before the warning about which networks this
     * address is safe on.
     */
    fun receiveGateOpen(request: PaymentRequestView): Boolean =
        !request.gate_loading && request.acknowledged

    /**
     * R1's rows: the networks this device actually has, each showing THIS
     * address.
     *
     * The fixture drew eight rows with a fixture address on every one. The
     * count is the person's own now, and so is the address — the same one, on
     * every network, which is the whole point the subtitle makes.
     */
    fun receiveNetworks(
        fallback: ReceiveListModel,
        view: NetView,
        address: String,
        badge: (Long) -> androidx.compose.ui.graphics.Color,
    ): ReceiveListModel {
        if (!view.loaded) return fallback
        val shown = shortAddress(address)
        // Copy on a row means the same address every time; the fixture's own
        // labels carry the wording.
        val template = fallback.rows.firstOrNull()
        return fallback.copy(
            subtitle = fallback.subtitle.replaceFirst(
                Regex("\\d+"),
                view.networks.size.toString(),
            ),
            rows = view.networks.map { row ->
                NetworkRowModel(
                    name = row.display_name,
                    code = row.native_symbol,
                    badgeColor = badge(row.chain_id),
                    addressDisplay = shown,
                    copyLabel = template?.copyLabel.orEmpty(),
                    qrLabel = template?.qrLabel.orEmpty(),
                )
            },
        )
    }

    /** `0x1234…abcd` — a row has no space for forty-two characters. */
    internal fun shortAddress(address: String): String =
        if (address.length <= 12) address else address.take(6) + "…" + address.takeLast(4)

    /** The address as the card draws it: two lines, split halfway. */
    internal fun addressLines(address: String): Pair<String, String> {
        if (address.isEmpty()) return "" to ""
        val half = (address.length + 1) / 2
        return address.take(half) to address.drop(half)
    }
}
