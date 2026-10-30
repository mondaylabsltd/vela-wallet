package app.getvela.wallet.feature.signing

import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.GuardView

/**
 * The signing sheet's haptic choreography (native parity Phase 3; haptics
 * rule ⑤, inventory ㉑ — `ClearSigningView.danger_haptic`): the sheet buzzes a
 * warning ONCE, as it opens on a request the core judged dangerous —
 * `eth_sign`, or a sign-in message whose domain does not bind to the page
 * asking (`clear_signing`), or an unbounded approval (`approval_guard`'s
 * `unlimited_warning`). The core decides each half; the sheet ORs the two and
 * decides nothing — the buzz and the red banner can never disagree.
 *
 * The rest of the choreography is the core's or the button's: the confirm
 * press is the button's own [app.getvela.wallet.core.platform.VelaHaptic.Press];
 * money leaving and a refusal are the send machine's `haptic` operation; money
 * arriving is the feed's.
 */
object SigningHaptics {
    fun warnOnOpen(clear: ClearSigningView, guard: GuardView): Boolean = clear.danger_haptic || guard.unlimited_warning
}
