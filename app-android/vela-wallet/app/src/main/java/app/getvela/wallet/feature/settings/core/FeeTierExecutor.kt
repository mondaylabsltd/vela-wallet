package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore

/**
 * The only place the `fee_tier_pref` core touches the outside world (spec 068;
 * Android's since 069). The same two sentences the display currency speaks,
 * against the same store.
 *
 * `vela.feeTier` survives sign-out — which clears only the accounts and the
 * active index — because a speed preference belongs to the person and the
 * device, not to the account. It is a preference, not a cache, so no
 * storage sweep offers it for clearing.
 */
class FeeTierExecutor(private val store: KeyValueStore) {

    suspend fun perform(operation: FeeTierPrefOperation): FeeTierPrefShellResult = when (operation) {
        // Raw, and absent means "never chose": a name this build does not
        // know reads as the factory `fast` in the core rather than being
        // coerced here into something that would then go on the wire.
        is FeeTierPrefOperation.ReadStoredTier ->
            FeeTierPrefShellResult.StoredTier(store.read(KeyValueStore.Keys.FEE_TIER))

        // Best effort: the committed choice stays on screen either way.
        is FeeTierPrefOperation.WriteStoredTier -> {
            store.write(KeyValueStore.Keys.FEE_TIER, operation.tier)
            FeeTierPrefShellResult.TierWritten
        }
    }

    fun neutralAnswer(operation: FeeTierPrefOperation): FeeTierPrefShellResult = when (operation) {
        is FeeTierPrefOperation.ReadStoredTier -> FeeTierPrefShellResult.StoredTier(null)
        is FeeTierPrefOperation.WriteStoredTier -> FeeTierPrefShellResult.TierWritten
    }
}
