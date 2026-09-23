package app.getvela.wallet

import android.app.Activity
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
        intent?.data?.toString()?.let(TrustedSignerCallbacks::deliver)
        finish()
        // No animation: there is nothing to see, and a transition would look
        // like the wallet reacting to something.
        overridePendingTransition(0, 0)
    }
}
