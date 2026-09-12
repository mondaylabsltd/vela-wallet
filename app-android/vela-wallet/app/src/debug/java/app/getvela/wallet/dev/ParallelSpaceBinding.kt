package app.getvela.wallet.dev

import android.app.Application
import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.UserOpSigner
import java.time.Instant
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_dev_fixtures.FixtureAccountRecord
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress

/**
 * Debug: the door exists.
 *
 * The parallel space on Android (spec 043 US0, research D2): the real app —
 * chains, relay, storage, every screen — with one substitution: where a
 * passkey would sign, the core's fixed keyset signs, through the debug-only
 * `vela_dev_fixtures` library. Entered by an intent extra
 * (`--ez vela.parallelSpace true`), remembered across relaunches in a
 * debug-only preference, left on sign-out.
 *
 * **What entering does to the account list.** The web's parallel space lives
 * under its own storage prefix; this one shares the device's store, and the
 * device may be the founder's, with a real wallet on it. So entering APPENDS
 * the fixture account (`add_account`, which also makes it active) rather than
 * replacing the list (`set_wallet` would overwrite the real accounts), and
 * leaving removes exactly that record and re-reads the store.
 *
 * **Which wallet.** The MULTI-key one every fixture key co-owns — the golden
 * Safe the web's and the desktop's parallel spaces send dust from — with the
 * whole keyset as its founding keys and key 0 pinned as the signer. A
 * single-key fixture Safe would be a wallet nobody funded.
 */
object ParallelSpaceBinding {
    fun install(app: Application) {
        ParallelSpaceHook.install(DebugParallelSpace(app))
    }
}

private class DebugParallelSpace(private val app: Application) : ParallelSpaceProvider {

    private val prefs = app.getSharedPreferences("vela.parallel", Context.MODE_PRIVATE)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    override fun active(): Boolean = prefs.getBoolean(KEY_ACTIVE, false)

    private fun preferred(): Int = prefs.getInt(KEY_SIGN_WITH, 0)

    override fun fixtureAccount(): FixtureAccount = record(preferred()).let {
        FixtureAccount(it.index.toInt(), it.credentialIdHex, it.publicKeyHex, fixtureMultiAddress())
    }

    private fun record(index: Int): FixtureAccountRecord {
        val all = fixtureAccounts()
        return all.getOrNull(index) ?: all.first()
    }

    override fun signer(): UserOpSigner = FixtureUserOpSigner(preferred())

    override fun enter() {
        if (active()) return
        prefs.edit().putBoolean(KEY_ACTIVE, true).apply()
        val keyset = fixtureAccounts()
        val pinned = keyset.first()
        val address = fixtureMultiAddress()
        VelaLog.event("parallel", "enter", "keys" to keyset.size, "address" to address)
        // The account as onboarding would have written it for a multi-key
        // wallet: id = the pinned (first) credential, the whole keyset as
        // founding keys with `internal` transports, a name that says what it
        // is on every screen that prints one.
        val keys = JSONArray()
        keyset.forEachIndexed { index, key ->
            keys.put(
                JSONObject()
                    .put("credential_id", key.credentialIdHex)
                    .put("public_key_hex", key.publicKeyHex)
                    .put("name", if (index == 0) "Parallel space" else key.name)
                    .put("transports", "internal"),
            )
        }
        val account = JSONObject()
            .put("id", pinned.credentialIdHex)
            .put("name", "Parallel space")
            .put("address", address)
            .put("public_key_hex", pinned.publicKeyHex)
            .put("created_at_iso", Instant.now().toString())
            .put("keys", keys)
        val container = (app as VelaWalletApplication).container
        container.session.accountEstablished(JSONObject().put("type", "add_account").put("account", account))
    }

    override fun leave() {
        if (!active()) return
        prefs.edit().putBoolean(KEY_ACTIVE, false).apply()
        val pinned = fixtureAccounts().first()
        VelaLog.event("parallel", "leave", "address" to fixtureMultiAddress())
        val container = (app as VelaWalletApplication).container
        scope.launch {
            container.session.removeFixtureAccount(pinned.credentialIdHex)
        }
    }

    @Composable
    override fun Badge() {
        val fixture = fixtureAccount()
        Box(
            modifier = Modifier.fillMaxWidth().statusBarsPadding(),
            contentAlignment = Alignment.TopEnd,
        ) {
            Text(
                text = "平行空间 · #${fixture.index} · ${fixture.address.take(6)}…${fixture.address.takeLast(4)}",
                color = Color.Black,
                fontSize = 11.sp,
                modifier = Modifier
                    .padding(top = 2.dp, end = 8.dp)
                    .background(Color(0xFFFFC107))
                    .padding(horizontal = 8.dp, vertical = 3.dp),
            )
        }
    }

    private companion object {
        const val KEY_ACTIVE = "active"
        const val KEY_SIGN_WITH = "signWith"
    }
}

/** The fixture keyset as a [UserOpSigner]: the same shape the passkey answers, no prompt. */
private class FixtureUserOpSigner(private val preferred: Int) : UserOpSigner {
    override suspend fun sign(
        challenge: ByteArray,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod,
    ): Assertion {
        val signed = fixtureAssert(
            challenge = challenge,
            allowCredentialIds = listOfNotNull(credentialIdHex?.takeIf { it.isNotBlank() }),
            preferred = preferred.toUInt(),
        )
        VelaLog.event("parallel", "signed", "credential" to signed.credentialIdHex.take(12))
        return Assertion(
            credentialIdHex = signed.credentialIdHex,
            signatureDerHex = signed.signatureDerHex,
            authenticatorDataHex = signed.authenticatorDataHex,
            clientDataJsonHex = signed.clientDataJsonHex,
            userIdHex = null,
            authenticatorAttachment = "platform",
        )
    }
}
