package app.getvela.wallet

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerCallbacks

/**
 * Where `velawallet://sign-result?…` lands (spec 076) — and nothing else.
 *
 * **Why a door of its own rather than [MainActivity]'s.** The rule for this
 * callback is that it changes no route and no state. `MainActivity` is
 * `singleTop`, and the Custom Tab showing the signer page sits ABOVE it in
 * this task, so a `VIEW` intent aimed at `MainActivity` would not reach the
 * instance the person is in the middle of using: `singleTop` only reuses an
 * activity that is already on top, and Android would build a second
 * `MainActivity` over the tab — a fresh app, with the flow's screen gone.
 * Making `MainActivity` `singleTask` would fix that by changing how the whole
 * app's task behaves, for one callback.
 *
 * This activity costs nothing instead: it is transparent, it is never in
 * recents, it hands the URL to whichever request is waiting for it, and it
 * finishes. The wallet's own screen is never entered, so it cannot be
 * disturbed. What brings the app back over the tab afterwards is the flow
 * itself, once it has its answer (`TrustedSignerTab.bringBack`).
 *
 * A callback nobody is waiting for — a replay, a stale tab, a process that was
 * killed while the browser was in front — settles nothing and is dropped in
 * silence. There is nothing to tell a person about it and nothing they could
 * do, and an error card here would be over whatever they are doing now.
 */
class SignResultActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val settled = intent?.data?.toString()?.let(TrustedSignerCallbacks::deliver) == true
        // The answer landed, so the person is done with the page: bring the
        // wallet back over it.
        //
        // **This has to happen HERE, and it took a device to find out why.** The
        // flow's own code tried it (`TrustedSignerTab.bringBack`) and could not:
        // by then the wallet is a BACKGROUND app, and a background app may
        // start an activity in its own task but the system will not move that
        // task in front of the browser's. Measured twice, with and without
        // `NEW_TASK`: `START … LAUNCH_SINGLE_TOP … result code=3`
        // (`START_DELIVERED_TO_TOP`) and the wallet still behind the page.
        //
        // This activity is the one exception, because the system started it
        // itself — Chrome asked for it, in the foreground — so its own
        // `startActivity` is a foreground start and does reorder the task.
        //
        // Only when something was waiting. A callback for nobody must not yank
        // the wallet in front of whatever the person is doing.
        if (settled) bringTheWalletBack()
        finish()
        // No animation: there is nothing to see, and a transition would look
        // like the wallet reacting to something.
        overridePendingTransition(0, 0)
    }

    /**
     * The wallet's own task, in front, with the screen it already had.
     *
     * `SINGLE_TOP` on a `singleTop` activity means `onNewIntent` rather than a
     * rebuild, and the intent carries no data, so nothing is routed and no
     * state changes — the person simply sees the wallet again. `CLEAR_TOP`
     * finishes anything the wallet itself had stacked above it; `NEW_TASK` is
     * what names the existing task to return to.
     */
    private fun bringTheWalletBack() {
        val wallet = Intent(this, MainActivity::class.java).addFlags(
            Intent.FLAG_ACTIVITY_NEW_TASK or
                Intent.FLAG_ACTIVITY_CLEAR_TOP or
                Intent.FLAG_ACTIVITY_SINGLE_TOP,
        )
        runCatching { startActivity(wallet) }
    }
}
