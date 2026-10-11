package app.getvela.wallet.dev

import androidx.compose.runtime.Composable
import app.getvela.wallet.feature.send.core.UserOpSigner

/**
 * The parallel space's seam into the main source set (spec 043, research D2).
 *
 * The parallel space is the real app with one substitution — where a passkey
 * would sign, the core's fixed keyset signs — behind a door that exists only
 * in DEBUG builds. Main code never names the debug classes: it asks this
 * object, and the build type decides who answers. `ParallelSpaceBinding`
 * (one file under `src/debug`, one under `src/release`) installs a provider
 * at application start; the release one installs nothing, so every method
 * here answers "not active" and the badge draws nothing.
 *
 * A release APK therefore carries neither the door nor a path to the fixture
 * signer; the fixture cdylib and its bindings are debug-only source-set
 * artefacts and are not packaged (FR-001, SC-010).
 */
object ParallelSpaceHook {
    @Volatile
    private var provider: ParallelSpaceProvider? = null

    /** Installed once by the build type's `ParallelSpaceBinding`. */
    fun install(provider: ParallelSpaceProvider?) {
        this.provider = provider
        spaceAddress = null
    }

    /** Inside the space right now — the badge answers to this, not to the build type. */
    fun active(): Boolean = provider?.active() == true

    /** The door: enter (a debug intent extra), leave (sign-out). No-ops without a provider. */
    fun enter() {
        provider?.enter()
    }

    fun leave() {
        provider?.leave()
    }

    /** The fixture signer when active, else `null` (the real passkey signs). */
    fun signer(): UserOpSigner? = provider?.takeIf { it.active() }?.signer()

    /** The fixture account the space signs in as, when active. */
    fun fixtureAccount(): FixtureAccount? = provider?.takeIf { it.active() }?.fixtureAccount()

    /**
     * Whether [address] is the space's test wallet — asked of the ACCOUNT,
     * never of the flag.
     *
     * The in-app browser tells the core which kind of account came to the
     * front (`account_switched.space`), and the core then keeps the person's
     * connected sites apart from the space's. [active] cannot answer that:
     * [leave] clears the flag FIRST and removes the record a moment later, so
     * for that moment the test wallet is still the account in front while the
     * flag already says "outside" — and an ordinary switch to the test wallet
     * is exactly what re-pinned every one of the person's sites to it (the
     * owner's Arc Orbs connection, 2026-10-11). The address is a fact about
     * the build's keyset; it does not move when a preference does.
     *
     * `false` in a release build: no provider, no space, no such account.
     */
    fun isSpaceAccount(address: String?): Boolean {
        if (address.isNullOrBlank()) return false
        val provider = provider ?: return false
        val space = spaceAddress ?: runCatching { provider.fixtureAccount().address }.getOrNull()?.also { spaceAddress = it }
        return space != null && space.equals(address, ignoreCase = true)
    }

    /** The test wallet's address, read once: one keyset per build, one Safe for it. */
    @Volatile
    private var spaceAddress: String? = null

    /** Drawn over every route while active; nothing otherwise. */
    @Composable
    fun Badge() {
        provider?.takeIf { it.active() }?.Badge()
    }
}

/** What the debug source set provides; absent in release. */
interface ParallelSpaceProvider {
    fun active(): Boolean

    fun enter()

    fun leave()

    fun signer(): UserOpSigner

    fun fixtureAccount(): FixtureAccount

    @Composable
    fun Badge()
}

/**
 * One account of the core's fixed keyset — the same scalars, credential ids
 * and derived Safe the web's `/parallel` and the desktop's
 * `VELA_PARALLEL_SPACE=1` use, so a wallet one client created in its space is
 * the wallet another signs for in its own.
 */
data class FixtureAccount(
    val index: Int,
    val credentialIdHex: String,
    val publicKeyHex: String,
    val address: String,
)
