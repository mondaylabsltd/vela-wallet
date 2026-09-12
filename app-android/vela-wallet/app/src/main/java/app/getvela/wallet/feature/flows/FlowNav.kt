package app.getvela.wallet.feature.flows

import androidx.compose.runtime.Composable
import app.getvela.wallet.core.diagnostics.VelaLog
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue

/**
 * Flow navigation (spec 021 SC-002) — the Android port of the web's
 * `nav.svelte.ts`, with the same entries and the same steps.
 *
 * A stack, not a current-screen field. The mocks stack: Receive opens a network
 * list and a network opens its QR; Send runs picker → form → confirm → receipt.
 * Back has to unwind one level, which a single field cannot express.
 *
 * It lives inside the wallet route rather than as NavHost destinations because
 * these screens are still fixtures: a `composable(...)` per state would put
 * fixture screens in the app's real back stack and in its deep-link surface,
 * and `vela.startDestination` would happily launch one.
 */

/** Where a flow can be entered from the wallet home. */
enum class WalletFlowEntry { Receive, Send, Scan, Activity, Assets, AddToken, TokenDetail, TxDetail }

/**
 * The stack an entry opens, deepest last.
 *
 * `AddToken` opens two: the assets screen and the sheet over it. That is what
 * makes the back chevron in the T3 mock mean something — it goes to the list
 * you were adding to, not out of the flow entirely.
 */
private val ENTRIES: Map<WalletFlowEntry, List<FlowState>> = mapOf(
    WalletFlowEntry.Receive to listOf(FlowState.R1),
    WalletFlowEntry.Send to listOf(FlowState.SD1),
    WalletFlowEntry.Scan to listOf(FlowState.S1),
    WalletFlowEntry.Activity to listOf(FlowState.A1),
    WalletFlowEntry.Assets to listOf(FlowState.T1),
    WalletFlowEntry.AddToken to listOf(FlowState.T1, FlowState.T3),
    WalletFlowEntry.TokenDetail to listOf(FlowState.T1, FlowState.T2),
    WalletFlowEntry.TxDetail to listOf(FlowState.A1, FlowState.A2),
)

/** Pushes a step deeper within a flow that is already open. */
private val STEPS: Map<FlowStep, FlowState> = mapOf(
    FlowStep.ReceiveQr to FlowState.R2,
    FlowStep.TxDetail to FlowState.A2,
    FlowStep.TokenDetail to FlowState.T2,
    FlowStep.AddToken to FlowState.T3,
    FlowStep.SendForm to FlowState.SD2,
    FlowStep.SendConfirm to FlowState.SD3,
    FlowStep.SendReceipt to FlowState.SD4B,
    FlowStep.ContactPick to FlowState.SD2E,
    FlowStep.FeeToken to FlowState.SD2F,
    FlowStep.BatchImport to FlowState.SD2C,
    FlowStep.SendMulti to FlowState.SD1B,
    FlowStep.AddRecipient to FlowState.SD2B,
    FlowStep.Scan to FlowState.S1,
    FlowStep.Receive to FlowState.R1,
)

/** The open flow stack. Empty means the wallet home is showing. */
class FlowNavState internal constructor() {
    var stack by mutableStateOf<List<FlowState>>(emptyList())
        private set

    /**
     * Which row opened the detail on top of the stack.
     *
     * A transaction id, or a token's `chainId:contract`. Carried here because a
     * detail screen is ABOUT something, and the stack alone cannot say what:
     * every row was opening the same screen, so tapping this person's own POL
     * showed somebody else's transaction. The desktop shipped the same shape
     * once — a page that displayed contact A while its delete acted on contact
     * B — and the fix there was the same: look the target up ONCE, from an id
     * the navigation carries.
     *
     * Cleared whenever the stack changes without one, so a stale id can never
     * be read by the next screen.
     */
    var selected by mutableStateOf<String?>(null)
        private set

    val top: FlowState? get() = stack.lastOrNull()
    val isOpen: Boolean get() = stack.isNotEmpty()

    fun enter(entry: WalletFlowEntry, id: String? = null) {
        VelaLog.event("flows", "enter", "entry" to entry.name, "id" to id?.take(24))
        stack = ENTRIES.getValue(entry)
        selected = id
    }

    /**
     * Step deeper. Unknown steps are ignored rather than throwing: screens emit
     * navigation intents generously (`Done`, `Chains`, …) and a flow that has
     * nowhere to put one should do nothing, not crash a wallet.
     */
    fun push(step: FlowStep, id: String? = null) {
        if (step == FlowStep.Done) {
            close()
            return
        }
        // Spec 048: a detail step without an item is not a screen — pushing it
        // showed nothing and left a phantom Back (device-found 2026-09-12).
        if (id == null && (step == FlowStep.TxDetail || step == FlowStep.TokenDetail)) {
            VelaLog.event("flows", "detail step refused without an id", "step" to step.name)
            return
        }
        val next = STEPS[step] ?: return
        if (top != next) {
            stack = stack + next
            selected = id
        }
    }

    /** One level up. At the root this leaves the flow and shows the wallet. */
    fun back() {
        VelaLog.event("flows", "back", "top" to top?.name, "depth" to stack.size)
        stack = stack.dropLast(1)
        // The id belonged to the screen just left. Keeping it would let the
        // screen underneath read a target it was never opened for.
        selected = null
    }

    fun close() {
        VelaLog.event("flows", "close", "top" to top?.name)
        stack = emptyList()
        selected = null
    }
}

@Composable
fun rememberFlowNavState(): FlowNavState = remember { FlowNavState() }
