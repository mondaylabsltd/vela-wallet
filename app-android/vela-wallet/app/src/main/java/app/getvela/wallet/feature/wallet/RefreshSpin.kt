package app.getvela.wallet.feature.wallet

import androidx.compose.runtime.Immutable

/**
 * How long the hero's refresh control keeps turning (issue 462).
 *
 * The core says when a refresh the person asked for is out
 * (`BalanceView.refreshing`) and leaves the minimum visible spin to the
 * shell (balance_dashboard.rs). Without one, a read answered from warm
 * connections flips true and false inside a frame or two: the tap looks
 * like it did nothing, which is the bug (#462) all over again.
 *
 * So the spin starts the moment the person taps (before the core's view has
 * even come back), or the moment the core says a refresh is out (a pull),
 * and ends only when BOTH the core has finished AND [MIN_SPIN_MS] has passed
 * since it started. While it turns, a second tap is refused — [tapped]
 * answers `null` and nothing is dispatched.
 *
 * Pure: times are passed in, so the rule is pinned by a JVM test, and the
 * screen only feeds it the core's flag and a clock.
 */
@Immutable
data class RefreshSpin(
    val spinning: Boolean = false,
    /** When this spin started (a monotonic clock, ms). */
    val since: Long = 0L,
    /** The core's `refreshing`, as last told. */
    val coreBusy: Boolean = false,
) {
    /** The person tapped: the spin starts now — or `null` while it is already turning (the control is inert). */
    fun tapped(now: Long): RefreshSpin? = if (spinning) null else copy(spinning = true, since = now)

    /** The core's `refreshing` changed. A pull's refresh starts the spin too. */
    fun core(busy: Boolean, now: Long): RefreshSpin = when {
        busy && !spinning -> copy(spinning = true, since = now, coreBusy = true)
        else -> copy(coreBusy = busy)
    }

    /** When the spin may stop — `null` while not spinning or while the core is still busy. */
    val releaseAt: Long? get() = if (spinning && !coreBusy) since + MIN_SPIN_MS else null

    /** The clock moved on: stop if the release time has come. */
    fun settle(now: Long): RefreshSpin = releaseAt?.takeIf { now >= it }?.let { copy(spinning = false) } ?: this

    companion object {
        /** The minimum visible spin (the core doc's 650 ms, the shells' to hold). */
        const val MIN_SPIN_MS: Long = 650L
    }
}
