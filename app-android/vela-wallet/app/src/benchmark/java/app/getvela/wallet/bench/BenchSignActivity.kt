package app.getvela.wallet.bench

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import app.getvela.wallet.MainActivity
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.feature.signing.core.IncomingRequest
import org.json.JSONArray

/**
 * Native parity Phase 4, benchmark build types only: the signing-sheet journey's
 * request. A release-shaped build cannot load the local test dApp (no cleartext,
 * no `vela.openUrl`), so this asks the sheet open the way a page's request does
 * — through [app.getvela.wallet.AppContainer.openSigningRequest] — with a
 * `personal_sign` from a fixed origin: offline, the same sheet every run. The
 * answer goes nowhere; the journey closes the sheet itself.
 *
 *   adb shell am start -n app.getvela.wallet/app.getvela.wallet.bench.BenchSignActivity
 */
class BenchSignActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val container = (application as VelaWalletApplication).container
        val params = JSONArray().put(MESSAGE_HEX).put(BenchFixture.ADDRESS)
        container.openSigningRequest(
            request = IncomingRequest(
                id = "bench-sign-${System.currentTimeMillis()}",
                method = "personal_sign",
                paramsJson = params.toString(),
                origin = ORIGIN,
                transportId = "bench",
                chainId = 1,
            ),
            answer = {},
            rememberUserOp = {},
        )
        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT))
        finish()
    }

    private companion object {
        const val ORIGIN = "https://bench.getvela.app"

        /** "Sign in to bench.getvela.app" — a sign-in message, the commonest thing a page asks. */
        const val MESSAGE_HEX = "0x5369676e20696e20746f2062656e63682e67657476656c612e617070"
    }
}
