package app.getvela.wallet.navigation

import app.getvela.wallet.feature.onboarding.core.SessionRoute

/**
 * r18-3 (§33; iOS r16-11 / r17-7, §26 / §30): what the root draws while a
 * cold start finds out which account is in front.
 *
 * Welcome ("Create Wallet / I already have a wallet", live buttons) says this
 * device holds no account, so it is drawn only once the core has READ the
 * account store and found it empty. Before that (`Loading`) the root draws
 * the page's own ground — nothing of any screen, nothing to press: the
 * NavHost's start destination (Welcome) flashed for 0.13–0.38 s on every
 * cold start the weekly lockup did not cover.
 *
 * And a multisig in front when the app last ran is the accounts machine's
 * (`vela.activeMultisig`), read from its store once the hub boots; until
 * then the home drew the Vela account's — the other account's home for
 * 0.63–0.75 s before the multisig's. So the ground also stands while the
 * stored front is unread, or names a multisig the hub has not loaded yet —
 * bounded by [HOLD_MS] from the moment the session says Wallet.
 */
object LaunchFront {
    /** The longest the wallet holds for the hub (its store read is local). */
    const val HOLD_MS = 2_000L

    /** The key the core keeps the multisig in front under (`multisig_shell::keys::ACTIVE`). */
    const val ACTIVE_KEY = "vela.activeMultisig"

    /** What the store said of the account in front at launch. */
    sealed interface Stored {
        /** Not read yet. */
        data object Unread : Stored

        /** A Vela account was in front (no multisig stored). */
        data object Vela : Stored

        /** This multisig was in front. */
        data class Multisig(val address: String) : Stored
    }

    /** Read once at launch, before the session boots (a debug seed first). */
    suspend fun read(store: app.getvela.wallet.core.data.KeyValueStore): Stored =
        runCatching { stored(store.read(ACTIVE_KEY)) }.getOrDefault(Stored.Vela)

    /** The stored value, read: a non-blank address is a multisig in front. */
    fun stored(raw: String?): Stored =
        raw?.trim { it == '"' || it.isWhitespace() }?.takeIf { it.isNotEmpty() }?.let(Stored::Multisig) ?: Stored.Vela

    /** What the root draws. */
    enum class Screen { Ground, Welcome, Wallet }

    /**
     * [hubLoaded]: the multisig accounts machine has read its store
     * (`MsAccountsView.loaded`). [expired]: [HOLD_MS] has passed since the
     * session said Wallet. [walletSettled]: the wallet route is the only one
     * the NavHost shows (no cross-fade out of Welcome under the lifted ground).
     */
    fun screen(route: SessionRoute, stored: Stored, hubLoaded: Boolean, expired: Boolean, walletSettled: Boolean = true): Screen =
        when (route) {
            SessionRoute.Loading -> Screen.Ground
            SessionRoute.Onboarding -> Screen.Welcome
            SessionRoute.Wallet -> when {
                expired -> Screen.Wallet
                !walletSettled -> Screen.Ground
                stored is Stored.Unread -> Screen.Ground
                stored is Stored.Multisig && !hubLoaded -> Screen.Ground
                else -> Screen.Wallet
            }
        }
}
